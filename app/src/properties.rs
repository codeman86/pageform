//! Right-hand properties panel.
//!
//! Hidden until the user opens it. Pinned panels stay open. Unpinned panels
//! close when the selection is cleared, including a click on empty canvas.

use eframe::egui::{self, Vec2};
use pageform_core::{
    AlignEdge, BorderStyleKind, CheckboxStyle, FieldId, FieldKind, FontFamily, GridSize,
    Orientation, PageSize, PdfPoint, Rgb, TextAlign,
};

use super::{kind_note, PageFormApp};

impl PageFormApp {
    pub(super) fn properties_panel(&mut self, ui: &mut egui::Ui) {
        bump_sidebar_fonts(ui);
        self.properties_header(ui);
        ui.separator();
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                if self.selection.len() > 1 {
                    self.multi_properties(ui);
                } else if self.primary().is_some() {
                    self.item_properties(ui);
                } else {
                    self.page_properties(ui);
                }
            });
    }

    fn properties_header(&mut self, ui: &mut egui::Ui) {
        let title = self.sidebar_title();
        ui.horizontal(|ui| {
            ui.strong(title);
            if self.selection.len() == 1 {
                if let Some(id) = self.primary() {
                    if let Some(field) = self.doc.field(id) {
                        info_icon(ui, kind_note(field.kind()));
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("X")
                    .on_hover_text("Close the properties panel")
                    .clicked()
                {
                    self.set_properties_open(false);
                }
                if pin_button(ui, self.properties_pinned)
                    .on_hover_text(
                        "Pinned panels stay open. Unpinned panels close when you click empty canvas.",
                    )
                    .clicked()
                {
                    self.set_properties_pinned(!self.properties_pinned);
                }
            });
        });
    }

    fn sidebar_title(&self) -> String {
        if self.selection.len() == 1 {
            if let Some(id) = self.primary() {
                if let Some(field) = self.doc.field(id) {
                    return sidebar_kind_title(field.kind()).to_string();
                }
            }
        }
        "Properties".to_string()
    }

    fn page_properties(&mut self, ui: &mut egui::Ui) {
        ui.label("Nothing selected");
        section(ui, "Page");
        let current = self.doc.page_size();
        let mut chosen = current;
        egui::ComboBox::from_id_salt("page_size")
            .selected_text(current.label())
            .show_ui(ui, |ui| {
                for size in PageSize::ALL {
                    ui.selectable_value(&mut chosen, size, size.label());
                }
            });
        if chosen != current {
            self.doc.set_page_size(chosen);
            self.refit_page();
        }
        let page = self.doc.page();
        ui.label(format!("{:.0} x {:.0} pt", page.width, page.height));
        ui.label("Orientation");
        ui.horizontal(|ui| {
            let portrait = self.doc.orientation() == Orientation::Portrait;
            if ui.selectable_label(portrait, "Portrait").clicked() && !portrait {
                self.doc.set_orientation(Orientation::Portrait);
                self.refit_page();
            }
            if ui.selectable_label(!portrait, "Landscape").clicked() && portrait {
                self.doc.set_orientation(Orientation::Landscape);
                self.refit_page();
            }
        });
        ui.label("Size and orientation are the PDF media box.");

        section(ui, "Margins");
        let margins = self.doc.margins();
        let mut next = margins;
        let mut changed = false;
        if let Some(value) = drag_pt(ui, "Top", margins.top, 0.0..=360.0) {
            next.top = value;
            changed = true;
        }
        if let Some(value) = drag_pt(ui, "Right", margins.right, 0.0..=360.0) {
            next.right = value;
            changed = true;
        }
        if let Some(value) = drag_pt(ui, "Bottom", margins.bottom, 0.0..=360.0) {
            next.bottom = value;
            changed = true;
        }
        if let Some(value) = drag_pt(ui, "Left", margins.left, 0.0..=360.0) {
            next.left = value;
            changed = true;
        }
        if changed {
            self.doc.set_margins(next);
        }
        ui.label("Canvas only. Not written into the PDF.");

        section(ui, "Grid");
        for size in GridSize::ALL {
            ui.radio_value(&mut self.grid_size, size, size.label())
                .on_hover_text(size.minor_label());
        }

        section(ui, "Header and footer");
        let mut header = self.doc.header_text().to_string();
        if ui
            .add(
                egui::TextEdit::singleline(&mut header)
                    .desired_width(f32::INFINITY)
                    .hint_text("Header"),
            )
            .changed()
        {
            self.doc.set_header_text(&header);
        }
        let mut footer = self.doc.footer_text().to_string();
        if ui
            .add(
                egui::TextEdit::singleline(&mut footer)
                    .desired_width(f32::INFINITY)
                    .hint_text("Footer"),
            )
            .changed()
        {
            self.doc.set_footer_text(&footer);
        }
        ui.label("Shown when Header or Footer is on in the View menu. Canvas only.");
    }

    fn item_properties(&mut self, ui: &mut egui::Ui) {
        self.sync_field_buffers();
        let Some(id) = self.primary() else {
            return;
        };
        let Some(field) = self.doc.field(id) else {
            self.selection.clear();
            return;
        };
        let kind = field.kind();
        let rect = field.rect();
        let style = field.style().clone();
        let line = field.line_ends();

        collapsing(ui, "Position", true, |ui| {
            self.position_grid(ui, id, rect);
            if let Some((start, end)) = line {
                self.line_end_grid(ui, id, start, end);
            }
        });

        collapsing(ui, "Text", true, |ui| {
            if kind.is_form_field() {
                self.identity_section(ui, id, kind);
            } else if matches!(
                kind,
                FieldKind::StaticText | FieldKind::TextBox | FieldKind::Image
            ) {
                self.caption_edit(ui, id, "Text");
            }
            self.type_section(ui, id, kind, &style);
        });

        if draws_text(kind) {
            collapsing(ui, "Font", true, |ui| {
                self.font_section(ui, id, &style);
            });
        }
        collapsing(ui, "Border", false, |ui| {
            if matches!(
                kind,
                FieldKind::Line | FieldKind::Rectangle | FieldKind::Image | FieldKind::Table
            ) {
                self.stroke_section(ui, id, &style);
            } else {
                self.border_section(ui, id, &style);
            }
        });
        if kind.is_form_field() {
            collapsing(ui, "Calculations", false, |ui| {
                self.calculations_section(ui);
            });
        }
    }

    fn position_grid(&mut self, ui: &mut egui::Ui, id: FieldId, rect: pageform_core::RectPt) {
        let mut next = rect;
        let mut moved = false;
        egui::Grid::new("position")
            .num_columns(4)
            .spacing(Vec2::new(8.0, 6.0))
            .show(ui, |ui| {
                moved |= labeled_drag(ui, "X", &mut next.x, 0.0..=2000.0, Some(POSITION_TIP));
                moved |= labeled_drag(ui, "Y", &mut next.y, 0.0..=2000.0, Some(POSITION_TIP));
                ui.end_row();
                moved |= labeled_drag(ui, "W", &mut next.w, 1.0..=2000.0, Some(POSITION_TIP));
                moved |= labeled_drag(ui, "H", &mut next.h, 1.0..=2000.0, Some(POSITION_TIP));
                ui.end_row();
            });
        if moved {
            self.doc.set_bounds(id, next);
        }
    }

    fn line_end_grid(&mut self, ui: &mut egui::Ui, id: FieldId, start: PdfPoint, end: PdfPoint) {
        egui::Grid::new("line-ends")
            .num_columns(4)
            .spacing(Vec2::new(8.0, 6.0))
            .show(ui, |ui| {
                if let Some(value) = labeled_drag_value(ui, "X1", start.x, 0.0..=2000.0, None) {
                    self.doc.set_line_end(
                        id,
                        0,
                        PdfPoint {
                            x: value,
                            y: start.y,
                        },
                    );
                }
                if let Some(value) = labeled_drag_value(ui, "Y1", start.y, 0.0..=2000.0, None) {
                    self.doc.set_line_end(
                        id,
                        0,
                        PdfPoint {
                            x: start.x,
                            y: value,
                        },
                    );
                }
                ui.end_row();
                if let Some(value) = labeled_drag_value(ui, "X2", end.x, 0.0..=2000.0, None) {
                    self.doc
                        .set_line_end(id, 1, PdfPoint { x: value, y: end.y });
                }
                if let Some(value) = labeled_drag_value(ui, "Y2", end.y, 0.0..=2000.0, None) {
                    self.doc
                        .set_line_end(id, 1, PdfPoint { x: end.x, y: value });
                }
                ui.end_row();
            });
    }

    fn identity_section(&mut self, ui: &mut egui::Ui, id: FieldId, kind: FieldKind) {
        let name_label = if kind == FieldKind::Radio {
            "Group name"
        } else {
            "Name"
        };
        let name_changed = labeled_edit(ui, name_label, &mut self.name_buf, "");
        if name_changed {
            self.name_error = self
                .doc
                .set_name(id, &self.name_buf)
                .err()
                .map(|err| err.to_string());
        }
        if labeled_edit(ui, "Tooltip", &mut self.tooltip_buf, "Exported as /TU") {
            self.doc.set_tooltip(id, &self.tooltip_buf);
        }
        let mut required = self
            .doc
            .field(id)
            .is_some_and(|field| field.style().required);
        if ui.checkbox(&mut required, "Required").changed() {
            self.doc.set_required(id, required);
        }
        let mut read_only = self
            .doc
            .field(id)
            .is_some_and(|field| field.style().read_only);
        if ui.checkbox(&mut read_only, "Read-only").changed() {
            self.doc.set_read_only(id, read_only);
        }
        if matches!(
            kind,
            FieldKind::Text | FieldKind::Date | FieldKind::Dropdown
        ) && labeled_edit(ui, "Default", &mut self.value_buf, "Default value")
        {
            self.doc.set_default_value(id, &self.value_buf);
            let text = self.value_buf.clone();
            self.note_sidebar_text(id, super::InlineTarget::DefaultValue, &text);
        }
        if let Some(error) = &self.name_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn type_section(
        &mut self,
        ui: &mut egui::Ui,
        id: FieldId,
        kind: FieldKind,
        style: &pageform_core::FieldStyle,
    ) {
        match kind {
            FieldKind::Dropdown => {
                ui.label("Items").on_hover_text("One item per line.");
                let edit = ui.add(
                    egui::TextEdit::multiline(&mut self.options_buf)
                        .desired_width(f32::INFINITY)
                        .desired_rows(4),
                );
                if edit.changed() {
                    let options = self
                        .options_buf
                        .lines()
                        .map(|option| option.trim().to_string())
                        .filter(|option| !option.is_empty())
                        .collect();
                    self.name_error = self
                        .doc
                        .set_options(id, options)
                        .err()
                        .map(|err| err.to_string());
                }
            }
            FieldKind::Radio => {
                self.caption_edit(ui, id, "Label");
                if !self.doc.radio_labels() {
                    ui.label("Hidden until Radio labels is on in Edit > Preferences.");
                }
                let checked = self
                    .doc
                    .field(id)
                    .is_some_and(|field| field.style().checked());
                let mut on = checked;
                if ui.checkbox(&mut on, "Selected by default").changed() {
                    self.doc.set_checked(id, on);
                }
                let group = self
                    .doc
                    .field(id)
                    .map(|field| field.name().to_string())
                    .unwrap_or_default();
                ui.label("Options in this group");
                let members: Vec<(String, String)> = self
                    .doc
                    .fields()
                    .iter()
                    .filter(|member| member.kind() == FieldKind::Radio && member.name() == group)
                    .map(|member| (member.on_state().to_string(), member.caption().to_string()))
                    .collect();
                for (state, caption) in members {
                    ui.label(format!("{state}  {caption}"));
                }
            }
            FieldKind::Checkbox => {
                let mut chosen = style.checkbox_style;
                egui::ComboBox::from_id_salt(("check", id.0))
                    .selected_text(chosen.label())
                    .show_ui(ui, |ui| {
                        for style in CheckboxStyle::ALL {
                            ui.selectable_value(&mut chosen, style, style.label());
                        }
                    });
                if chosen != style.checkbox_style {
                    self.doc
                        .update_style(id, |style| style.checkbox_style = chosen);
                }
                let mut on = style.checked();
                if ui.checkbox(&mut on, "Checked by default").changed() {
                    self.doc.set_checked(id, on);
                }
            }
            FieldKind::Date => {
                ui.label("Format").on_hover_text(
                    "Not a calendar popup. The format is the empty-field hint and /TU when tooltip is blank.",
                );
                let mut format = style.date_format.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut format).desired_width(f32::INFINITY))
                    .changed()
                {
                    self.doc
                        .update_style(id, |style| style.date_format = format);
                }
                self.max_chars_row(ui, id, style.max_chars);
            }
            FieldKind::Text => {
                self.max_chars_row(ui, id, style.max_chars);
                self.multiline_and_align(ui, id, style);
            }
            FieldKind::TextBox => {
                self.caption_edit(ui, id, "Text");
                self.multiline_and_align(ui, id, style);
            }
            FieldKind::StaticText => {
                self.align_row(ui, id, style.align);
            }
            FieldKind::Signature => {
                self.caption_edit(ui, id, "Label");
            }
            FieldKind::Image => {}
            FieldKind::Table => {}
            FieldKind::Line | FieldKind::Rectangle => {}
        }
    }

    fn font_section(&mut self, ui: &mut egui::Ui, id: FieldId, style: &pageform_core::FieldStyle) {
        ui.horizontal(|ui| {
            let mut font = style.font;
            egui::ComboBox::from_id_salt(("font", id.0))
                .width(108.0)
                .selected_text(font.label())
                .show_ui(ui, |ui| {
                    for family in FontFamily::ALL {
                        ui.selectable_value(&mut font, family, family.label());
                    }
                });
            if font != style.font {
                self.doc.update_style(id, |style| style.font = font);
            }
            if let Some(size) = drag_num(ui, style.font_size, 4.0..=96.0) {
                self.doc.update_style(id, |style| style.font_size = size);
            }
            if let Some(color) = color_swatch(ui, style.text_color) {
                self.doc
                    .update_style(id, |style| style.text_color = color);
            }
            info_icon(
                ui,
                "Helvetica, Times, and Courier are standard PDF fonts. On the canvas, Times uses the same proportional face as Helvetica.",
            );
        });
    }

    fn border_section(
        &mut self,
        ui: &mut egui::Ui,
        id: FieldId,
        style: &pageform_core::FieldStyle,
    ) {
        ui.horizontal(|ui| {
            if let Some(width) = drag_num(ui, style.border_width, 0.0..=24.0) {
                self.doc
                    .update_style(id, |style| style.border_width = width);
            }
            self.border_style_row(ui, id, style.border_style);
            if let Some(color) = color_swatch(ui, style.border_color) {
                self.doc
                    .update_style(id, |style| style.border_color = color);
            }
        });
        self.fill_row(ui, id, style.fill);
    }

    fn stroke_section(
        &mut self,
        ui: &mut egui::Ui,
        id: FieldId,
        style: &pageform_core::FieldStyle,
    ) {
        ui.horizontal(|ui| {
            if let Some(width) = drag_num(ui, style.stroke_width, 0.0..=24.0) {
                self.doc
                    .update_style(id, |style| style.stroke_width = width);
            }
            self.border_style_row(ui, id, style.border_style);
            if let Some(color) = color_swatch(ui, style.stroke_color) {
                self.doc
                    .update_style(id, |style| style.stroke_color = color);
            }
        });
        if self
            .doc
            .field(id)
            .is_some_and(|field| field.kind() != FieldKind::Line)
        {
            self.fill_row(ui, id, style.fill);
        }
    }

    fn border_style_row(&mut self, ui: &mut egui::Ui, id: FieldId, current: BorderStyleKind) {
        let mut chosen = current;
        egui::ComboBox::from_id_salt(("border", id.0))
            .width(96.0)
            .selected_text(chosen.label())
            .show_ui(ui, |ui| {
                for style in BorderStyleKind::ALL {
                    ui.selectable_value(&mut chosen, style, style.label());
                }
            });
        if chosen != current {
            self.doc
                .update_style(id, |style| style.border_style = chosen);
        }
    }

    fn fill_row(&mut self, ui: &mut egui::Ui, id: FieldId, fill: Option<Rgb>) {
        ui.horizontal(|ui| {
            ui.add_sized(Vec2::new(LABEL_W, 18.0), egui::Label::new("Fill"));
            let mut enabled = fill.is_some();
            if ui.checkbox(&mut enabled, "").changed() {
                self.doc.update_style(id, |style| {
                    style.fill = if enabled { Some(Rgb::WHITE) } else { None };
                });
            }
            if let Some(color) = fill {
                if let Some(color) = color_swatch(ui, color) {
                    self.doc.update_style(id, |style| style.fill = Some(color));
                }
            }
        });
    }

    fn calculations_section(&self, ui: &mut egui::Ui) {
        ui.add_enabled(false, egui::Button::new("Edit calculation"))
            .on_hover_text("Calculated fields are not available yet. Nothing here is exported.");
    }

    fn multi_properties(&mut self, ui: &mut egui::Ui) {
        let count = self.selection.len();
        ui.label(format!("{count} items selected"));
        ui.label("Shift-click or Ctrl-click to change the selection. The last item is the size reference.");
        section(ui, "Align");
        ui.horizontal_wrapped(|ui| {
            for (label, edge) in [
                ("Left", AlignEdge::Left),
                ("Center", AlignEdge::Center),
                ("Right", AlignEdge::Right),
                ("Top", AlignEdge::Top),
                ("Middle", AlignEdge::Middle),
                ("Bottom", AlignEdge::Bottom),
            ] {
                if ui.button(label).clicked() {
                    let ids = self.selection.clone();
                    self.doc.align_fields(&ids, edge);
                }
            }
        });
        section(ui, "Distribute evenly");
        let can_distribute = count >= 3;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(can_distribute, egui::Button::new("Horizontally"))
                .on_hover_text(
                    "Equal gaps from the leftmost item to the rightmost. Needs three items.",
                )
                .clicked()
            {
                let ids = self.selection.clone();
                self.doc.distribute_fields(&ids, true);
            }
            if ui
                .add_enabled(can_distribute, egui::Button::new("Vertically"))
                .on_hover_text("Equal gaps from the lowest item to the highest. Needs three items.")
                .clicked()
            {
                let ids = self.selection.clone();
                self.doc.distribute_fields(&ids, false);
            }
        });
        section(ui, "Match size");
        ui.horizontal(|ui| {
            if ui.button("Width").clicked() {
                let ids = self.selection.clone();
                let reference = ids.len() - 1;
                self.doc.match_field_size(&ids, reference, true, false);
            }
            if ui.button("Height").clicked() {
                let ids = self.selection.clone();
                let reference = ids.len() - 1;
                self.doc.match_field_size(&ids, reference, false, true);
            }
        });
        self.calculations_section(ui);
    }

    fn caption_edit(&mut self, ui: &mut egui::Ui, id: FieldId, label: &str) {
        ui.horizontal(|ui| {
            ui.add_sized(Vec2::new(LABEL_W, 18.0), egui::Label::new(label));
            let width = ui.available_width();
            let edit =
                ui.add(egui::TextEdit::singleline(&mut self.caption_buf).desired_width(width));
            if edit.changed() {
                self.name_error = self
                    .doc
                    .set_caption(id, &self.caption_buf)
                    .err()
                    .map(|err| err.to_string());
                let text = self.caption_buf.clone();
                self.note_sidebar_text(id, super::InlineTarget::Caption, &text);
            }
        });
    }

    fn max_chars_row(&mut self, ui: &mut egui::Ui, id: FieldId, current: Option<i32>) {
        let mut unlimited = current.is_none();
        if ui.checkbox(&mut unlimited, "No character limit").changed() {
            self.doc.update_style(id, |style| {
                style.max_chars = if unlimited { None } else { Some(20) };
            });
        }
        if let Some(max) = current {
            if let Some(value) = drag_pt(ui, "Max chars", max as f32, 1.0..=5000.0) {
                let max = value.round() as i32;
                self.doc
                    .update_style(id, |style| style.max_chars = Some(max));
            }
        }
    }

    fn multiline_and_align(
        &mut self,
        ui: &mut egui::Ui,
        id: FieldId,
        style: &pageform_core::FieldStyle,
    ) {
        let mut multiline = style.multiline;
        if ui.checkbox(&mut multiline, "Multiline").changed() {
            self.doc
                .update_style(id, |style| style.multiline = multiline);
        }
        self.align_row(ui, id, style.align);
    }

    fn align_row(&mut self, ui: &mut egui::Ui, id: FieldId, current: TextAlign) {
        ui.horizontal(|ui| {
            ui.add_sized(Vec2::new(LABEL_W, 18.0), egui::Label::new("Align"));
            for align in TextAlign::ALL {
                if align_icon(ui, align, current == align).clicked() && current != align {
                    self.doc.update_style(id, |style| style.align = align);
                }
            }
        });
    }

    pub(super) fn sync_field_buffers(&mut self) {
        let id = self.primary();
        if self.name_for == id {
            return;
        }
        self.name_for = id;
        let field = id.and_then(|id| self.doc.field(id));
        self.name_buf = field
            .map(|field| field.name().to_string())
            .unwrap_or_default();
        self.caption_buf = field
            .map(|field| field.caption().to_string())
            .unwrap_or_default();
        self.tooltip_buf = field
            .map(|field| field.style().tooltip.clone())
            .unwrap_or_default();
        self.value_buf = field
            .map(|field| field.style().default_value.clone())
            .unwrap_or_default();
        self.options_buf = field
            .map(|field| field.options().join("\n"))
            .unwrap_or_default();
        self.name_error = None;
    }

    fn refit_page(&mut self) {
        self.view.page_height = self.doc.page().height;
        self.fitted = false;
    }
}

