//! One look for SkinCraft, both editors and its own panels.
//!
//! PhotoCraft and VectorCraft each install their own theme (visuals, spacing, text styles, fonts
//! and the colour tokens their custom painting reads) once, on their first frame, and again when
//! their preferences change it; whichever ran last would win, so the window changed character
//! when switching editors. SkinCraft owns the theme instead: every frame, after the active
//! editor's logic, it writes both editors' tokens from one palette, keeps one egui style and one
//! font set, so the vector and raster editors, the file browser and the preview look the same.
//! The palette is the game's own (`ddi-theme`'s Infinito: its ground, plates and pink accent),
//! dark or light following the system.
use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};
use photocraft_ui_egui::theme as photo;
use vectorcraft_ui_egui::theme as vector;

/// The colours both editors are drawn in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub dark: bool,
    /// Behind documents (canvas surround, pasteboard).
    pub ground: Color32,
    /// Menu bars, tab strips, title and tool bars.
    pub chrome: Color32,
    /// Docks behind panels.
    pub dock: Color32,
    /// Panels and cards.
    pub panel: Color32,
    /// Buttons at rest.
    pub raised: Color32,
    /// Text fields and inputs.
    pub field: Color32,
    pub field_border: Color32,
    pub line: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    /// A selected row or tool.
    pub selected: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub text_off: Color32,
    pub accent: Color32,
    pub accent_strong: Color32,
    /// Text on the accent.
    pub on_accent: Color32,
    pub danger: Color32,
    pub warning: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

impl Palette {
    /// Infinito's colours (`crates/theme/src/infinito.toml` in the game).
    pub const DARK: Palette = Palette {
        dark: true,
        ground: hex(0x0a090d),
        chrome: hex(0x14121a),
        dock: hex(0x110f16),
        panel: hex(0x1c1924),
        raised: hex(0x241f2d),
        field: hex(0x0f0d14),
        field_border: hex(0x3a3542),
        line: hex(0x26222e),
        hover: hex(0x2a2433),
        pressed: hex(0x342c3f),
        selected: hex(0x3a2434),
        text: hex(0xf2eee9),
        text_dim: hex(0xc9c2bb),
        text_faint: hex(0x8a837d),
        text_off: hex(0x5f5866),
        accent: hex(0xff6ea0),
        accent_strong: hex(0xff8fb6),
        on_accent: hex(0x120d12),
        danger: hex(0xe8584a),
        warning: hex(0xffd166),
    };

    /// The same plates, light, with the accent deepened to read on them.
    pub const LIGHT: Palette = Palette {
        dark: false,
        ground: hex(0xd9d4dd),
        chrome: hex(0xece8ef),
        dock: hex(0xe3dee7),
        panel: hex(0xf6f3f8),
        raised: hex(0xe6e1ea),
        field: hex(0xffffff),
        field_border: hex(0xbdb5c4),
        line: hex(0xd6d0dc),
        hover: hex(0xe0d9e6),
        pressed: hex(0xd3cada),
        selected: hex(0xf7d3e1),
        text: hex(0x1d1922),
        text_dim: hex(0x4c4554),
        text_faint: hex(0x7a7282),
        text_off: hex(0xa59dad),
        accent: hex(0xd8336f),
        accent_strong: hex(0xb81f58),
        on_accent: hex(0xffffff),
        danger: hex(0xc23b2e),
        warning: hex(0xa66f00),
    };

    /// The palette for the system's appearance.
    pub fn for_system(ctx: &egui::Context) -> Palette {
        match ctx.system_theme() {
            Some(egui::Theme::Light) => Palette::LIGHT,
            _ => Palette::DARK,
        }
    }

