//! Appearance and AcroForm values edited from the properties panel.
//!
//! Defaults match the Milestone 0 export: Helvetica 12, black text, a 1 pt
//! solid black border, and a white fill on widgets. Lines and rectangles
//! have no fill. Static text has no border until one is set.

use crate::document::DATE_HINT;
use crate::FieldKind;

/// RGB in 0–1, the same range PDF color operators use.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Rgb {
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
    };
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
    };

    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
        }
    }

    pub fn is_black(self) -> bool {
        self.r <= 0.001 && self.g <= 0.001 && self.b <= 0.001
    }
}

/// `/Q` quadding for variable text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl TextAlign {
    pub const ALL: [TextAlign; 3] = [TextAlign::Left, TextAlign::Center, TextAlign::Right];

    pub fn label(self) -> &'static str {
        match self {
            TextAlign::Left => "Left",
            TextAlign::Center => "Center",
            TextAlign::Right => "Right",
        }
    }
}

/// Standard fonts that a PDF can name without embedding a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum FontFamily {
    #[default]
    Helvetica,
    Times,
    Courier,
}

impl FontFamily {
    pub const ALL: [FontFamily; 3] = [
        FontFamily::Helvetica,
        FontFamily::Times,
        FontFamily::Courier,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FontFamily::Helvetica => "Helvetica",
            FontFamily::Times => "Times",
            FontFamily::Courier => "Courier",
        }
    }

    /// Resource key used in `/DA` and `/DR`.
    pub fn pdf_key(self) -> &'static [u8] {
        match self {
            FontFamily::Helvetica => b"Helv",
            FontFamily::Times => b"TiRo",
            FontFamily::Courier => b"Cour",
        }
    }

    pub fn pdf_key_str(self) -> &'static str {
        match self {
            FontFamily::Helvetica => "Helv",
            FontFamily::Times => "TiRo",
            FontFamily::Courier => "Cour",
        }
    }

    pub fn base_font(self) -> &'static [u8] {
        match self {
            FontFamily::Helvetica => b"Helvetica",
            FontFamily::Times => b"Times-Roman",
            FontFamily::Courier => b"Courier",
        }
    }
}

/// `/BS /S` border style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum BorderStyleKind {
    #[default]
    Solid,
    Dashed,
    Beveled,
    Inset,
    Underline,
}

impl BorderStyleKind {
    pub const ALL: [BorderStyleKind; 5] = [
        BorderStyleKind::Solid,
        BorderStyleKind::Dashed,
        BorderStyleKind::Beveled,
        BorderStyleKind::Inset,
        BorderStyleKind::Underline,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BorderStyleKind::Solid => "Solid",
            BorderStyleKind::Dashed => "Dashed",
            BorderStyleKind::Beveled => "Beveled",
            BorderStyleKind::Inset => "Inset",
            BorderStyleKind::Underline => "Underline",
        }
    }
}

/// Checkbox mark. Exported as `/MK /CA` (ZapfDingbats) and drawn in `/AP`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum CheckboxStyle {
    #[default]
    Check,
    Cross,
    Circle,
    Diamond,
    Square,
    Star,
}

impl CheckboxStyle {
    pub const ALL: [CheckboxStyle; 6] = [
        CheckboxStyle::Check,
        CheckboxStyle::Cross,
        CheckboxStyle::Circle,
        CheckboxStyle::Diamond,
        CheckboxStyle::Square,
        CheckboxStyle::Star,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CheckboxStyle::Check => "Check",
            CheckboxStyle::Cross => "Cross",
            CheckboxStyle::Circle => "Circle",
            CheckboxStyle::Diamond => "Diamond",
            CheckboxStyle::Square => "Square",
            CheckboxStyle::Star => "Star",
        }
    }

    /// ZapfDingbats character viewers use when they read `/MK /CA`.
    pub fn zapf(self) -> &'static str {
        match self {
            CheckboxStyle::Check => "4",
            CheckboxStyle::Cross => "8",
            CheckboxStyle::Circle => "l",
            CheckboxStyle::Diamond => "u",
            CheckboxStyle::Square => "n",
            CheckboxStyle::Star => "H",
        }
    }
}

/// Paper sizes the properties panel can apply. Portrait point sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum PageSize {
    #[default]
    Letter,
    Legal,
    A4,
}

impl PageSize {
    pub const ALL: [PageSize; 3] = [PageSize::Letter, PageSize::Legal, PageSize::A4];

    pub fn label(self) -> &'static str {
        match self {
            PageSize::Letter => "US Letter",
            PageSize::Legal => "US Legal",
            PageSize::A4 => "A4",
        }
    }

    /// Width and height in points, portrait.
    pub fn portrait_pt(self) -> (f32, f32) {
        match self {
            PageSize::Letter => (612.0, 792.0),
            PageSize::Legal => (612.0, 1008.0),
            PageSize::A4 => (595.0, 842.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

impl Orientation {
    pub fn label(self) -> &'static str {
        match self {
            Orientation::Portrait => "Portrait",
            Orientation::Landscape => "Landscape",
        }
    }
}

/// Canvas print-safe inset. Not written into the PDF.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Margins {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            top: 36.0,
            right: 36.0,
            bottom: 36.0,
            left: 36.0,
        }
    }
}