fn draws_text(kind: FieldKind) -> bool {
    matches!(
        kind,
        FieldKind::Text
            | FieldKind::Date
            | FieldKind::Dropdown
            | FieldKind::StaticText
            | FieldKind::TextBox
            | FieldKind::Signature
            | FieldKind::Radio
    )
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(title).strong());
}

const LABEL_W: f32 = 72.0;
const POSITION_TIP: &str = "Points. Y is up from the bottom of the page.";

fn drag_pt(
    ui: &mut egui::Ui,
    label: &str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
) -> Option<f32> {
    drag_pt_wheel(ui, label, value, range)
}

/// Wheel changes the value by 1 pt. Shift+wheel uses 0.5 pt. The scroll is consumed.
fn drag_pt_wheel(
    ui: &mut egui::Ui,
    label: &str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
) -> Option<f32> {
    ui.horizontal(|ui| {
        ui.add_sized(Vec2::new(LABEL_W, 18.0), egui::Label::new(label));
        drag_num(ui, value, range)
    })
    .inner
}

fn drag_num(ui: &mut egui::Ui, value: f32, range: std::ops::RangeInclusive<f32>) -> Option<f32> {
    let mut value = value;
    let response = ui
        .add(
            egui::DragValue::new(&mut value)
                .range(range.clone())
                .speed(0.5)
                .max_decimals(1),
        )
        .on_hover_text("Scroll to adjust by 1 pt. Shift+scroll adjusts by 0.5 pt.");
    let mut changed = response.changed();
    if response.hovered() {
        let delta = take_wheel_delta(ui);
        if delta != 0.0 {
            let next = (value + delta).clamp(*range.start(), *range.end());
            if (next - value).abs() > f32::EPSILON {
                value = next;
                changed = true;
            }
        }
    }
    changed.then_some(value)
}

