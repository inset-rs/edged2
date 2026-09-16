//! Reusable application fixture with simulated time and input.

use super::{gpu, view::CaptureView};
use inset_embedder::valo::Context;
use inset_embedder::{
    Clipboard, Offset, PointerChange, PointerData, PointerDataPacket, PointerDeviceKind,
    SystemFontSource, TargetPlatform,
};
use inset_foundation::{App, AppCell};
use inset_gestures::GestureBinding;
use inset_rendering::RenderParagraph;
use inset_scheduler::SchedulerBinding;
use inset_test::TestPlatform;
use inset_widgets::{AnyElement, WidgetsBinding};
use std::{cell::RefCell, rc::Rc, time::Duration};

/// In-memory platform clipboard for headless editing tests.
#[derive(Default)]
struct MemoryClipboard(RefCell<Option<String>>);

impl Clipboard for MemoryClipboard {
    fn set_text(&self, text: &str) {
        *self.0.borrow_mut() = Some(text.to_owned());
    }

    fn text(&self) -> Option<String> {
        self.0.borrow().clone()
    }

    fn has_strings(&self) -> bool {
        self.0
            .borrow()
            .as_ref()
            .is_some_and(|text| !text.is_empty())
    }
}
/// Mounts an arbitrary application with simulated time and pointer input.
pub struct Fixture {
    /// Application arena and scheduler checkpoints.
    pub cell: Rc<AppCell>,

    /// Headless rendered output.
    pub view: Rc<CaptureView>,

    /// Simulated event timestamp.
    pub at: Duration,

    /// Gesture identifiers increase on each down, as in the engine converter.
    next_pointer: i64,

    /// Identifier of the current touch gesture, shared with manually constructed packets.
    pub touch_pointer: i64,
}

impl Fixture {
    /// Mounts whatever `run` starts, for a test of one control.
    pub fn with_root(size: [u32; 2], run: impl FnOnce(&mut App)) -> Self {
        let (device, queue) = gpu::headless_device().expect("GPU required");
        let view = Rc::new(CaptureView {
            size,
            renderer: RefCell::new(Context::new(device, queue)),
            pixels: RefCell::new(Vec::new()),
        });
        let platform = TestPlatform::new()
            .on(TargetPlatform::MacOS)
            .with_view(view.clone())
            .with_clipboard(Rc::new(MemoryClipboard::default()))
            .with_font_source(|| Box::new(SystemFontSource::platform()));
        let cell = AppCell::with_platform(Rc::new(platform));
        {
            let mut app = cell.borrow_mut();
            inset_painting::PaintingBinding::instance(&mut app).install_fonts(&mut app, |fonts| {
                fonts.add_source(SystemFontSource::platform());
            });
            run(&mut app);
        }
        cell.elapse(Duration::ZERO);
        let mut result = Self {
            cell,
            view,
            at: Duration::ZERO,
            next_pointer: 0,
            touch_pointer: 0,
        };
        result.pump();
        result
    }

    /// Runs ten 20 ms frames, processing microtasks between frame phases.
    pub fn pump(&mut self) {
        for _ in 0..10 {
            self.at += Duration::from_millis(20);
            SchedulerBinding::handle_begin_frame(&mut self.cell.borrow_mut(), Some(self.at));
            self.cell.checkpoint();
            SchedulerBinding::handle_draw_frame(&mut self.cell.borrow_mut());
            self.cell.checkpoint();
        }
    }

    /// Enumerates painted subtrees while excluding descendants of Offstage widgets.
    fn onstage_elements(&self) -> Vec<AnyElement> {
        let mut app = self.cell.borrow_mut();
        let root = WidgetsBinding::instance(&mut app)
            .root_element(&app)
            .unwrap();
        let mut elements = vec![root];
        let mut index = 0;
        while index < elements.len() {
            let element = elements[index];
            let offstage = inset_widgets::downcast_widget::<inset_widgets::Offstage>(
                element.widget(&app).as_ref(),
            )
            .is_some_and(|widget| widget.offstage);
            if !offstage {
                elements.extend(element.children(&app));
            }
            index += 1;
        }
        elements
    }

    /// Finds the center of the first onstage text label.
    pub fn find(&self, text: &str) -> Offset {
        let elements = self.onstage_elements();
        let app = self.cell.borrow();
        for element in elements {
            if let Some(object) = element.render_object(&app)
                && let Some(paragraph) = object.downcast::<RenderParagraph>(&app)
                && paragraph.text(&app).to_plain_text(true, true) == text
            {
                let object = object.as_box().unwrap();
                let size = object.size(&app);
                return object.local_to_global(
                    &app,
                    Offset::new(size.width() / 2.0, size.height() / 2.0),
                    None,
                );
            }
        }
        panic!("missing label {text}")
    }

    /// Sends a touch event at the fixture timestamp.
    pub fn send(&mut self, change: PointerChange, point: Offset) {
        if change == PointerChange::Down {
            self.next_pointer += 1;
            self.touch_pointer = self.next_pointer;
        }
        let mut app = self.cell.borrow_mut();
        GestureBinding::instance(&mut app).handle_pointer_data_packet(
            &mut app,
            PointerDataPacket::new(vec![PointerData {
                change,
                kind: PointerDeviceKind::Touch,
                time_stamp: self.at,
                pointer_identifier: self.touch_pointer,
                physical_x: point.dx(),
                physical_y: point.dy(),
                ..Default::default()
            }]),
        );
        drop(app);
        self.cell.checkpoint();
    }

    /// Taps a text label and pumps the resulting frames.
    pub fn tap(&mut self, text: &str) {
        let p = self.find(text);
        self.send(PointerChange::Down, p);
        self.at += Duration::from_millis(20);
        self.send(PointerChange::Up, p);
        self.pump();
    }

    /// Checks the frame and optionally writes it under EDGED_CAPTURE_DIR.
    pub fn capture(&self, name: &str) {
        let pixels = self.view.pixels.borrow();
        assert_eq!(
            pixels.len(),
            (self.view.size[0] * self.view.size[1] * 4) as usize
        );
        assert!(pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
        if let Ok(dir) = std::env::var("EDGED_CAPTURE_DIR") {
            let dir = std::path::Path::new(&dir);
            std::fs::create_dir_all(dir).unwrap();
            gpu::write_png(&dir.join(format!("{name}.png")), self.view.size, &pixels);
        }
    }
}
