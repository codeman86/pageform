//! Colored toolbar glyphs drawn with egui's painter.
//!
//! These are original geometric icons: filled shapes, dark outlines, and a
//! small fixed palette. They are not copied from another application's art,
//! and the repo does not include an icon pack.

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

const BUTTON: f32 = 28.0;
const MENU_BUTTON: f32 = 40.0;

const OUTLINE: Color32 = Color32::from_rgb(36, 40, 48);
const PAPER: Color32 = Color32::from_rgb(252, 249, 242);
const PAPER_EDGE: Color32 = Color32::from_rgb(186, 178, 164);
const FOLDER: Color32 = Color32::from_rgb(232, 176, 52);
const FOLDER_DARK: Color32 = Color32::from_rgb(186, 128, 28);
const BLUE: Color32 = Color32::from_rgb(36, 112, 204);
const BLUE_DARK: Color32 = Color32::from_rgb(22, 72, 148);
const GREEN: Color32 = Color32::from_rgb(46, 150, 72);
const RED: Color32 = Color32::from_rgb(196, 58, 52);
const GOLD: Color32 = Color32::from_rgb(240, 186, 48);
const SLATE: Color32 = Color32::from_rgb(132, 142, 156);
const SLATE_DARK: Color32 = Color32::from_rgb(78, 86, 98);
const WHITE: Color32 = Color32::from_rgb(255, 255, 255);
const NAVY: Color32 = Color32::from_rgb(28, 36, 58);

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
    let glyph = if caret {
        Rect::from_min_size(rect.min + Vec2::new(3.0, 4.0), Vec2::splat(20.0))
    } else {
        Rect::from_center_size(rect.center(), Vec2::splat(20.0))
    };
    paint(ui.painter(), glyph, icon);
    if caret {
        let caret_box = Rect::from_center_size(
            Pos2::new(rect.right() - 8.0, rect.center().y + 1.0),
            Vec2::new(8.0, 6.0),
        );
        paint_caret(ui.painter(), caret_box, visuals.fg_stroke.color);
    }
    response.on_hover_text(tip)
}

