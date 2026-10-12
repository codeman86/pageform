//! Monochrome toolbar icons from Material Symbols Light.
//!
//! Path data is the Iconify set `material-symbols-light` (Google, Apache-2.0):
//! https://github.com/google/material-design-icons/blob/master/LICENSE
//! https://icon-sets.iconify.design/material-symbols-light/
//!
//! Glyphs are filled with the current theme text color. No colored fills and
//! no imported icon images.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use lyon_path::builder::SvgPathBuilder;
use lyon_path::Path;
use lyon_tessellation::math::{point, vector};
use lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};
use std::sync::OnceLock;

include!("icon_paths.rs");

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
const VIEWBOX: f32 = 24.0;

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

impl Icon {
    const ALL: [Icon; 26] = [
        Icon::New,
        Icon::Open,
        Icon::Save,
        Icon::Print,
        Icon::Select,
        Icon::InsertPage,
        Icon::Checkbox,
        Icon::Date,
        Icon::Dropdown,
        Icon::Image,
        Icon::Line,
        Icon::Radio,
        Icon::Shapes,
        Icon::Signature,
        Icon::Table,
        Icon::Text,
        Icon::TextBox,
        Icon::TextField,
        Icon::Grid,
        Icon::PageNumber,
        Icon::Dots,
        Icon::Guides,
        Icon::Margins,
        Icon::Snap,
        Icon::Moon,
        Icon::Sun,
    ];

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|icon| std::mem::discriminant(icon) == std::mem::discriminant(&self))
            .expect("icon")
    }

    /// Iconify name inside `material-symbols-light`.
    pub fn iconify_name(self) -> &'static str {
        icon_source(self).0
    }
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
    let glyph = glyph_rect(rect, caret, icon);
    paint_icon(ui.painter(), glyph, visuals.fg_stroke.color, icon);
    if caret {
        let caret_box = Rect::from_center_size(
            Pos2::new(rect.right() - 8.0, rect.center().y + 1.0),
            Vec2::new(8.0, 6.0),
        );
        paint_caret(ui.painter(), caret_box, visuals.fg_stroke.color);
    }
    response.on_hover_text(tip)
}

fn glyph_rect(rect: Rect, caret: bool, icon: Icon) -> Rect {
    // New sits further inside the button so the page corner is not clipped
    // by the dropdown caret.
    let (size, left_pad) = match icon {
        Icon::New => (16.0, 7.0),
        _ => (18.0, 5.0),
    };
    if caret {
        Rect::from_min_size(
            rect.min + Vec2::new(left_pad, (rect.height() - size) * 0.5),
            Vec2::splat(size),
        )
    } else {
        Rect::from_center_size(rect.center(), Vec2::splat(size))
    }
}

struct Tess {
    vertices: Vec<Pos2>,
    indices: Vec<u32>,
}

fn meshes() -> &'static [Tess] {
    static CACHE: OnceLock<Vec<Tess>> = OnceLock::new();
    CACHE.get_or_init(|| Icon::ALL.iter().copied().map(tessellate).collect())
}

fn paint_icon(painter: &egui::Painter, rect: Rect, color: Color32, icon: Icon) {
    let _icon_name = icon.iconify_name();
    let tess = &meshes()[icon.index()];
    if tess.indices.is_empty() {
        return;
    }
    let mesh = egui::Mesh {
        vertices: tess
            .vertices
            .iter()
            .map(|pos| egui::epaint::Vertex {
                pos: Pos2::new(
                    rect.left() + pos.x / VIEWBOX * rect.width(),
                    rect.top() + pos.y / VIEWBOX * rect.height(),
                ),
                uv: egui::epaint::WHITE_UV,
                color,
            })
            .collect(),
        indices: tess.indices.clone(),
        ..Default::default()
    };
    painter.add(egui::Shape::mesh(mesh));
}