fn labeled_edit(ui: &mut egui::Ui, label: &str, text: &mut String, hint: &str) -> bool {
    ui.horizontal(|ui| {
        ui.add_sized(Vec2::new(LABEL_W, 18.0), egui::Label::new(label));
        let width = ui.available_width();
        ui.add(
            egui::TextEdit::singleline(text)
                .desired_width(width)
                .hint_text(hint),
        )
        .changed()
    })
    .inner
}

fn labeled_drag(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    tip: Option<&str>,
) -> bool {
    if let Some(next) = labeled_drag_value(ui, label, *value, range, tip) {
        *value = next;
        true
    } else {
        false
    }
}

fn labeled_drag_value(
    ui: &mut egui::Ui,
    label: &str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    tip: Option<&str>,
) -> Option<f32> {
    let mut changed = None;
    ui.horizontal(|ui| {
        let label = ui.add_sized(Vec2::new(22.0, 18.0), egui::Label::new(label));
        if let Some(tip) = tip {
            label.on_hover_text(tip);
        }
        if let Some(next) = drag_num(ui, value, range) {
            changed = Some(next);
        }
    });
    changed
}

fn collapsing(ui: &mut egui::Ui, title: &str, open: bool, body: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(title)
        .default_open(open)
        .id_salt(title)
        .show(ui, body);
}

