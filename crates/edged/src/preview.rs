//! What the preview window shows: the picture of the window the pointer
//! rests on, framed and named so it reads as a picture and not the window.
//!
//! The picture arrives as a buffer the renderer samples where it lies: the
//! host's image context, reached by naming the host, imports it as a texture
//! with no copy. It is made into an image once per picture taken, and kept
//! until the next.

use std::time::{Duration, Instant};

use edged_core::Core;
use edged_macos::{Picture, WindowId};
use inset::{
    Align, AlignmentGeometry, Animation, AnimationBehavior, AnimationController, App, Border,
    BorderRadius, BorderRadiusGeometry, BorderSide, BorderStyle, BoxDecoration, BoxFit,
    BuildContext, ClipRRect, Color, DecoratedBox, EdgeInsetsGeometry, FadeTransition, FontWeight,
    Handle, ImageInfo, IntoWidget, Listener, MediaQuery, Padding, RawImage,
    SingleTickerProviderStateMixin, SingleTickerProviderStateMixinData, SizedBox, Stack, StackFit,
    State, StateData, StatefulWidget, Text, TextOverflow, Ticker, TickerCallback,
    TickerProviderObject, WidgetRef,
};
use inset_embedder_winit::WinitPlatform;

use crate::theme::Design;

/// The size a preview's window is made with, before the first rest.
pub const INITIAL_SIZE: [f64; 2] = [400.0, 300.0];
pub const FRAME_RADIUS: f64 = 10.0;
const FRAME_WIDTH: f64 = 2.0;
const TITLE_SIZE: f64 = 12.0;
/// How much of the picture shows: less than all of it, so the blur behind
/// shows through and the picture reads as one, not as the window arriving.
const PICTURE_OPACITY: f64 = 0.9;
/// How long a picture takes to fade in when it arrives; the window has
/// already glided to where it belongs.
const PICTURE_FADE: Duration = Duration::from_millis(120);

#[derive(Debug)]
pub struct PreviewContent {
    pub core: Core,
}

pub struct PreviewContentState {
    state: StateData<PreviewContent>,
    single_ticker_provider: SingleTickerProviderStateMixinData,
    /// The image made of the last picture that was ready, with the window it
    /// is of, its title, and which picture it was made of.
    image: Option<(WindowId, String, Instant, ImageInfo)>,
    /// Runs from nothing to the picture's opacity each time a new picture is drawn.
    fade: Option<Handle<AnimationController>>,
}

impl StatefulWidget for PreviewContent {
    type State = PreviewContentState;

    fn create_state(&self) -> PreviewContentState {
        PreviewContentState {
            state: StateData::new(),
            single_ticker_provider: SingleTickerProviderStateMixinData::default(),
            image: None,
            fade: None,
        }
    }
}

impl SingleTickerProviderStateMixin for PreviewContentState {
    fn single_ticker_provider_data(
        self: Handle<Self>,
        app: &App,
    ) -> &SingleTickerProviderStateMixinData {
        &app.get(self).single_ticker_provider
    }

    fn single_ticker_provider_data_mut(
        self: Handle<Self>,
        app: &mut App,
    ) -> &mut SingleTickerProviderStateMixinData {
        &mut app.get_mut(self).single_ticker_provider
    }
}

impl TickerProviderObject for PreviewContentState {
    fn create_ticker(self: Handle<Self>, app: &mut App, on_tick: TickerCallback) -> Handle<Ticker> {
        SingleTickerProviderStateMixin::create_ticker(self, app, on_tick)
    }
}

impl State for PreviewContentState {
    type Widget = PreviewContent;
    inset::state_accessors!();

    fn init_state(self: Handle<Self>, app: &mut App) {
        let fade = AnimationController::create(
            app,
            Some(1.0),
            Some(PICTURE_FADE),
            None,
            0.0,
            1.0,
            AnimationBehavior::Normal,
            self,
        );
        fade.add_listener(app, Listener::new(move |app| self.set_state(app, |_| {})));
        app.get_mut(self).fade = Some(fade);
    }

    fn dispose(self: Handle<Self>, app: &mut App) {
        if let Some(fade) = app.get_mut(self).fade.take() {
            fade.dispose(app);
        }
        SingleTickerProviderStateMixin::dispose(self, app);
    }

