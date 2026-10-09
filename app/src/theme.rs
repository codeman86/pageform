//! Light and dark theme for the window chrome and the canvas overlays.
//!
//! The letter page is a design surface, so its fill follows the theme and the
//! grid marks are chosen to stay readable on that fill. Field fills stay white
//! in both themes. None of these colors are written into the PDF.

use eframe::egui::{self, Color32};

pub const STORAGE_KEY: &str = "pageform-theme";

/// Missing or unknown values open in the dark theme.
pub fn is_dark(stored: Option<&str>) -> bool {
    !matches!(stored, Some("light"))
}

pub fn storage_value(dark: bool) -> &'static str {
    if dark {
        "dark"
    } else {
        "light"
    }
}

pub fn toggle_label(dark: bool) -> &'static str {
    if dark {
        "Dark"
    } else {
        "Light"
    }
}

pub fn toggle_tip(dark: bool) -> &'static str {
    if dark {
        "Dark theme. Click to switch to light."
    } else {
        "Light theme. Click to switch to dark."
    }
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark {
        egui::ThemePreference::Dark
    } else {
        egui::ThemePreference::Light
    });
    ctx.set_visuals(if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    });
}

/// Colors for the page surface and the overlays drawn on it.
#[derive(Clone, Copy)]
pub struct CanvasColors {
    pub desk: Color32,
    pub page: Color32,
    pub shadow: Color32,
    pub page_edge: Color32,
    pub minor_dot: Color32,
    pub major_dot: Color32,
    pub minor_guide: Color32,
    pub major_guide: Color32,
    pub selection: Color32,
    pub text_stroke: Color32,
    pub check_stroke: Color32,
    pub field_fill: Color32,
    pub field_text: Color32,
    pub handle_fill: Color32,
    pub margin: Color32,
}

pub fn canvas_colors(dark: bool) -> CanvasColors {
    if dark {
        CanvasColors {
            desk: Color32::from_rgb(20, 22, 26),
            page: Color32::from_rgb(36, 40, 46),
            shadow: Color32::from_rgba_unmultiplied(0, 0, 0, 110),
            page_edge: Color32::from_rgb(168, 176, 188),
            minor_dot: Color32::from_rgb(164, 172, 184),
            major_dot: Color32::from_rgb(226, 230, 236),
            minor_guide: Color32::from_rgb(86, 116, 140),
            major_guide: Color32::from_rgb(156, 190, 214),
            selection: Color32::from_rgb(120, 176, 255),
            text_stroke: Color32::from_rgb(36, 48, 72),
            check_stroke: Color32::from_rgb(32, 32, 36),
            field_fill: Color32::WHITE,
            field_text: Color32::from_rgb(50, 58, 72),
            handle_fill: Color32::WHITE,
            margin: Color32::from_rgb(220, 146, 146),
        }
    } else {
        // Same canvas colors as Milestone 0's light window.
        CanvasColors {
            desk: Color32::from_rgb(232, 234, 238),
            page: Color32::WHITE,
            shadow: Color32::from_rgba_unmultiplied(0, 0, 0, 28),
            page_edge: Color32::from_rgb(60, 64, 72),
            minor_dot: Color32::from_rgb(186, 192, 202),
            major_dot: Color32::from_rgb(142, 150, 164),
            minor_guide: Color32::from_rgb(214, 228, 236),
            major_guide: Color32::from_rgb(168, 196, 216),
            selection: Color32::from_rgb(25, 102, 204),
            text_stroke: Color32::from_rgb(36, 48, 72),
            check_stroke: Color32::from_rgb(32, 32, 36),
            field_fill: Color32::WHITE,
            field_text: Color32::from_rgb(50, 58, 72),
            handle_fill: Color32::WHITE,
            margin: Color32::from_rgb(186, 112, 112),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel_luma(color: Color32) -> f32 {
        fn lin(channel: u8) -> f32 {
            let s = channel as f32 / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * lin(color.r()) + 0.7152 * lin(color.g()) + 0.0722 * lin(color.b())
    }

    #[test]
    fn missing_theme_defaults_to_dark() {
        assert!(is_dark(None));
        assert!(is_dark(Some("dark")));
        assert!(is_dark(Some("")));
        assert!(is_dark(Some("system")));
        assert!(!is_dark(Some("light")));
        assert_eq!(storage_value(true), "dark");
        assert_eq!(storage_value(false), "light");
        assert_eq!(toggle_label(true), "Dark");
        assert_eq!(toggle_label(false), "Light");
    }

    #[test]
    fn light_canvas_matches_milestone_0_colors() {
        let colors = canvas_colors(false);
        assert_eq!(colors.desk, Color32::from_rgb(232, 234, 238));
        assert_eq!(colors.page, Color32::WHITE);
        assert_eq!(colors.minor_dot, Color32::from_rgb(186, 192, 202));
        assert_eq!(colors.major_dot, Color32::from_rgb(142, 150, 164));
        assert_eq!(colors.minor_guide, Color32::from_rgb(214, 228, 236));
        assert_eq!(colors.major_guide, Color32::from_rgb(168, 196, 216));
        assert_eq!(colors.selection, Color32::from_rgb(25, 102, 204));
        assert_eq!(colors.margin, Color32::from_rgb(186, 112, 112));
    }

    #[test]
    fn dark_canvas_marks_are_lighter_than_the_page() {
        let colors = canvas_colors(true);
        let page = rel_luma(colors.page);
        assert!(rel_luma(colors.minor_dot) > page + 0.15);
        assert!(rel_luma(colors.major_dot) > rel_luma(colors.minor_dot));
        assert!(rel_luma(colors.minor_guide) > page + 0.05);
        assert!(rel_luma(colors.major_guide) > rel_luma(colors.minor_guide));
        assert!(rel_luma(colors.page_edge) > page);
        assert!(rel_luma(colors.margin) > page);
        assert!(rel_luma(colors.desk) < page);
    }

    #[test]
    fn light_canvas_marks_are_darker_than_the_page() {
        let colors = canvas_colors(false);
        let page = rel_luma(colors.page);
        assert!(rel_luma(colors.minor_dot) < page - 0.15);
        assert!(rel_luma(colors.major_dot) < rel_luma(colors.minor_dot));
        assert!(rel_luma(colors.minor_guide) < page - 0.05);
        assert!(rel_luma(colors.major_guide) < rel_luma(colors.minor_guide));
        assert!(rel_luma(colors.page_edge) < page);
        assert!(rel_luma(colors.margin) < page);
    }
}