fn sidebar_kind_title(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::StaticText => "Static Text",
        other => other.label(),
    }
}

fn pin_button(ui: &mut egui::Ui, pinned: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
    let color = if pinned {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    if response.hovered() || pinned {
        ui.painter().rect_filled(
            rect,
            3.0,
            ui.style().interact_selectable(&response, pinned).bg_fill,
        );
    }
    let center = rect.center() + Vec2::new(0.0, 1.0);
    ui.painter().circle_stroke(
        center + Vec2::new(0.0, -3.5),
        2.6,
        egui::Stroke::new(1.3_f32, color),
    );
    ui.painter().line_segment(
        [center + Vec2::new(0.0, -1.0), center + Vec2::new(0.0, 5.0)],
        egui::Stroke::new(1.3_f32, color),
    );
    response
}

fn info_icon(ui: &mut egui::Ui, tip: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
    let color = ui.visuals().weak_text_color();
    ui.painter()
        .circle_stroke(rect.center(), 6.0, egui::Stroke::new(1.0_f32, color));
    ui.painter().text(
        rect.center() + Vec2::new(0.0, 1.0),
        egui::Align2::CENTER_CENTER,
        "i",
        egui::FontId::proportional(11.0),
        color,
    );
    response.on_hover_text(tip)
}

fn align_icon(ui: &mut egui::Ui, align: TextAlign, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::click());
    let visuals = ui.style().interact_selectable(&response, selected);
    if selected || response.hovered() {
        ui.painter().rect_filled(rect, 3.0, visuals.bg_fill);
    }
    let color = visuals.text_color();
    let left = rect.left() + 4.0;
    let right = rect.right() - 4.0;
    let widths = [14.0, 9.0, 12.0];
    for (index, width) in widths.into_iter().enumerate() {
        let y = rect.top() + 5.0 + index as f32 * 5.0;
        let x = match align {
            TextAlign::Left => left,
            TextAlign::Center => rect.center().x - width * 0.5,
            TextAlign::Right => right - width,
        };
        ui.painter().line_segment(
            [egui::pos2(x, y), egui::pos2(x + width, y)],
            egui::Stroke::new(1.4_f32, color),
        );
    }
    response.on_hover_text(match align {
        TextAlign::Left => "Align left",
        TextAlign::Center => "Align center",
        TextAlign::Right => "Align right",
    })
}