    /// PhotoCraft's tokens in these colours (its Photoshop-grammar layout kept).
    pub fn photo_tokens(&self) -> photo::Tokens {
        let base = photo::Tokens::for_kind(if self.dark {
            photo::ThemeKind::Pro
        } else {
            photo::ThemeKind::ProLight
        });
        photo::Tokens {
            chrome: self.chrome,
            canvas: self.ground,
            canvas_dot: self.panel,
            dock: self.dock,
            card: self.panel,
            card_border: self.line,
            field: self.field,
            field_border: self.field_border,
            hover: self.hover,
            pressed: self.pressed,
            text: self.text,
            text_dim: self.text_dim,
            text_faint: self.text_faint,
            icon: self.text_dim,
            accent: self.accent,
            accent_soft: self.selected,
            accent_border: self.accent,
            accent_text: self.on_accent,
            separator: self.line,
            primary_bg: self.accent,
            primary_text: self.on_accent,
            danger: self.danger,
            warning: self.warning,
            radius_sm: 4.0,
            radius: 6.0,
            radius_lg: 10.0,
            tab_strip: self.chrome,
            row_selected: self.selected,
            histogram_bg: self.field,
            ..base
        }
    }

    /// VectorCraft's tokens in these colours (its functional colours, guides and selection
    /// marks, kept).
    pub fn vector_tokens(&self) -> vector::Tokens {
        let base = vector::Tokens::for_brightness(if self.dark {
            vector::Brightness::Dark
        } else {
            vector::Brightness::Light
        });
        vector::Tokens {
            app_bar: self.chrome,
            panel: self.panel,
            panel_darker: self.dock,
            input: self.field,
            input_border: self.field_border,
            border: self.line,
            divider: self.line,
            text: self.text,
            text_dim: self.text_dim,
            text_disabled: self.text_off,
            icon: self.text_dim,
            hover: self.hover,
            tool_active: self.selected,
            accent: self.accent,
            accent_strong: self.accent_strong,
            row_selected: self.selected,
            pasteboard: self.ground,
            ruler: self.chrome,
            ruler_tick: self.text_faint,
            button: self.raised,
            radius: 4,
            text_strong: self.text,
            tab_strip: self.chrome,
            button_border: self.field_border,
            flyout_active: self.accent,
            error: self.danger,
            warning: self.warning,
            ..base
        }
    }

    /// egui's visuals in these colours.
    pub fn visuals(&self) -> Visuals {
        let mut v = if self.dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        v.panel_fill = self.chrome;
        v.window_fill = self.panel;
        v.window_stroke = Stroke::new(1.0, self.line);
        v.extreme_bg_color = self.field;
        v.faint_bg_color = self.panel;
        v.code_bg_color = self.field;
        v.override_text_color = Some(self.text);
        v.hyperlink_color = self.accent;
        v.warn_fg_color = self.warning;
        v.error_fg_color = self.danger;
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(6);
        let shadow = Color32::from_black_alpha(if self.dark { 150 } else { 60 });
        v.window_shadow = egui::Shadow {
            offset: [0, 8],
            blur: 28,
            spread: 0,
            color: shadow,
        };
        v.popup_shadow = egui::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: shadow,
        };
        v.selection.bg_fill = self.accent;
        v.selection.stroke = Stroke::new(1.0, self.on_accent);
        v.slider_trailing_fill = true;
        v.handle_shape = egui::style::HandleShape::Circle;
        v.striped = false;
        v.indent_has_left_vline = false;
        let r = CornerRadius::same(4);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = self.panel;
        w.noninteractive.weak_bg_fill = self.panel;
        w.noninteractive.bg_stroke = Stroke::new(1.0, self.line);
        w.noninteractive.fg_stroke = Stroke::new(1.0, self.text_dim);
        w.noninteractive.corner_radius = r;
        for (wv, bg, stroke) in [
            (&mut w.inactive, self.raised, Stroke::NONE),
            (
                &mut w.hovered,
                self.hover,
                Stroke::new(1.0, self.field_border),
            ),
            (&mut w.active, self.pressed, Stroke::new(1.0, self.accent)),
            (&mut w.open, self.hover, Stroke::new(1.0, self.field_border)),
        ] {
            wv.bg_fill = bg;
            wv.weak_bg_fill = bg;
            wv.bg_stroke = stroke;
            wv.fg_stroke = Stroke::new(1.0, self.text);
            wv.corner_radius = r;
            wv.expansion = 0.0;
        }
        v
    }
}