fn paint(painter: &egui::Painter, rect: Rect, icon: Icon) {
    let ink = Ink { painter, rect };
    match icon {
        Icon::New => ink.new_doc(),
        Icon::Open => ink.folder(),
        Icon::Save => ink.floppy(),
        Icon::Print => ink.printer(),
        Icon::Select => ink.pointer(),
        Icon::InsertPage => ink.insert_page(),
        Icon::Checkbox => ink.checkbox(),
        Icon::Date => ink.calendar(),
        Icon::Dropdown => ink.dropdown(),
        Icon::Image => ink.picture(),
        Icon::Line => ink.rule(),
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
}

impl Ink<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        Pos2::new(
            self.rect.left() + self.rect.width() * x,
            self.rect.top() + self.rect.height() * y,
        )
    }

    fn box_at(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_max(self.at(x, y), self.at(x + w, y + h))
    }

    fn fill(&self, x: f32, y: f32, w: f32, h: f32, color: Color32) {
        self.painter
            .rect_filled(self.box_at(x, y, w, h), 1.0, color);
    }

    fn stroke_box(&self, x: f32, y: f32, w: f32, h: f32) {
        self.painter.rect_stroke(
            self.box_at(x, y, w, h),
            1.0,
            Stroke::new(1.35_f32, OUTLINE),
            StrokeKind::Inside,
        );
    }

    fn disk(&self, x: f32, y: f32, radius: f32, color: Color32) {
        self.painter
            .circle_filled(self.at(x, y), self.rect.width() * radius, color);
    }

    fn ring(&self, x: f32, y: f32, radius: f32) {
        self.painter.circle_stroke(
            self.at(x, y),
            self.rect.width() * radius,
            Stroke::new(1.35_f32, OUTLINE),
        );
    }

    fn poly(&self, points: &[(f32, f32)], fill: Color32) {
        let mapped = points
            .iter()
            .map(|(x, y)| self.at(*x, *y))
            .collect::<Vec<_>>();
        self.painter.add(egui::Shape::convex_polygon(
            mapped,
            fill,
            Stroke::new(1.2_f32, OUTLINE),
        ));
    }

    fn thick(&self, a: (f32, f32), b: (f32, f32), color: Color32, width: f32) {
        self.painter.line_segment(
            [self.at(a.0, a.1), self.at(b.0, b.1)],
            Stroke::new(width, color),
        );
    }

    fn page(&self, x: f32, y: f32, w: f32, h: f32) {
        self.fill(x + 0.04, y + 0.04, w, h, PAPER_EDGE);
        self.fill(x, y, w, h, PAPER);
        let fold = 0.22 * w;
        self.poly(
            &[
                (x + w - fold, y),
                (x + w, y + fold),
                (x + w - fold, y + fold),
            ],
            PAPER_EDGE,
        );
        self.stroke_box(x, y, w, h);
    }

    fn new_doc(&self) {
        self.page(0.02, 0.06, 0.62, 0.78);
        self.disk(0.74, 0.72, 0.22, GREEN);
        self.ring(0.74, 0.72, 0.22);
        self.thick((0.74, 0.58), (0.74, 0.86), WHITE, 2.2);
        self.thick((0.60, 0.72), (0.88, 0.72), WHITE, 2.2);
    }

    fn folder(&self) {
        self.fill(0.12, 0.18, 0.42, 0.18, FOLDER_DARK);
        self.stroke_box(0.12, 0.18, 0.42, 0.18);
        self.fill(0.06, 0.32, 0.88, 0.58, FOLDER);
        self.stroke_box(0.06, 0.32, 0.88, 0.58);
        self.fill(0.10, 0.40, 0.80, 0.08, FOLDER_DARK);
    }

    fn floppy(&self) {
        self.fill(0.12, 0.08, 0.76, 0.84, BLUE);
        self.stroke_box(0.12, 0.08, 0.76, 0.84);
        self.fill(0.30, 0.08, 0.40, 0.26, SLATE_DARK);
        self.fill(0.36, 0.12, 0.10, 0.12, SLATE);
        self.fill(0.24, 0.50, 0.52, 0.36, WHITE);
        self.stroke_box(0.24, 0.50, 0.52, 0.36);
        self.thick((0.34, 0.60), (0.66, 0.60), BLUE, 1.6);
        self.thick((0.34, 0.70), (0.66, 0.70), BLUE, 1.6);
    }

    fn printer(&self) {
        self.fill(0.28, 0.04, 0.44, 0.22, WHITE);
        self.stroke_box(0.28, 0.04, 0.44, 0.22);
        self.fill(0.06, 0.28, 0.88, 0.40, SLATE);
        self.stroke_box(0.06, 0.28, 0.88, 0.40);
        self.fill(0.14, 0.36, 0.18, 0.08, SLATE_DARK);
        self.disk(0.78, 0.42, 0.06, GREEN);
        self.fill(0.22, 0.56, 0.56, 0.36, WHITE);
        self.stroke_box(0.22, 0.56, 0.56, 0.36);
        self.thick((0.32, 0.68), (0.68, 0.68), SLATE_DARK, 1.3);
        self.thick((0.32, 0.78), (0.58, 0.78), SLATE_DARK, 1.3);
    }

    fn pointer(&self) {
        self.poly(
            &[
                (0.18, 0.04),
                (0.18, 0.90),
                (0.42, 0.64),
                (0.56, 0.52),
                (0.84, 0.48),
            ],
            NAVY,
        );
        self.poly(
            &[(0.40, 0.66), (0.58, 0.88), (0.74, 0.74), (0.54, 0.56)],
            NAVY,
        );
    }

    fn insert_page(&self) {
        self.page(0.16, 0.02, 0.58, 0.72);
        self.fill(0.04, 0.16, 0.58, 0.72, PAPER);
        self.stroke_box(0.04, 0.16, 0.58, 0.72);
        self.disk(0.74, 0.74, 0.20, BLUE);
        self.ring(0.74, 0.74, 0.20);
        self.thick((0.74, 0.62), (0.74, 0.86), WHITE, 2.0);
        self.thick((0.62, 0.74), (0.86, 0.74), WHITE, 2.0);
    }

    fn checkbox(&self) {
        self.fill(0.10, 0.10, 0.80, 0.80, WHITE);
        self.stroke_box(0.10, 0.10, 0.80, 0.80);
        self.thick((0.26, 0.52), (0.44, 0.72), GREEN, 2.6);
        self.thick((0.44, 0.72), (0.78, 0.30), GREEN, 2.6);
    }

    fn calendar(&self) {
        self.fill(0.08, 0.16, 0.84, 0.76, WHITE);
        self.stroke_box(0.08, 0.16, 0.84, 0.76);
        self.fill(0.08, 0.16, 0.84, 0.22, RED);
        self.fill(0.26, 0.06, 0.10, 0.20, NAVY);
        self.fill(0.64, 0.06, 0.10, 0.20, NAVY);
        self.disk(0.32, 0.58, 0.06, BLUE);
        self.disk(0.50, 0.58, 0.06, BLUE);
        self.disk(0.68, 0.58, 0.06, BLUE);
    }

    fn dropdown(&self) {
        self.fill(0.04, 0.26, 0.92, 0.48, WHITE);
        self.stroke_box(0.04, 0.26, 0.92, 0.48);
        self.fill(0.66, 0.26, 0.30, 0.48, BLUE);
        self.poly(&[(0.74, 0.42), (0.90, 0.42), (0.82, 0.60)], WHITE);
    }

    fn picture(&self) {
        self.fill(0.06, 0.14, 0.88, 0.72, WHITE);
        self.stroke_box(0.06, 0.14, 0.88, 0.72);
        self.fill(0.06, 0.14, 0.88, 0.40, Color32::from_rgb(126, 186, 232));
        self.disk(0.28, 0.30, 0.10, GOLD);
        self.poly(&[(0.10, 0.78), (0.40, 0.46), (0.62, 0.78)], GREEN);
        self.poly(
            &[(0.40, 0.78), (0.70, 0.40), (0.92, 0.78)],
            Color32::from_rgb(36, 130, 72),
        );
    }

    fn rule(&self) {
        self.disk(0.16, 0.78, 0.08, BLUE_DARK);
        self.disk(0.84, 0.22, 0.08, BLUE_DARK);
        self.thick((0.16, 0.78), (0.84, 0.22), BLUE, 3.0);
    }

    fn radio(&self) {
        self.disk(0.50, 0.50, 0.40, WHITE);
        self.ring(0.50, 0.50, 0.40);
        self.disk(0.50, 0.50, 0.18, BLUE);
    }

    fn shapes(&self) {
        self.fill(0.28, 0.04, 0.64, 0.56, GOLD);
        self.stroke_box(0.28, 0.04, 0.64, 0.56);
        self.fill(0.04, 0.36, 0.62, 0.56, BLUE);
        self.stroke_box(0.04, 0.36, 0.62, 0.56);
    }

    fn signature(&self) {
        self.fill(0.04, 0.22, 0.92, 0.56, PAPER);
        self.stroke_box(0.04, 0.22, 0.92, 0.56);
        self.thick((0.12, 0.58), (0.28, 0.40), NAVY, 2.0);
        self.thick((0.28, 0.40), (0.42, 0.66), NAVY, 2.0);
        self.thick((0.42, 0.66), (0.58, 0.36), NAVY, 2.0);
        self.thick((0.58, 0.36), (0.88, 0.62), NAVY, 2.0);
    }

    fn table(&self) {
        self.fill(0.06, 0.10, 0.88, 0.80, WHITE);
        self.stroke_box(0.06, 0.10, 0.88, 0.80);
        self.fill(0.06, 0.10, 0.88, 0.22, BLUE);
        self.thick((0.06, 0.50), (0.94, 0.50), OUTLINE, 1.3);
        self.thick((0.50, 0.10), (0.50, 0.90), OUTLINE, 1.3);
    }

    fn letter_t(&self) {
        self.fill(0.06, 0.08, 0.88, 0.84, PAPER);
        self.stroke_box(0.06, 0.08, 0.88, 0.84);
        self.fill(0.18, 0.22, 0.64, 0.16, NAVY);
        self.fill(0.42, 0.22, 0.16, 0.58, NAVY);
    }

    fn text_box(&self) {
        self.fill(0.06, 0.10, 0.88, 0.80, PAPER);
        self.stroke_box(0.06, 0.10, 0.88, 0.80);
        self.fill(0.18, 0.26, 0.64, 0.10, BLUE);
        self.fill(0.18, 0.44, 0.64, 0.10, NAVY);
        self.fill(0.18, 0.62, 0.42, 0.10, NAVY);
    }

    fn text_field(&self) {
        self.fill(0.04, 0.28, 0.92, 0.44, WHITE);
        self.stroke_box(0.04, 0.28, 0.92, 0.44);
        self.fill(0.04, 0.62, 0.92, 0.10, BLUE);
        self.thick((0.22, 0.36), (0.22, 0.58), NAVY, 1.8);
    }

    fn grid(&self) {
        self.fill(0.06, 0.06, 0.88, 0.88, PAPER);
        self.stroke_box(0.06, 0.06, 0.88, 0.88);
        for step in [0.34, 0.62] {
            self.thick((step, 0.06), (step, 0.94), BLUE, 1.5);
            self.thick((0.06, step), (0.94, step), BLUE, 1.5);
        }
    }

    fn page_number(&self) {
        self.page(0.12, 0.04, 0.70, 0.90);
        self.fill(0.40, 0.28, 0.14, 0.48, BLUE);
        self.poly(&[(0.34, 0.40), (0.54, 0.26), (0.54, 0.40)], BLUE);
        self.fill(0.34, 0.70, 0.28, 0.08, BLUE);
    }

    fn dots(&self) {
        self.fill(0.06, 0.06, 0.88, 0.88, PAPER);
        self.stroke_box(0.06, 0.06, 0.88, 0.88);
        for x in [0.28, 0.50, 0.72] {
            for y in [0.28, 0.50, 0.72] {
                self.disk(x, y, 0.07, BLUE);
            }
        }
    }

    fn guides(&self) {
        self.page(0.10, 0.06, 0.80, 0.88);
        self.thick((0.50, 0.10), (0.50, 0.90), RED, 1.8);
        self.thick((0.16, 0.50), (0.84, 0.50), RED, 1.8);
    }

    fn margins(&self) {
        self.page(0.06, 0.06, 0.88, 0.88);
        self.stroke_only(0.26, 0.26, 0.48, 0.48, BLUE);
    }

    fn stroke_only(&self, x: f32, y: f32, w: f32, h: f32, color: Color32) {
        self.painter.rect_stroke(
            self.box_at(x, y, w, h),
            1.0,
            Stroke::new(1.6_f32, color),
            StrokeKind::Inside,
        );
    }

    fn snap(&self) {
        self.disk(0.50, 0.36, 0.34, RED);
        self.disk(0.50, 0.40, 0.16, Color32::from_rgb(210, 214, 220));
        self.fill(0.16, 0.36, 0.20, 0.40, RED);
        self.fill(0.64, 0.36, 0.20, 0.40, RED);
        self.fill(0.16, 0.68, 0.20, 0.22, SLATE);
        self.fill(0.64, 0.68, 0.20, 0.22, SLATE);
        self.stroke_box(0.16, 0.68, 0.20, 0.22);
        self.stroke_box(0.64, 0.68, 0.20, 0.22);
    }

    fn moon(&self) {
        let steps = 8;
        let mut points = Vec::with_capacity(steps + 1);
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let angle = std::f32::consts::FRAC_PI_2 + t * std::f32::consts::PI;
            points.push((0.64 + 0.34 * angle.cos(), 0.50 + 0.40 * angle.sin()));
        }
        self.poly(&points, GOLD);
    }

    fn sun(&self) {
        self.disk(0.50, 0.50, 0.24, GOLD);
        self.ring(0.50, 0.50, 0.24);
        for (x0, y0, x1, y1) in [
            (0.50, 0.04, 0.50, 0.18),
            (0.50, 0.82, 0.50, 0.96),
            (0.04, 0.50, 0.18, 0.50),
            (0.82, 0.50, 0.96, 0.50),
            (0.16, 0.16, 0.28, 0.28),
            (0.72, 0.72, 0.84, 0.84),
            (0.16, 0.84, 0.28, 0.72),
            (0.72, 0.28, 0.84, 0.16),
        ] {
            self.thick((x0, y0), (x1, y1), Color32::from_rgb(214, 120, 28), 2.2);
        }
    }
}

fn paint_caret(painter: &egui::Painter, rect: Rect, color: Color32) {
    painter.add(egui::Shape::convex_polygon(
        vec![rect.left_top(), rect.right_top(), rect.center_bottom()],
        color,
        Stroke::NONE,
    ));
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