fn color_swatch(ui: &mut egui::Ui, color: Rgb) -> Option<Rgb> {
    let mut bytes = [
        (color.r * 255.0).round().clamp(0.0, 255.0) as u8,
        (color.g * 255.0).round().clamp(0.0, 255.0) as u8,
        (color.b * 255.0).round().clamp(0.0, 255.0) as u8,
    ];
    ui.color_edit_button_srgb(&mut bytes).changed().then(|| {
        Rgb::new(
            bytes[0] as f32 / 255.0,
            bytes[1] as f32 / 255.0,
            bytes[2] as f32 / 255.0,
        )
    })
}

/// How long a press/release pair (or a line event and its pixel twin) counts
/// as the same notch. A second physical notch is slower than this.
const NOTCH_LOCK_SECS: f64 = 0.05;

/// One wheel step for this frame, then forget the scroll so nothing else moves.
///
/// A second call in the same frame returns 0. X11 delivers one notch as a
/// button press and a button release, sometimes with a pixel event as well.
fn take_wheel_delta(ui: &mut egui::Ui) -> f32 {
    let (time, delta) = ui.input(|input| {
        let (delta, _) = wheel_delta_from_events(&input.events);
        (input.time, delta)
    });
    ui.ctx().input_mut(|input| {
        input.raw_scroll_delta = egui::Vec2::ZERO;
        input.smooth_scroll_delta = egui::Vec2::ZERO;
    });
    if delta == 0.0 {
        return 0.0;
    }
    let last_id = egui::Id::new("pageform-wheel-last");
    let last = ui.data(|data| data.get_temp::<(f64, f32)>(last_id));
    if !notch_is_new(time, delta, last) {
        return 0.0;
    }
    // The properties UI can run twice in one frame. Count the notch once.
    let stamp = (time.to_bits(), delta.to_bits());
    let id = egui::Id::new("pageform-wheel-once");
    if ui.data(|data| data.get_temp::<(u64, u32)>(id)) == Some(stamp) {
        return 0.0;
    }
    ui.data_mut(|data| {
        data.insert_temp(id, stamp);
        data.insert_temp(last_id, (time, delta));
    });
    delta
}

