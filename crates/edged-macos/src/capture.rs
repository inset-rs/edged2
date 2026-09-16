//! Pictures of windows, through ScreenCaptureKit: a window on another Space,
//! minimized, or hidden, pictured without being brought forward.
//!
//! A picture comes as a Core Video buffer with an `IOSurface` behind it, which
//! the renderer samples where it lies; nothing here reads the pixels. Both
//! calls are made on a thread of their own, since the first of them sets the
//! system's capture service up and takes a second doing it, and answer on a
//! thread of the system's choosing; each answer completes the future the
//! caller awaits. The list of windows the system will picture is dear to read
//! and cheap to keep, so it is handed out whole for the caller to keep.

use std::collections::HashMap;
use std::future::Future;
use std::ptr::NonNull;
use std::sync::Mutex;

use futures_channel::oneshot;
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_core_foundation::CFRetained;
use objc2_core_media::CMSampleBuffer;
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetHeight, CVPixelBufferGetWidth, kCVPixelFormatType_32BGRA,
};
use objc2_foundation::NSError;
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

/// The answering side of a call's future, for a completion handler the system may call
/// more than once: the first call answers, later ones find nothing to answer with.
type Answer<T> = Mutex<Option<oneshot::Sender<T>>>;

fn answer<T>(answer: &Answer<T>, value: T) {
    if let Some(send) = answer.lock().ok().and_then(|mut send| send.take()) {
        let _ = send.send(value);
    }
}

/// Asks which windows can be pictured and answers them once the system has
/// looked. Takes tens of milliseconds: keep the answer.
pub fn capturable_windows() -> impl Future<Output = HashMap<WindowId, CaptureTarget>> {
    let (send, receive) = oneshot::channel();
    let send: Answer<Targets> = Mutex::new(Some(send));
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
                answer(&send, Targets(targets));
            },
        );
        unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
    });
    async move { receive.await.map(|targets| targets.0).unwrap_or_default() }
}

/// Pictures one window at up to `width` by `height` pixels, keeping its
/// proportions, and answers the picture; `None` when the system would not
/// picture it, as without screen recording access.
pub fn capture_window(
    target: CaptureTarget,
    width: u32,
    height: u32,
) -> impl Future<Output = Option<Picture>> {
    let (send, receive) = oneshot::channel();
    let send: Answer<Taken> = Mutex::new(Some(send));
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
                answer(&send, Taken(picture));
            });
        unsafe {
            SCScreenshotManager::captureSampleBufferWithFilter_configuration_completionHandler(
                &filter,
                &configuration,
                Some(&block),
            )
        };
    });
    async move { receive.await.ok().and_then(|taken| taken.0) }
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