fn tessellate(icon: Icon) -> Tess {
    let path = parse_path(icon_source(icon).1);
    let mut buffers: VertexBuffers<lyon_tessellation::math::Point, u16> = VertexBuffers::new();
    let mut tess = FillTessellator::new();
    if tess
        .tessellate_path(
            &path,
            &FillOptions::tolerance(0.08),
            &mut BuffersBuilder::new(&mut buffers, |vertex: FillVertex| vertex.position()),
        )
        .is_err()
    {
        return Tess {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
    }
    Tess {
        vertices: buffers
            .vertices
            .into_iter()
            .map(|p| Pos2::new(p.x, p.y))
            .collect(),
        indices: buffers.indices.into_iter().map(u32::from).collect(),
    }
}

fn parse_path(data: &str) -> Path {
    let bytes = data.as_bytes();
    let mut index = 0;
    let mut builder = Path::builder().with_svg();
    let mut command = b'M';

    let skip = |index: &mut usize| {
        while let Some(c) = bytes.get(*index).copied() {
            if c == b',' || c.is_ascii_whitespace() {
                *index += 1;
            } else {
                break;
            }
        }
    };
    let number = |index: &mut usize| -> f32 {
        skip(index);
        let start = *index;
        if matches!(bytes.get(*index), Some(b'+' | b'-')) {
            *index += 1;
        }
        let mut dotted = false;
        while let Some(c) = bytes.get(*index).copied() {
            if c.is_ascii_digit() {
                *index += 1;
            } else if c == b'.' && !dotted {
                dotted = true;
                *index += 1;
            } else {
                break;
            }
        }
        std::str::from_utf8(&bytes[start..*index])
            .ok()
            .and_then(|text| text.parse().ok())
            .unwrap_or(0.0)
    };
    let pair = |index: &mut usize| -> (f32, f32) { (number(index), number(index)) };

    skip(&mut index);
    while index < bytes.len() {
        skip(&mut index);
        if index >= bytes.len() {
            break;
        }
        if bytes[index].is_ascii_alphabetic() {
            command = bytes[index];
            index += 1;
        }
        match command {
            b'M' => {
                let (x, y) = pair(&mut index);
                builder.move_to(point(x, y));
                command = b'L';
            }
            b'm' => {
                let (x, y) = pair(&mut index);
                builder.relative_move_to(vector(x, y));
                command = b'l';
            }
            b'L' => {
                let (x, y) = pair(&mut index);
                builder.line_to(point(x, y));
            }
            b'l' => {
                let (x, y) = pair(&mut index);
                builder.relative_line_to(vector(x, y));
            }
            b'H' => builder.horizontal_line_to(number(&mut index)),
            b'h' => builder.relative_horizontal_line_to(number(&mut index)),
            b'V' => builder.vertical_line_to(number(&mut index)),
            b'v' => builder.relative_vertical_line_to(number(&mut index)),
            b'Q' => {
                let (x1, y1) = pair(&mut index);
                let (x, y) = pair(&mut index);
                builder.quadratic_bezier_to(point(x1, y1), point(x, y));
            }
            b'q' => {
                let (x1, y1) = pair(&mut index);
                let (x, y) = pair(&mut index);
                builder.relative_quadratic_bezier_to(vector(x1, y1), vector(x, y));
            }
            b'T' => {
                let (x, y) = pair(&mut index);
                builder.smooth_quadratic_bezier_to(point(x, y));
            }
            b't' => {
                let (x, y) = pair(&mut index);
                builder.smooth_relative_quadratic_bezier_to(vector(x, y));
            }
            b'Z' | b'z' => builder.close(),
            _ => break,
        }
    }
    builder.build()
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
        assert!(!VIEW_TOGGLES.contains(&"Header"));
        assert!(!VIEW_TOGGLES.contains(&"Footer"));
        assert_eq!(DOC_COMMANDS, ["New", "Open", "Save", "Print"]);
        assert_eq!(
            VIEW_TOGGLES,
            ["Dots", "Guides", "Margins", "Page numbers", "Snap"]
        );
        assert_eq!(VIEW_TOGGLES.len(), VIEW_ICONS.len());
    }

    #[test]
    fn every_icon_is_a_material_symbols_light_glyph() {
        let names: Vec<_> = Icon::ALL.iter().map(|icon| icon.iconify_name()).collect();
        assert_eq!(
            names,
            [
                "description-outline",
                "folder-open-outline",
                "save-outline",
                "print-outline",
                "arrow-selector-tool-outline",
                "note-stack-add-outline",
                "check-box-outline",
                "calendar-today-outline",
                "form-select-outline",
                "image-outline",
                "horizontal-rule",
                "radio-button-checked",
                "shapes-outline",
                "signature-outline-rounded",
                "table-outline",
                "material-symbols:title",
                "text-snippet-outline",
                "text-fields",
                "grid-on-outline",
                "looks-one-outline",
                "grain",
                "grid-guides",
                "margin-outline",
                "my-location-outline",
                "dark-mode-outline",
                "light-mode-outline",
            ]
        );
        for icon in Icon::ALL {
            let tess = tessellate(icon);
            assert!(
                tess.indices.len() >= 3,
                "{} did not tessellate",
                icon.iconify_name()
            );
            let area = mesh_area(&tess);
            assert!(
                area < 280.0,
                "{} fills {:.0} px² of the 24² box (solid square)",
                icon.iconify_name(),
                area
            );
        }
        assert_visible_stroke(Icon::Dropdown, 14.0, 8.0, 40.0);
        assert_visible_stroke(Icon::Line, 12.0, 2.4, 30.0);
        assert_eq!(Icon::TextField.iconify_name(), "text-fields");
        assert_eq!(Icon::Text.iconify_name(), "material-symbols:title");
        let text = tessellate(Icon::Text);
        assert!(covers(&text, 12.0, 5.5), "Text crossbar");
        assert!(covers(&text, 7.0, 5.5), "Text crossbar left");
        assert!(covers(&text, 17.0, 5.5), "Text crossbar right");
        assert!(covers(&text, 12.0, 13.0), "Text stem");
        assert!(covers(&text, 12.0, 18.5), "Text stem reaches the bottom");
        assert!(
            !covers(&text, 8.0, 14.0),
            "Text stem is not the whole width"
        );
        let dropdown = tessellate(Icon::Dropdown);
        assert!(covers(&dropdown, 8.0, 8.8), "combo text line");
        assert!(covers(&dropdown, 7.0, 12.7), "combo second text line");
        assert!(covers(&dropdown, 18.3, 10.2), "combo chevron");
        assert!(covers(&dropdown, 12.0, 5.8), "combo frame");
        assert!(
            !covers(&dropdown, 8.0, 10.8),
            "combo field is not a filled plate"
        );
        assert!(
            !covers(&dropdown, 18.3, 15.2),
            "chevron button is not a filled square"
        );
    }

    fn covers(tess: &Tess, x: f32, y: f32) -> bool {
        let mut index = 0;
        while index + 2 < tess.indices.len() {
            let a = tess.vertices[tess.indices[index] as usize];
            let b = tess.vertices[tess.indices[index + 1] as usize];
            let c = tess.vertices[tess.indices[index + 2] as usize];
            let side = |p: Pos2, q: Pos2| (x - q.x) * (p.y - q.y) - (p.x - q.x) * (y - q.y);
            let d1 = side(a, b);
            let d2 = side(b, c);
            let d3 = side(c, a);
            let has_neg = d1 < -0.05 || d2 < -0.05 || d3 < -0.05;
            let has_pos = d1 > 0.05 || d2 > 0.05 || d3 > 0.05;
            if !(has_neg && has_pos) {
                return true;
            }
            index += 3;
        }
        false
    }

    fn assert_visible_stroke(icon: Icon, min_width: f32, min_height: f32, min_area: f32) {
        let tess = tessellate(icon);
        let (min, max) = mesh_bounds(&tess);
        let width = max.x - min.x;
        let height = max.y - min.y;
        let area = mesh_area(&tess);
        assert!(
            width >= min_width && height >= min_height && area >= min_area,
            "{} spans {:.1}×{:.1} and fills {:.0}; expected at least {:.0}×{:.0} and {:.0}",
            icon.iconify_name(),
            width,
            height,
            area,
            min_width,
            min_height,
            min_area
        );
    }

    fn mesh_bounds(tess: &Tess) -> (Pos2, Pos2) {
        let mut min = Pos2::new(f32::MAX, f32::MAX);
        let mut max = Pos2::new(f32::MIN, f32::MIN);
        for pos in &tess.vertices {
            min.x = min.x.min(pos.x);
            min.y = min.y.min(pos.y);
            max.x = max.x.max(pos.x);
            max.y = max.y.max(pos.y);
        }
        (min, max)
    }

    fn mesh_area(tess: &Tess) -> f32 {
        let mut area = 0.0;
        let mut index = 0;
        while index + 2 < tess.indices.len() {
            let a = tess.vertices[tess.indices[index] as usize];
            let b = tess.vertices[tess.indices[index + 1] as usize];
            let c = tess.vertices[tess.indices[index + 2] as usize];
            area += ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)).abs() * 0.5;
            index += 3;
        }
        area
    }

    fn assert_sorted(labels: &[&str]) {
        let mut sorted = labels.to_vec();
        sorted.sort_unstable();
        assert_eq!(labels, sorted.as_slice());
    }
}
