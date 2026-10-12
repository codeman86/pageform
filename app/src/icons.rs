//! Small toolbar glyphs drawn with egui's painter.
//!
//! No icon font and no image pack. Each glyph is a few strokes in a unit box
//! so it stays readable in the light and dark themes.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

/// Document commands, left to right.
pub const DOC_COMMANDS: &[&str] = &["New", "Open", "Save", "Print"];

/// Insert tools, alphabetical. Square/Rectangle lives under Shapes.
pub const INSERT_TOOLS: &[&str] = &[
    "Checkbox",
    "Date",
    "Drop-down",
    "Image",
    "Line",
    "Radio",
    "Shapes",
    "Signature",
    "Table",
    "Text",
    "Text Box",
    "Text Field",
];

/// View toggles on the toolbar, alphabetical. Header and footer stay in the View menu.
pub const VIEW_TOGGLES: &[&str] = &["Dots", "Guides", "Margins", "Page numbers", "Snap"];

pub const VIEW_ICONS: [Icon; 5] = [
    Icon::Dots,
    Icon::Guides,
    Icon::Margins,
    Icon::PageNumber,
    Icon::Snap,
];

const BUTTON: f32 = 26.0;
const MENU_BUTTON: f32 = 36.0;

#[derive(Clone, Copy)]
pub enum Icon {
    New,
    Open,
    Save,
    Print,
    Select,
    InsertPage,
    Checkbox,
    Date,
    Dropdown,
    Image,
    Line,
    Radio,
    Shapes,
    Signature,
    Table,
    Text,
    TextBox,
    TextField,
    Grid,
    PageNumber,
    Dots,
    Guides,
    Margins,
    Snap,
    Moon,
    Sun,
}

pub fn icon_button(ui: &mut egui::Ui, icon: Icon, selected: bool, tip: &str) -> egui::Response {
    icon_widget(ui, icon, selected, false, tip)
}

pub fn icon_menu(ui: &mut egui::Ui, icon: Icon, selected: bool, tip: &str) -> egui::Response {
    icon_widget(ui, icon, selected, true, tip)
}

fn icon_widget(
    ui: &mut egui::Ui,
    icon: Icon,
    selected: bool,
    caret: bool,
    tip: &str,
) -> egui::Response {
    let width = if caret { MENU_BUTTON } else { BUTTON };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, BUTTON), Sense::click());
    let visuals = ui.style().interact_selectable(&response, selected);
    if selected || response.hovered() {
        ui.painter().rect_filled(rect, 4.0, visuals.bg_fill);
    }
    if selected {
        ui.painter()
            .rect_stroke(rect, 4.0, visuals.bg_stroke, StrokeKind::Inside);
    }
    let color = visuals.fg_stroke.color;
    let glyph = if caret {
        Rect::from_min_size(rect.min + Vec2::new(4.0, 4.0), Vec2::splat(18.0))
    } else {
        Rect::from_center_size(rect.center(), Vec2::splat(16.0))
    };
    paint(ui.painter(), glyph, color, icon);
    if caret {
        let caret_box = Rect::from_center_size(
            Pos2::new(rect.right() - 8.0, rect.center().y + 1.0),
            Vec2::new(7.0, 5.0),
        );
        paint_caret(ui.painter(), caret_box, color);
    }
    response.on_hover_text(tip)
}

fn paint(painter: &egui::Painter, rect: Rect, color: Color32, icon: Icon) {
    let ink = Ink {
        painter,
        rect,
        color,
    };
    match icon {
        Icon::New => ink.page_plus(0.78),
        Icon::Open => ink.folder(),
        Icon::Save => ink.floppy(),
        Icon::Print => ink.printer(),
        Icon::Select => ink.pointer(),
        Icon::InsertPage => ink.page_plus(0.62),
        Icon::Checkbox => ink.checkbox(),
        Icon::Date => ink.calendar(),
        Icon::Dropdown => ink.dropdown(),
        Icon::Image => ink.picture(),
        Icon::Line => ink.line((0.08, 0.78), (0.92, 0.22)),
        Icon::Radio => ink.radio(),
        Icon::Shapes => ink.shapes(),
        Icon::Signature => ink.signature(),
        Icon::Table => ink.table(),
        Icon::Text => ink.letter_t(),
        Icon::TextBox => ink.text_box(),
        Icon::TextField => ink.text_field(),
        Icon::Grid => ink.grid(),
        Icon::PageNumber => ink.page_number(),
        Icon::Dots => ink.dots(),
        Icon::Guides => ink.guides(),
        Icon::Margins => ink.margins(),
        Icon::Snap => ink.snap(),
        Icon::Moon => ink.moon(),
        Icon::Sun => ink.sun(),
    }
}

struct Ink<'a> {
    painter: &'a egui::Painter,
    rect: Rect,
    color: Color32,
}

