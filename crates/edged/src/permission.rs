//! What the panel shows until accessibility access is granted. The panel
//! stays out while it does: a strip cannot say what it needs.

use edged_core::{Permission, Permissions};
use inset::{
    App, BuildContext, Column, CrossAxisAlignment, EdgeInsetsGeometry, Entity, FontWeight,
    IntoWidget, Listener, Padding, SizedBox, StatelessWidget, Text, WidgetRef,
};
use inset_winui::{Button, ButtonStyle, HyperlinkButton};

use crate::theme::Design;

const PADDING: f64 = 16.0;

/// The one permission a window switcher cannot work without, and the two ways
/// to grant it.
#[derive(Debug)]
pub struct PermissionView {
    pub design: Design,
    pub permissions: Entity<Permissions>,
}

impl StatelessWidget for PermissionView {
    fn build(&self, _app: &mut App, _context: BuildContext) -> WidgetRef {
        let palette = &self.design.palette;
        let grant = self.permissions.clone();
        let settings = self.permissions.clone();
        let text = palette.label;
        let secondary = palette.secondary_label;
        Padding::new(EdgeInsetsGeometry::all(PADDING))
            .child(
                Column::new()
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .children(vec![
                        Text::new("Accessibility access")
                            .style(palette.text(15.0, FontWeight::W600, text))
                            .into_widget(),
                        SizedBox::new().height(6.0).into_widget(),
                        Text::new(
                            "Edged reads the windows of other applications through macOS's \
                             accessibility interface, which macOS asks you to allow.",
                        )
                        .style(palette.text(13.0, FontWeight::W400, secondary))
                        .into_widget(),
                        SizedBox::new().height(PADDING).into_widget(),
                        Button::text(
                            "Grant access",
                            Listener::new(move |app: &mut App| {
                                grant.update(app, |permissions, cx| {
                                    permissions.request(cx, Permission::Accessibility)
                                })
                            }),
                        )
                        .style(ButtonStyle::Accent)
                        .into_widget(),
                        SizedBox::new().height(8.0).into_widget(),
                        Button::text(
                            "Open System Settings",
                            Listener::new(move |app: &mut App| {
                                settings.read(app).open_settings(Permission::Accessibility)
                            }),
                        )
                        .into_widget(),
                        SizedBox::new().height(PADDING).into_widget(),
                        Text::new("The panel fills in as soon as access is granted.")
                            .style(palette.text(11.0, FontWeight::W400, secondary))
                            .into_widget(),
                        SizedBox::new().height(8.0).into_widget(),
                        HyperlinkButton::text(
                            "Quit Edged",
                            Listener::new(|_app| std::process::exit(0)),
                        )
                        .into_widget(),
                    ]),
            )
            .into_widget()
    }
}
