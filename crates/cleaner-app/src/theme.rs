//! The dark indigo application theme. These color
//! constants are the single source of truth: the egui style and every custom
//! surface reference them so the look stays in sync.

use eframe::egui::{self, Color32};

pub(crate) const BACKGROUND: Color32 = Color32::from_rgb(0x0f, 0x11, 0x15);
pub(crate) const SURFACE: Color32 = Color32::from_rgb(0x18, 0x1b, 0x22);
pub(crate) const SURFACE_RAISED: Color32 = Color32::from_rgb(0x1f, 0x23, 0x2c);
pub(crate) const SURFACE_SUNKEN: Color32 = Color32::from_rgb(0x16, 0x19, 0x22);
pub(crate) const BORDER: Color32 = Color32::from_rgb(0x2a, 0x2f, 0x3a);
pub(crate) const HOVER: Color32 = Color32::from_rgb(0x25, 0x2b, 0x36);
pub(crate) const PRESSED: Color32 = Color32::from_rgb(0x2f, 0x36, 0x43);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(0x63, 0x66, 0xf1);
pub(crate) const SELECTION: Color32 = Color32::from_rgba_premultiplied(0x21, 0x22, 0x50, 0x55);
pub(crate) const ROW_ALT: Color32 = Color32::from_rgba_premultiplied(0x0c, 0x0c, 0x0c, 0x0c);
pub(crate) const TEXT: Color32 = Color32::from_rgb(0xe6, 0xe9, 0xef);
pub(crate) const MUTED: Color32 = Color32::from_rgb(0x8b, 0x93, 0xa3);
pub(crate) const DANGER: Color32 = Color32::from_rgb(0xff, 0x5c, 0x5c);

/// Installs the Inter fonts and the dark indigo style on the egui context.
pub(crate) fn apply(ctx: &egui::Context) {
    install_fonts(ctx);

    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(style);
}

fn style(style: &mut egui::Style) {
    let visuals = &mut style.visuals;
    *visuals = egui::Visuals::dark();

    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = SURFACE;
    visuals.window_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.extreme_bg_color = SURFACE_SUNKEN;
    visuals.faint_bg_color = ROW_ALT;
    visuals.selection.bg_fill = SELECTION;
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;

    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.bg_fill = SURFACE_RAISED;
    visuals.widgets.inactive.weak_bg_fill = SURFACE_RAISED;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.bg_fill = HOVER;
    visuals.widgets.hovered.weak_bg_fill = HOVER;
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, TEXT);
    visuals.widgets.active.bg_fill = PRESSED;
    visuals.widgets.active.weak_bg_fill = PRESSED;
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.5, TEXT);
    visuals.widgets.open.bg_fill = SURFACE_RAISED;
    visuals.widgets.open.weak_bg_fill = SURFACE_RAISED;

    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "inter".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/Inter-Regular.ttf")).into(),
    );
    fonts.font_data.insert(
        "inter-bold".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/Inter-Bold.ttf")).into(),
    );
    fonts.font_data.insert(
        "phosphor".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/Phosphor.ttf")).into(),
    );
    let proportional = fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default();
    proportional.insert(0, "inter".to_owned());
    // Phosphor is a fallback so glyphs render inside proportional text runs.
    proportional.insert(1, "phosphor".to_owned());
    fonts.families.insert(
        egui::FontFamily::Name("bold".into()),
        vec!["inter-bold".to_owned(), "phosphor".to_owned()],
    );
    ctx.set_fonts(fonts);
}

/// The bold Inter family for headings and emphasized values.
pub(crate) fn bold() -> egui::FontFamily {
    egui::FontFamily::Name("bold".into())
}

fn clamp01(t: f64) -> f64 {
    t.clamp(0.0, 1.0)
}

/// Maps a byte size to a white→red tone on a log scale, so larger targets
/// read "hotter" and are easy to spot. Zero bytes render muted.
#[must_use]
pub(crate) fn magnitude_color(bytes: u64) -> Color32 {
    // ~1 MiB (text tone) → ~8 GiB (red).
    const LO: f64 = 20.0;
    const HI: f64 = 33.0;
    if bytes == 0 {
        return MUTED;
    }
    #[expect(clippy::cast_precision_loss, reason = "display-only approximation")]
    let t = clamp01(((bytes as f64).log2() - LO) / (HI - LO));
    let lerp = |a: u8, b: u8| -> u8 {
        let value = f64::from(a) + (f64::from(b) - f64::from(a)) * t;
        #[expect(clippy::cast_possible_truncation, reason = "clamped to [a, b]")]
        #[expect(clippy::cast_sign_loss, reason = "clamped to [a, b]")]
        {
            value.round() as u8
        }
    };
    Color32::from_rgb(
        lerp(TEXT.r(), 0xff),
        lerp(TEXT.g(), 0x5c),
        lerp(TEXT.b(), 0x5c),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[expect(clippy::float_cmp, reason = "clamp returns its exact bounds")]
    fn clamp01_bounds() {
        assert_eq!(clamp01(-1.0), 0.0);
        assert_eq!(clamp01(0.5), 0.5);
        assert_eq!(clamp01(2.0), 1.0);
    }

    #[test]
    fn magnitude_color_scales_toward_red() {
        assert_eq!(magnitude_color(0), MUTED);
        // Small sizes stay at the text tone.
        assert_eq!(magnitude_color(1024), TEXT);
        // Huge sizes saturate to the hot tone.
        let hot = magnitude_color(16 * 1024 * 1024 * 1024 * 1024);
        assert_eq!(hot, Color32::from_rgb(0xff, 0x5c, 0x5c));
        // Mid sizes are strictly between.
        let mid = magnitude_color(512 * 1024 * 1024);
        assert!(mid.r() > TEXT.r());
        assert!(mid.r() < 0xff);
    }
}