impl Ink<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        Pos2::new(
            self.rect.left() + self.rect.width() * x,
            self.rect.top() + self.rect.height() * y,
        )
    }

    fn stroke(&self) -> Stroke {
        Stroke::new(1.4_f32, self.color)
    }

    fn line(&self, a: (f32, f32), b: (f32, f32)) {
        self.painter
            .line_segment([self.at(a.0, a.1), self.at(b.0, b.1)], self.stroke());
    }

    fn poly(&self, points: &[(f32, f32)]) {
        for pair in points.windows(2) {
            self.line(pair[0], pair[1]);
        }
    }

    fn frame(&self, x: f32, y: f32, w: f32, h: f32) {
        let rect = Rect::from_min_max(self.at(x, y), self.at(x + w, y + h));
        self.painter
            .rect_stroke(rect, 0.0, self.stroke(), StrokeKind::Inside);
    }

    fn dot(&self, x: f32, y: f32, radius: f32) {
        self.painter
            .circle_filled(self.at(x, y), radius, self.color);
    }

    fn dash(&self, a: (f32, f32), b: (f32, f32)) {
        let start = self.at(a.0, a.1);
        let end = self.at(b.0, b.1);
        let delta = end - start;
        let len = delta.length();
        if len < 1.0 {
            return;
        }
        let dir = delta / len;
        let mut traveled = 0.0;
        while traveled < len {
            let stop = (traveled + 3.0).min(len);
            self.painter
                .line_segment([start + dir * traveled, start + dir * stop], self.stroke());
            traveled += 5.0;
        }
    }

    fn page_plus(&self, plus_at: f32) {
        self.frame(0.08, 0.06, 0.62, 0.88);
        self.line((0.52, 0.06), (0.70, 0.24));
        self.line((0.52, 0.06), (0.52, 0.24));
        self.line((0.52, 0.24), (0.70, 0.24));
        self.line((plus_at - 0.16, plus_at), (plus_at + 0.16, plus_at));
        self.line((plus_at, plus_at - 0.16), (plus_at, plus_at + 0.16));
    }

    fn folder(&self) {
        self.poly(&[
            (0.08, 0.28),
            (0.08, 0.84),
            (0.92, 0.84),
            (0.92, 0.36),
            (0.46, 0.36),
            (0.36, 0.18),
            (0.08, 0.18),
            (0.08, 0.28),
        ]);
    }

    fn floppy(&self) {
        self.frame(0.14, 0.08, 0.72, 0.84);
        self.frame(0.30, 0.08, 0.40, 0.28);
        self.frame(0.28, 0.52, 0.44, 0.40);
    }

    fn printer(&self) {
        self.frame(0.28, 0.04, 0.44, 0.24);
        self.frame(0.08, 0.30, 0.84, 0.36);
        self.frame(0.24, 0.58, 0.52, 0.36);
        self.line((0.18, 0.42), (0.32, 0.42));
    }

    fn pointer(&self) {
        self.poly(&[
            (0.18, 0.06),
            (0.18, 0.90),
            (0.40, 0.66),
            (0.56, 0.84),
            (0.70, 0.72),
            (0.52, 0.54),
            (0.78, 0.50),
            (0.18, 0.06),
        ]);
    }

    fn checkbox(&self) {
        self.frame(0.12, 0.12, 0.76, 0.76);
        self.poly(&[(0.28, 0.52), (0.44, 0.70), (0.76, 0.30)]);
    }

    fn calendar(&self) {
        self.frame(0.10, 0.16, 0.80, 0.74);
        self.line((0.10, 0.36), (0.90, 0.36));
        self.line((0.30, 0.08), (0.30, 0.24));
        self.line((0.70, 0.08), (0.70, 0.24));
    }

    fn dropdown(&self) {
        self.frame(0.06, 0.28, 0.88, 0.44);
        self.line((0.68, 0.28), (0.68, 0.72));
        self.poly(&[(0.74, 0.42), (0.82, 0.56), (0.90, 0.42)]);
    }

    fn picture(&self) {
        self.frame(0.08, 0.16, 0.84, 0.68);
        self.painter
            .circle_stroke(self.at(0.32, 0.36), self.rect.width() * 0.08, self.stroke());
        self.poly(&[(0.16, 0.72), (0.40, 0.48), (0.58, 0.64), (0.84, 0.40)]);
    }

    fn radio(&self) {
        let center = self.at(0.5, 0.5);
        let radius = self.rect.width() * 0.36;
        self.painter.circle_stroke(center, radius, self.stroke());
        self.painter
            .circle_filled(center, radius * 0.38, self.color);
    }

    fn shapes(&self) {
        self.frame(0.06, 0.28, 0.58, 0.62);
        self.frame(0.36, 0.08, 0.56, 0.50);
    }

    fn signature(&self) {
        self.poly(&[
            (0.06, 0.62),
            (0.22, 0.38),
            (0.36, 0.70),
            (0.52, 0.30),
            (0.68, 0.66),
            (0.94, 0.28),
        ]);
    }

    fn table(&self) {
        self.frame(0.08, 0.12, 0.84, 0.76);
        self.line((0.08, 0.50), (0.92, 0.50));
        self.line((0.50, 0.12), (0.50, 0.88));
    }

    fn letter_t(&self) {
        self.line((0.16, 0.16), (0.84, 0.16));
        self.line((0.50, 0.16), (0.50, 0.88));
    }

    fn text_box(&self) {
        self.frame(0.08, 0.12, 0.84, 0.76);
        self.line((0.20, 0.32), (0.80, 0.32));
        self.line((0.20, 0.50), (0.80, 0.50));
        self.line((0.20, 0.68), (0.62, 0.68));
    }

    fn text_field(&self) {
        self.frame(0.06, 0.28, 0.88, 0.44);
        self.line((0.22, 0.38), (0.22, 0.62));
    }

    fn grid(&self) {
        for step in [0.18, 0.50, 0.82] {
            self.line((step, 0.10), (step, 0.90));
            self.line((0.10, step), (0.90, step));
        }
    }

    fn page_number(&self) {
        self.frame(0.16, 0.06, 0.68, 0.88);
        self.line((0.42, 0.28), (0.56, 0.18));
        self.line((0.56, 0.18), (0.56, 0.74));
        self.line((0.42, 0.74), (0.70, 0.74));
    }

    fn dots(&self) {
        for x in [0.22, 0.50, 0.78] {
            for y in [0.22, 0.50, 0.78] {
                self.dot(x, y, 1.5);
            }
        }
    }

    fn guides(&self) {
        self.dash((0.50, 0.06), (0.50, 0.94));
        self.dash((0.06, 0.50), (0.94, 0.50));
    }

    fn margins(&self) {
        self.frame(0.08, 0.08, 0.84, 0.84);
        self.dash((0.28, 0.28), (0.72, 0.28));
        self.dash((0.72, 0.28), (0.72, 0.72));
        self.dash((0.72, 0.72), (0.28, 0.72));
        self.dash((0.28, 0.72), (0.28, 0.28));
    }

    fn snap(&self) {
        self.line((0.50, 0.08), (0.50, 0.92));
        self.line((0.08, 0.50), (0.92, 0.50));
        self.dot(0.50, 0.50, 2.4);
    }

    fn moon(&self) {
        self.poly(&[
            (0.62, 0.12),
            (0.40, 0.18),
            (0.24, 0.40),
            (0.24, 0.64),
            (0.42, 0.88),
            (0.68, 0.84),
            (0.48, 0.68),
            (0.44, 0.48),
            (0.54, 0.28),
            (0.62, 0.12),
        ]);
    }

    fn sun(&self) {
        self.painter
            .circle_stroke(self.at(0.50, 0.50), self.rect.width() * 0.22, self.stroke());
        for (x0, y0, x1, y1) in [
            (0.50, 0.06, 0.50, 0.20),
            (0.50, 0.80, 0.50, 0.94),
            (0.06, 0.50, 0.20, 0.50),
            (0.80, 0.50, 0.94, 0.50),
            (0.18, 0.18, 0.28, 0.28),
            (0.72, 0.72, 0.82, 0.82),
            (0.18, 0.82, 0.28, 0.72),
            (0.72, 0.28, 0.82, 0.18),
        ] {
            self.line((x0, y0), (x1, y1));
        }
    }
}