/// Points to add for the wheel events in this frame, and whether a line event
/// supplied them.
///
/// A vertical notch and a Shift+wheel notch that egui (or the OS) turned into a
/// horizontal delta both count. Shift selects the 0.5 pt step.
///
/// On X11, winit turns buttons 4 and 5 into a line event on both press and
/// release, so one xdotool notch is two identical `Line` events. Those are one
/// step. A single event whose magnitude is greater than 1 is that many notches.
/// Pixel events that accompany a line event are the same notch.
fn wheel_delta_from_events(events: &[egui::Event]) -> (f32, bool) {
    let mut line_sign = 0.0_f32;
    let mut line_notches = 0.0_f32;
    let mut line_shift = false;
    let mut saw_line = false;
    let mut point_sign = 0.0_f32;
    let mut point_shift = false;
    for event in events {
        let egui::Event::MouseWheel {
            unit,
            delta,
            modifiers,
        } = event
        else {
            continue;
        };
        let amount = if delta.y.abs() >= delta.x.abs() {
            delta.y
        } else {
            delta.x
        };
        if amount == 0.0 {
            continue;
        }
        match unit {
            egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => {
                let sign = amount.signum();
                let mag = amount.abs();
                if !saw_line {
                    line_sign = sign;
                    line_shift = modifiers.shift;
                    // A unit line is one notch. A larger value in one event is
                    // several notches (a flick reported as a single delta).
                    line_notches = if mag <= 1.0 { 1.0 } else { mag };
                    saw_line = true;
                } else if (sign - line_sign).abs() < 0.01 && mag <= 1.0 {
                    // Press plus release, or a second copy of the same notch.
                } else {
                    line_notches += mag.max(1.0);
                }
            }
            egui::MouseWheelUnit::Point => {
                point_sign = amount.signum();
                point_shift = modifiers.shift;
            }
        }
    }
    if saw_line {
        let step = if line_shift { 0.5 } else { 1.0 };
        (line_sign * step * line_notches, true)
    } else if point_sign != 0.0 {
        let step = if point_shift { 0.5 } else { 1.0 };
        (point_sign * step, false)
    } else {
        (0.0, false)
    }
}

/// False when `delta` is the rest of a notch already accepted at `last`.
fn notch_is_new(now: f64, delta: f32, last: Option<(f64, f32)>) -> bool {
    if delta == 0.0 {
        return false;
    }
    let Some((then, previous)) = last else {
        return true;
    };
    if previous.signum() == delta.signum() && (now - then).abs() < NOTCH_LOCK_SECS {
        return false;
    }
    true
}