/// Values the properties panel edits on one item.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FieldStyle {
    pub tooltip: String,
    pub required: bool,
    pub read_only: bool,
    pub default_value: String,
    pub max_chars: Option<i32>,
    pub multiline: bool,
    pub align: TextAlign,
    pub font: FontFamily,
    pub font_size: f32,
    pub text_color: Rgb,
    pub border_width: f32,
    pub border_color: Rgb,
    pub border_style: BorderStyleKind,
    pub fill: Option<Rgb>,
    pub stroke_width: f32,
    pub stroke_color: Rgb,
    pub checkbox_style: CheckboxStyle,
    pub date_format: String,
}

impl Default for FieldStyle {
    fn default() -> Self {
        Self {
            tooltip: String::new(),
            required: false,
            read_only: false,
            default_value: String::new(),
            max_chars: None,
            multiline: false,
            align: TextAlign::Left,
            font: FontFamily::Helvetica,
            font_size: 12.0,
            text_color: Rgb::BLACK,
            border_width: 1.0,
            border_color: Rgb::BLACK,
            border_style: BorderStyleKind::Solid,
            fill: Some(Rgb::WHITE),
            stroke_width: 1.0,
            stroke_color: Rgb::BLACK,
            checkbox_style: CheckboxStyle::Check,
            date_format: DATE_HINT.to_string(),
        }
    }
}

impl FieldStyle {
    pub fn for_kind(kind: FieldKind) -> Self {
        let mut style = Self::default();
        match kind {
            FieldKind::Date => style.max_chars = Some(10),
            FieldKind::TextBox => style.multiline = true,
            FieldKind::StaticText => {
                style.border_width = 0.0;
                style.fill = None;
            }
            FieldKind::Line => {
                style.fill = None;
                style.stroke_width = 1.5;
            }
            FieldKind::Rectangle => style.fill = None,
            FieldKind::Text
            | FieldKind::Checkbox
            | FieldKind::Radio
            | FieldKind::Table
            | FieldKind::Signature
            | FieldKind::Dropdown
            | FieldKind::Image => {}
        }
        style
    }

    pub fn checked(&self) -> bool {
        matches!(
            self.default_value.to_ascii_lowercase().as_str(),
            "yes" | "on" | "true"
        )
    }
}

/// Drop characters that would not survive a PDF literal string.
pub fn clean_text(text: &str, allow_newline: bool) -> String {
    text.chars()
        .filter(|ch| !ch.is_control() || (allow_newline && *ch == '\n'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_original_widget_look() {
        let text = FieldStyle::for_kind(FieldKind::Text);
        assert_eq!(text.font, FontFamily::Helvetica);
        assert!((text.font_size - 12.0).abs() < 0.01);
        assert!(text.text_color.is_black());
        assert!((text.border_width - 1.0).abs() < 0.01);
        assert_eq!(text.border_style, BorderStyleKind::Solid);
        assert_eq!(text.fill, Some(Rgb::WHITE));
        assert!(text.tooltip.is_empty());
        assert!(!text.required && !text.read_only);
        assert_eq!(text.align, TextAlign::Left);
        assert!(text.max_chars.is_none());

        let date = FieldStyle::for_kind(FieldKind::Date);
        assert_eq!(date.max_chars, Some(10));
        assert_eq!(date.date_format, DATE_HINT);

        let label = FieldStyle::for_kind(FieldKind::StaticText);
        assert!(label.fill.is_none());
        assert!(label.border_width < 0.01);
        let line = FieldStyle::for_kind(FieldKind::Line);
        assert!(line.fill.is_none());
        assert!((line.stroke_width - 1.5).abs() < 0.01);
        let shape = FieldStyle::for_kind(FieldKind::Rectangle);
        assert!(shape.fill.is_none());
        assert!((shape.stroke_width - 1.0).abs() < 0.01);
        assert!(FieldStyle::for_kind(FieldKind::TextBox).multiline);
    }

    #[test]
    fn standard_font_keys_and_checkbox_marks() {
        assert_eq!(FontFamily::Helvetica.pdf_key(), b"Helv");
        assert_eq!(FontFamily::Times.base_font(), b"Times-Roman");
        assert_eq!(FontFamily::Courier.pdf_key_str(), "Cour");
        assert_eq!(CheckboxStyle::Check.zapf(), "4");
        assert_eq!(CheckboxStyle::Cross.zapf(), "8");
        assert!(!FieldStyle::default().checked());
        let style = FieldStyle {
            default_value: "Yes".to_string(),
            ..FieldStyle::default()
        };
        assert!(style.checked());
    }
}