/// The spacing and text styles both editors share: between PhotoCraft's and VectorCraft's own,
/// in the families both bind.
fn style(s: &mut egui::Style) {
    s.text_styles = [
        (TextStyle::Small, FontId::proportional(10.5)),
        (TextStyle::Body, FontId::proportional(12.5)),
        (TextStyle::Button, FontId::proportional(12.5)),
        (
            TextStyle::Heading,
            FontId::new(15.0, FontFamily::Name("semibold".into())),
        ),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ]
    .into();
    s.spacing.item_spacing = egui::vec2(7.0, 5.0);
    s.spacing.button_padding = egui::vec2(8.0, 3.0);
    s.spacing.interact_size = egui::vec2(24.0, 24.0);
    s.spacing.menu_margin = egui::Margin::same(6);
    s.spacing.slider_width = 140.0;
    s.animation_time = 0.08;
}

/// Font families each editor draws in, by name, and the family each is an alias of.
fn aliases() -> [(&'static str, FontFamily); 5] {
    [
        ("ui", FontFamily::Proportional),
        ("ui-semibold", FontFamily::Name("semibold".into())),
        ("mono", FontFamily::Monospace),
        ("semibold", FontFamily::Name("ui-semibold".into())),
        ("medium", FontFamily::Name("ui-semibold".into())),
    ]
}

/// SkinCraft's theme, kept over both editors'.
pub struct Theme {
    palette: Option<Palette>,
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}

impl Theme {
    pub fn new() -> Self {
        Theme { palette: None }
    }

    /// The palette last applied.
    pub fn palette(&self) -> Palette {
        self.palette.unwrap_or(Palette::DARK)
    }

    /// Applies the theme over whatever an editor installed: both editors' tokens, the visuals,
    /// the style and the fonts. Call after the editors' logic, every frame. Returns whether the
    /// fonts are in place (egui takes new fonts on the next pass; until then nothing is drawn,
    /// so no frame shows the editors' own).
    pub fn enforce(&mut self, ctx: &egui::Context) -> bool {
        let palette = Palette::for_system(ctx);
        self.palette = Some(palette);
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new("photocraft-theme"), palette.photo_tokens());
            d.insert_temp(egui::Id::NULL, palette.vector_tokens());
        });
        let visuals = palette.visuals();
        if ctx.global_style().visuals != visuals {
            ctx.set_visuals(visuals);
            ctx.global_style_mut(style);
            // egui picks its style by the OS appearance: both branches get the theme.
            let s = ctx.global_style();
            ctx.set_style_of(egui::Theme::Light, std::sync::Arc::clone(&s));
            ctx.set_style_of(egui::Theme::Dark, s);
        }
        self.fonts(ctx)
    }

    /// PhotoCraft's fonts (Inter, JetBrains Mono, and the script fallbacks its plugins add as
    /// text needs them), with every family name either editor draws in. VectorCraft replaces the
    /// whole table with its own on its first frame: then PhotoCraft's are installed again.
    fn fonts(&mut self, ctx: &egui::Context) -> bool {
        let (photo_faces, aliased) = ctx.fonts(|f| {
            let d = f.definitions();
            (
                d.font_data.contains_key("Inter-Medium"),
                aliases()
                    .iter()
                    .all(|(name, _)| d.families.contains_key(&FontFamily::Name((*name).into()))),
            )
        });
        if !photo_faces {
            photo::install_fonts(ctx);
            ctx.request_repaint();
            return false;
        }
        if aliased {
            return true;
        }
        let mut fonts = ctx.fonts(|f| f.definitions().clone());
        for (name, from) in aliases() {
            let stack = fonts
                .families
                .get(&from)
                .or_else(|| fonts.families.get(&FontFamily::Proportional))
                .cloned()
                .unwrap_or_default();
            fonts
                .families
                .entry(FontFamily::Name(name.into()))
                .or_insert(stack);
        }
        ctx.set_fonts(fonts);
        ctx.request_repaint();
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_editors_read_the_same_colours() {
        for p in [Palette::DARK, Palette::LIGHT] {
            let (photo, vector) = (p.photo_tokens(), p.vector_tokens());
            assert_eq!(photo.chrome, vector.app_bar);
            assert_eq!(photo.card, vector.panel);
            assert_eq!(photo.canvas, vector.pasteboard);
            assert_eq!(photo.accent, vector.accent);
            assert_eq!(photo.text, vector.text);
            assert_eq!(photo.dark(), p.dark);
            assert_eq!(vector.dark, p.dark);
        }
    }
}