fn bump_sidebar_fonts(ui: &mut egui::Ui) {
    let styles = &mut ui.style_mut().text_styles;
    for (kind, size) in [
        (egui::TextStyle::Body, 15.5),
        (egui::TextStyle::Button, 15.5),
        (egui::TextStyle::Small, 13.5),
        (egui::TextStyle::Monospace, 15.0),
    ] {
        if let Some(font) = styles.get_mut(&kind) {
            font.size = size;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wheel(delta: egui::Vec2, shift: bool) -> egui::Event {
        let modifiers = egui::Modifiers {
            shift,
            ..Default::default()
        };
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta,
            modifiers,
        }
    }

    #[test]
    fn wheel_steps_are_one_point_and_shift_is_half_on_either_axis() {
        assert_eq!(
            wheel_delta_from_events(&[wheel(egui::vec2(0.0, 1.0), false)]).0,
            1.0
        );
        assert_eq!(
            wheel_delta_from_events(&[wheel(egui::vec2(0.0, -1.0), false)]).0,
            -1.0
        );
        // egui and some OSes turn Shift+wheel into a horizontal delta.
        assert_eq!(
            wheel_delta_from_events(&[wheel(egui::vec2(1.0, 0.0), true)]).0,
            0.5
        );
        assert_eq!(
            wheel_delta_from_events(&[wheel(egui::vec2(0.0, -2.0), true)]).0,
            -1.0
        );
        assert_eq!(
            wheel_delta_from_events(&[egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 14.0),
                modifiers: egui::Modifiers::default(),
            }])
            .0,
            1.0
        );
        // One physical notch is often both a line event and a pixel event.
        let doubled = [
            wheel(egui::vec2(0.0, 1.0), false),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 15.0),
                modifiers: egui::Modifiers::default(),
            },
        ];
        let (delta, from_line) = wheel_delta_from_events(&doubled);
        assert_eq!(delta, 1.0);
        assert!(from_line);
        // Two pixel events in the same frame are still one notch.
        let pixels = [
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 15.0),
                modifiers: egui::Modifiers::default(),
            },
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 15.0),
                modifiers: egui::Modifiers::default(),
            },
        ];
        assert_eq!(wheel_delta_from_events(&pixels).0, 1.0);
        // X11 / xdotool: button 4 press and release are two line events.
        let x11_press_release = [
            wheel(egui::vec2(0.0, 1.0), false),
            wheel(egui::vec2(0.0, 1.0), false),
        ];
        assert_eq!(wheel_delta_from_events(&x11_press_release).0, 1.0);
        let x11_shift = [
            wheel(egui::vec2(0.0, 1.0), true),
            wheel(egui::vec2(0.0, 1.0), true),
        ];
        assert_eq!(wheel_delta_from_events(&x11_shift).0, 0.5);
        let x11_down = [
            wheel(egui::vec2(0.0, -1.0), false),
            wheel(egui::vec2(0.0, -1.0), false),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -15.0),
                modifiers: egui::Modifiers::default(),
            },
        ];
        assert_eq!(wheel_delta_from_events(&x11_down).0, -1.0);
        // The release can land in the next frame.
        assert!(notch_is_new(1.0, 1.0, None));
        assert!(!notch_is_new(1.016, 1.0, Some((1.0, 1.0))));
        assert!(notch_is_new(1.016, -0.5, Some((1.0, 1.0))));
        assert!(notch_is_new(1.08, 1.0, Some((1.0, 1.0))));
    }

    #[test]
    fn hovered_font_size_wheel_changes_the_value_and_consumes_scroll() {
        let ctx = egui::Context::default();
        let mut value = 12.0_f32;
        let mut time = 0.0_f64;
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0));
        let input = |time: f64, events: Vec<egui::Event>| egui::RawInput {
            screen_rect: Some(screen),
            time: Some(time),
            events,
            ..Default::default()
        };
        let show = |ctx: &egui::Context, value: &mut f32| {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.set_min_height(2000.0);
                    if let Some(next) = drag_pt_wheel(ui, "Size", *value, 4.0..=96.0) {
                        *value = next;
                    }
                });
            });
        };

        time += 1.0;
        let placed = ctx.run(input(time, vec![]), |ctx| show(ctx, &mut value));
        let at = galley_center(&placed, "12.0").or_else(|| galley_center(&placed, "12"));
        let at = at.expect("font size value");

        time += 1.0;
        let _ = ctx.run(
            input(
                time,
                vec![
                    egui::Event::PointerMoved(at),
                    wheel(egui::vec2(0.0, 1.0), false),
                ],
            ),
            |ctx| show(ctx, &mut value),
        );
        assert_eq!(value, 13.0, "wheel up adds 1 pt");
        time += 0.016;
        let _ = ctx.run(
            input(
                time,
                vec![
                    egui::Event::PointerMoved(at),
                    wheel(egui::vec2(0.0, 1.0), false),
                ],
            ),
            |ctx| show(ctx, &mut value),
        );
        assert_eq!(value, 13.0, "the button release is not a second notch");
        time += 1.0;
        let _ = ctx.run(
            input(
                time,
                vec![
                    egui::Event::PointerMoved(at),
                    wheel(egui::vec2(0.0, 1.0), false),
                    wheel(egui::vec2(0.0, 1.0), false),
                ],
            ),
            |ctx| show(ctx, &mut value),
        );
        assert_eq!(value, 14.0, "press and release in one frame add 1 pt");
        time += 0.02;
        let _ = ctx.run(
            input(
                time,
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, 15.0),
                        modifiers: egui::Modifiers::default(),
                    },
                ],
            ),
            |ctx| show(ctx, &mut value),
        );
        assert_eq!(
            value, 14.0,
            "a pixel event just after a line event is the same notch"
        );
        assert_eq!(
            ctx.input(|input| input.raw_scroll_delta),
            egui::Vec2::ZERO,
            "the notch must not remain for the canvas to zoom"
        );
        assert_eq!(
            ctx.input(|input| input.smooth_scroll_delta),
            egui::Vec2::ZERO,
            "the notch must not remain for the sidebar to scroll"
        );

        time += 1.0;
        let _ = ctx.run(
            input(
                time,
                vec![
                    egui::Event::PointerMoved(at),
                    wheel(egui::vec2(-1.0, 0.0), true),
                ],
            ),
            |ctx| show(ctx, &mut value),
        );
        assert_eq!(value, 13.5, "Shift+horizontal wheel subtracts 0.5 pt");

        value = 96.0;
        time += 1.0;
        let _ = ctx.run(
            input(
                time,
                vec![
                    egui::Event::PointerMoved(at),
                    wheel(egui::vec2(0.0, 3.0), false),
                ],
            ),
            |ctx| show(ctx, &mut value),
        );
        assert_eq!(value, 96.0, "font size stays inside 4..=96");

        value = 0.0;
        time += 1.0;
        let _ = ctx.run(input(time, vec![]), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if let Some(next) = drag_pt_wheel(ui, "Width", value, 0.0..=24.0) {
                    value = next;
                }
            });
        });
        // The width field is in a different place. Re-find it next frame.
        time += 1.0;
        let placed = ctx.run(input(time, vec![]), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = drag_pt_wheel(ui, "Width", value, 0.0..=24.0);
            });
        });
        let width_at = galley_center(&placed, "0.0").or_else(|| galley_center(&placed, "0"));
        let width_at = width_at.expect("border width value");
        time += 1.0;
        let _ = ctx.run(
            input(
                time,
                vec![
                    egui::Event::PointerMoved(width_at),
                    wheel(egui::vec2(0.0, -1.0), false),
                ],
            ),
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if let Some(next) = drag_pt_wheel(ui, "Width", value, 0.0..=24.0) {
                        value = next;
                    }
                });
            },
        );
        assert_eq!(value, 0.0, "border width does not go below 0");
    }

    #[test]
    fn stroke_width_and_size_drag_values_follow_the_wheel() {
        let ctx = egui::Context::default();
        let mut width = 1.5_f32;
        let mut height = 18.0_f32;
        let mut time = 0.0_f64;
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0));
        let input = |time: f64, events: Vec<egui::Event>| egui::RawInput {
            screen_rect: Some(screen),
            time: Some(time),
            events,
            ..Default::default()
        };
        let show = |ctx: &egui::Context, width: &mut f32, height: &mut f32| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if let Some(next) = drag_pt(ui, "Width", *width, 0.0..=24.0) {
                    *width = next;
                }
                if let Some(next) = drag_pt(ui, "H", *height, 1.0..=2000.0) {
                    *height = next;
                }
            });
        };
        time += 1.0;
        let placed = ctx.run(input(time, vec![]), |ctx| {
            show(ctx, &mut width, &mut height)
        });
        let width_at = galley_center(&placed, "1.5").expect("stroke width");
        let height_at = galley_center(&placed, "18.0")
            .or_else(|| galley_center(&placed, "18"))
            .expect("height");
        time += 1.0;
        let _ = ctx.run(
            input(
                time,
                vec![
                    egui::Event::PointerMoved(width_at),
                    wheel(egui::vec2(0.0, 1.0), false),
                ],
            ),
            |ctx| show(ctx, &mut width, &mut height),
        );
        assert_eq!(width, 2.5, "wheel up increases stroke width");
        assert_eq!(height, 18.0, "only the hovered value changes");
        time += 1.0;
        let _ = ctx.run(
            input(
                time,
                vec![
                    egui::Event::PointerMoved(width_at),
                    wheel(egui::vec2(0.0, -1.0), false),
                ],
            ),
            |ctx| show(ctx, &mut width, &mut height),
        );
        assert_eq!(width, 1.5, "wheel down decreases stroke width");
        time += 1.0;
        let _ = ctx.run(
            input(
                time,
                vec![
                    egui::Event::PointerMoved(height_at),
                    wheel(egui::vec2(0.0, 1.0), false),
                ],
            ),
            |ctx| show(ctx, &mut width, &mut height),
        );
        assert_eq!(height, 19.0, "wheel up increases a size field");
        assert_eq!(width, 1.5);
    }

    fn galley_center(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
        fn walk(shape: &egui::epaint::Shape, label: &str) -> Option<egui::Pos2> {
            match shape {
                egui::epaint::Shape::Text(text) if text.galley.text().trim() == label => {
                    Some(text.pos + egui::vec2(4.0, text.galley.size().y * 0.5))
                }
                egui::epaint::Shape::Vec(shapes) => {
                    shapes.iter().find_map(|shape| walk(shape, label))
                }
                _ => None,
            }
        }
        output
            .shapes
            .iter()
            .find_map(|clipped| walk(&clipped.shape, label))
    }
}