    fn build(self: Handle<Self>, app: &mut App, context: BuildContext) -> WidgetRef {
        let core = self.widget(app).core.clone();
        let brightness = MediaQuery::platform_brightness_of(app, context);
        let design = Design::of(app, brightness, &core.appearance);
        let palette = design.palette;
        let ready = {
            let previews = core.previews.read(app);
            previews.showing.clone().and_then(|rest| {
                let preview = previews.picture_of(rest.window.id)?;
                Some((rest, preview.taken, preview.picture.clone()))
            })
        };
        let platform = app.platform();
        if let Some((rest, taken, picture)) = ready {
            let title = if rest.window.title.is_empty() {
                "Untitled".to_owned()
            } else {
                rest.window.title.clone()
            };
            image_of(self, app, rest.window.id, title, taken, || {
                import_picture(&platform, &picture)
            });
        }
        // What is drawn is the last picture that was ready: while the next window's
        // is on its way, the one before stays, so nothing blank comes between.
        let Some((_, title, _, image)) = app.get(self).image.clone() else {
            return SizedBox::new().into_widget();
        };
        let image = Some(image);
        let body: WidgetRef = match (image, app.get(self).fade) {
            (Some(image), Some(fade)) => FadeTransition::new(fade.view())
                .child(
                    RawImage::new(Some(image))
                        .fit(BoxFit::Contain)
                        .opacity(PICTURE_OPACITY),
                )
                .into_widget(),
            _ => SizedBox::new().into_widget(),
        };
        let caption = Align::new()
            .alignment(AlignmentGeometry::BOTTOM_CENTER)
            .child(
                DecoratedBox::new(
                    BoxDecoration::new().color(Color::from_argb(0xB3, 0x00, 0x00, 0x00)),
                )
                .child(
                    Padding::new(EdgeInsetsGeometry::symmetric(4.0, 10.0)).child(
                        Text::new(title)
                            .style(palette.text(
                                TITLE_SIZE,
                                FontWeight::W500,
                                Color::from_argb(0xFF, 0xFF, 0xFF, 0xFF),
                            ))
                            .max_lines(1)
                            .overflow(TextOverflow::Ellipsis),
                    ),
                ),
            )
            .into_widget();
        DecoratedBox::new(
            BoxDecoration::new()
                .border(Border::all(
                    palette.accent,
                    FRAME_WIDTH,
                    BorderStyle::Solid,
                    BorderSide::STROKE_ALIGN_INSIDE,
                ))
                .border_radius(BorderRadius::circular(FRAME_RADIUS)),
        )
        .child(
            ClipRRect::new()
                .border_radius(BorderRadiusGeometry::circular(FRAME_RADIUS))
                .child(
                    Stack::new()
                        .fit(StackFit::Expand)
                        .children(vec![body, caption]),
                ),
        )
        .into_widget()
    }
}

/// Makes the image of the picture taken at `taken` of window `id` once and
/// keeps it while the same picture shows. A picture of another window fades
/// in from nothing; a fresher picture of the same window replaces its own.
/// The picture as an image on the host's device: its buffer sampled where it lies, with
/// nothing copied. `None` when the host is not the winit one, has no device yet, or
/// cannot take the buffer.
fn import_picture(
    platform: &inset::ui::PlatformRef,
    picture: &Picture,
) -> Option<inset::ui::Image> {
    let images = platform.downcast_ref::<WinitPlatform>()?.image_context()?;
    let texture = valo_codec_apple::import_pixel_buffer(images.device(), picture.buffer()).ok()?;
    images.import_texture(texture, false).ok()
}

fn image_of(
    this: Handle<PreviewContentState>,
    app: &mut App,
    id: WindowId,
    title: String,
    taken: Instant,
    import: impl FnOnce() -> Option<inset::ui::Image>,
) {
    if let Some((kept_id, _, kept_taken, _)) = &app.get(this).image
        && *kept_id == id
        && *kept_taken == taken
    {
        return;
    }
    let another_window = app
        .get(this)
        .image
        .as_ref()
        .is_none_or(|(kept, _, _, _)| *kept != id);
    let Some(image) = import() else {
        return;
    };
    app.get_mut(this).image = Some((id, title, taken, ImageInfo::new(image)));
    if another_window && let Some(fade) = app.get(this).fade {
        fade.set_value(app, 0.0);
        fade.forward(app, None);
    }
}
