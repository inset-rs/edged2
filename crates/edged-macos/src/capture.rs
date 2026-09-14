//! Pictures of windows, through ScreenCaptureKit: a window on another Space,
//! minimized, or hidden, pictured without being brought forward.
//!
//! A picture comes as a Core Video buffer with an `IOSurface` behind it, which
//! the renderer samples where it lies; nothing here reads the pixels. Both
//! calls are made on a thread of their own, since the first of them sets the
//! system's capture service up and takes a second doing it, and answer on a
//! thread of the system's choosing; the answers are carried to the main
//! thread, where the caller's continuation runs. The list of windows the
//! system will picture is dear to read and cheap to keep, so it is handed out
//! whole for the caller to keep.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::NonNull;
use std::sync::Mutex;

use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_core_foundation::CFRetained;
use objc2_core_media::CMSampleBuffer;
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetHeight, CVPixelBufferGetWidth, kCVPixelFormatType_32BGRA,
};
use objc2_foundation::{NSError, NSOperationQueue};
use objc2_screen_capture_kit::{
    SCCaptureResolutionType, SCContentFilter, SCScreenshotManager, SCShareableContent,
    SCStreamConfiguration, SCWindow,
};

use crate::private::WindowId;

/// A picture of a window: a finished Core Video buffer, blue, green, red and
/// alpha at eight bits each, with an `IOSurface` behind it.
pub struct Picture {
    pub width: u32,
    pub height: u32,
    buffer: CFRetained<CVPixelBuffer>,
}

impl Picture {
    /// The buffer, for the renderer to sample; a retained reference, so the
    /// picture and the renderer each keep it alive as long as they need.
    pub fn buffer(&self) -> CFRetained<CVPixelBuffer> {
        self.buffer.clone()
    }
}

/// A picture on its way to the main thread.
struct Taken(Option<Picture>);

// SAFETY: the buffer is finished with when the system hands it over and is only ever
// retained and released across threads, which Core Foundation permits.
unsafe impl Send for Taken {}

/// A window the system will picture: an immutable description the system
/// hands out, held on the main thread and lent to the capture thread.
#[derive(Clone)]
pub struct CaptureTarget(Retained<SCWindow>);

// SAFETY: the description is immutable and the system reads it from any thread; only
// ownership crosses, and Objective-C retains and releases are thread-safe.
unsafe impl Send for CaptureTarget {}

impl CaptureTarget {
    fn window(&self) -> &SCWindow {
        &self.0
    }
}

/// The windows the system will picture, keyed by window id.
struct Targets(HashMap<WindowId, CaptureTarget>);

// SAFETY: the system hands the list out on its own thread and it is read on the main
// thread alone from then on; the objects in it are immutable descriptions.
unsafe impl Send for Targets {}

/// Asks which windows can be pictured and hands them to `on_done` on the
/// main thread. Takes tens of milliseconds: keep the answer.
pub fn capturable_windows(on_done: impl FnOnce(HashMap<WindowId, CaptureTarget>) + 'static) {
    let reply = Reply::new(move |targets: Targets| on_done(targets.0));
    std::thread::spawn(move || {
        let block = block2::RcBlock::new(
            move |content: *mut SCShareableContent, _error: *mut NSError| {
                let mut targets = HashMap::new();
                if let Some(content) = NonNull::new(content) {
                    // SAFETY: the system hands over a live object for the block's duration.
                    let content: &SCShareableContent = unsafe { content.as_ref() };
                    for window in unsafe { content.windows() }.iter() {
                        let id = unsafe { window.windowID() } as WindowId;
                        targets.insert(id, CaptureTarget(window));
                    }
                }
                reply.deliver(Targets(targets));
            },
        );
        unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
    });
}

/// Pictures one window at up to `width` by `height` pixels, keeping its
/// proportions, and hands the picture to `on_done` on the main thread; `None`
/// when the system would not picture it, as without screen recording access.
pub fn capture_window(
    target: &CaptureTarget,
    width: u32,
    height: u32,
    on_done: impl FnOnce(Option<Picture>) + 'static,
) {
    let reply = Reply::new(move |taken: Taken| on_done(taken.0));
    let target = target.clone();
    std::thread::spawn(move || {
        let filter = unsafe {
            SCContentFilter::initWithDesktopIndependentWindow(
                SCContentFilter::alloc(),
                target.window(),
            )
        };
        let configuration = unsafe { SCStreamConfiguration::new() };
        unsafe {
            configuration.setWidth(width as usize);
            configuration.setHeight(height as usize);
            configuration.setPixelFormat(kCVPixelFormatType_32BGRA);
            configuration.setScalesToFit(true);
            configuration.setPreservesAspectRatio(true);
            configuration.setShowsCursor(false);
            configuration.setIgnoreShadowsSingleWindow(true);
            configuration.setIgnoreGlobalClipSingleWindow(true);
            configuration.setCaptureResolution(SCCaptureResolutionType::Best);
        }
        let block =
            block2::RcBlock::new(move |sample: *mut CMSampleBuffer, _error: *mut NSError| {
                // SAFETY: the system hands over a live sample for the block's duration.
                let picture =
                    NonNull::new(sample).and_then(|sample| picture_of(unsafe { sample.as_ref() }));
                reply.deliver(Taken(picture));
            });
        unsafe {
            SCScreenshotManager::captureSampleBufferWithFilter_configuration_completionHandler(
                &filter,
                &configuration,
                Some(&block),
            )
        };
    });
}

/// The sample's buffer, retained: what the renderer samples.
fn picture_of(sample: &CMSampleBuffer) -> Option<Picture> {
    let buffer = unsafe { sample.image_buffer() }?;
    Some(Picture {
        width: CVPixelBufferGetWidth(&buffer) as u32,
        height: CVPixelBufferGetHeight(&buffer) as u32,
        buffer,
    })
}

thread_local! {
    /// Continuations waiting on the main thread for an answer from another.
    static PENDING: RefCell<HashMap<u64, Box<dyn Any>>> = RefCell::new(HashMap::new());
    static NEXT_REPLY: RefCell<u64> = const { RefCell::new(0) };
}

/// A continuation left on the main thread, and the ticket that finds it
/// again from wherever the answer arrives.
struct Reply<T> {
    id: u64,
    _payload: std::marker::PhantomData<fn(T)>,
}

impl<T: Send + 'static> Reply<T> {
    fn new(on_done: impl FnOnce(T) + 'static) -> Reply<T> {
        let id = NEXT_REPLY.with(|next| {
            let mut next = next.borrow_mut();
            *next += 1;
            *next
        });
        let boxed: Box<dyn FnOnce(T)> = Box::new(on_done);
        PENDING.with(|pending| pending.borrow_mut().insert(id, Box::new(boxed)));
        Reply {
            id,
            _payload: std::marker::PhantomData,
        }
    }

    /// Carries the answer to the main thread and runs the continuation there.
    fn deliver(&self, payload: T) {
        let id = self.id;
        let slot = Mutex::new(Some(payload));
        let block = block2::RcBlock::new(move || {
            let Some(payload) = slot.lock().ok().and_then(|mut slot| slot.take()) else {
                return;
            };
            let continuation = PENDING.with(|pending| pending.borrow_mut().remove(&id));
            if let Some(continuation) = continuation
                && let Ok(continuation) = continuation.downcast::<Box<dyn FnOnce(T)>>()
            {
                continuation(payload);
            }
        });
        unsafe { NSOperationQueue::mainQueue().addOperationWithBlock(&block) };
    }
}