fn paint_caret(painter: &egui::Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(1.4_f32, color);
    painter.line_segment([rect.left_top(), rect.center_bottom()], stroke);
    painter.line_segment([rect.right_top(), rect.center_bottom()], stroke);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_groups_are_alphabetical_and_omit_removed_items() {
        assert_sorted(INSERT_TOOLS);
        assert_sorted(VIEW_TOGGLES);
        assert_eq!(
            INSERT_TOOLS,
            [
                "Checkbox",
                "Date",
                "Drop-down",
                "Image",
                "Line",
                "Radio",
                "Shapes",
                "Signature",
                "Table",
                "Text",
                "Text Box",
                "Text Field",
            ]
        );
        assert!(!INSERT_TOOLS.contains(&"Square/Rectangle"));
        assert!(!INSERT_TOOLS.contains(&"Header"));
        assert!(!INSERT_TOOLS.contains(&"Footer"));
        assert!(!VIEW_TOGGLES.contains(&"Header"));
        assert!(!VIEW_TOGGLES.contains(&"Footer"));
        assert_eq!(DOC_COMMANDS, ["New", "Open", "Save", "Print"]);
        assert_eq!(
            VIEW_TOGGLES,
            ["Dots", "Guides", "Margins", "Page numbers", "Snap"]
        );
        assert_eq!(VIEW_TOGGLES.len(), VIEW_ICONS.len());
    }

    fn assert_sorted(labels: &[&str]) {
        let mut sorted = labels.to_vec();
        sorted.sort_unstable();
        assert_eq!(labels, sorted.as_slice());
    }
}
