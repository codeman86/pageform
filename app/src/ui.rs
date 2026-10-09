//! egui canvas. Geometry and PDF export live in `pageform-core`.

use eframe::egui::{
    self, Align2, CursorIcon, FontId, Key, PointerButton, Pos2, Rect, Sense, Stroke, Vec2,
};
use pageform_core::{
    default_size, export_pdf, hit_handle, move_rect, place_default, print_safe_rect,
    rect_from_drag, resize_rect, snap_point, Document, FieldId, FieldKind, GridSize, Handle, Page,
    PdfPoint, RectPt, ScreenRect, ViewTransform, DATE_HINT, PRINT_SAFE_MARGIN_PT, TABLE_COLUMNS,
    TABLE_ROWS,
};

pub fn run_gui(demo: bool) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 840.0])
            .with_min_inner_size([860.0, 640.0])
            .with_title("PageForm"),
        ..Default::default()
    };
    eframe::run_native(
        "PageForm",
        options,
        Box::new(move |cc| {
            let app = PageFormApp::new(demo, cc.storage);
            crate::theme::apply(&cc.egui_ctx, app.dark);
            Ok(Box::new(app))
        }),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Select,
    Text,
    StaticText,
    Checkbox,
    Radio,
    Table,
    Signature,
    Date,
    Dropdown,
    Line,
    Rectangle,
    TextBox,
    Image,
}

impl Tool {
    fn place_kind(self) -> Option<FieldKind> {
        match self {
            Tool::Select => None,
            Tool::Text => Some(FieldKind::Text),
            Tool::StaticText => Some(FieldKind::StaticText),
            Tool::Checkbox => Some(FieldKind::Checkbox),
            Tool::Radio => Some(FieldKind::Radio),
            Tool::Table => Some(FieldKind::Table),
            Tool::Signature => Some(FieldKind::Signature),
            Tool::Date => Some(FieldKind::Date),
            Tool::Dropdown => Some(FieldKind::Dropdown),
            Tool::Line => Some(FieldKind::Line),
            Tool::Rectangle => Some(FieldKind::Rectangle),
            Tool::TextBox => Some(FieldKind::TextBox),
            Tool::Image => Some(FieldKind::Image),
        }
    }

    fn for_kind(kind: FieldKind) -> Self {
        match kind {
            FieldKind::Text => Tool::Text,
            FieldKind::StaticText => Tool::StaticText,
            FieldKind::Checkbox => Tool::Checkbox,
            FieldKind::Radio => Tool::Radio,
            FieldKind::Table => Tool::Table,
            FieldKind::Signature => Tool::Signature,
            FieldKind::Date => Tool::Date,
            FieldKind::Dropdown => Tool::Dropdown,
            FieldKind::Line => Tool::Line,
            FieldKind::Rectangle => Tool::Rectangle,
            FieldKind::TextBox => Tool::TextBox,
            FieldKind::Image => Tool::Image,
        }
    }
}

#[derive(Clone, Copy)]
enum Drag {
    Pan,
    Place {
        start: PdfPoint,
        kind: FieldKind,
    },
    Move {
        id: FieldId,
        grab_x: f32,
        grab_y: f32,
    },
    Resize {
        id: FieldId,
        handle: Handle,
        origin: RectPt,
    },
    LineEnd {
        id: FieldId,
        index: usize,
    },
}

struct PageFormApp {
    doc: Document,
    tool: Tool,
    selection: Option<FieldId>,
    dots_visible: bool,
    guides_visible: bool,
    margins_visible: bool,
    header_visible: bool,
    footer_visible: bool,
    page_numbers_visible: bool,
    grid_size: GridSize,
    snap: bool,
    view: ViewTransform,
    fitted: bool,
    canvas_rect: Option<Rect>,
    drag: Option<Drag>,
    name_buf: String,
    caption_buf: String,
    options_buf: String,
    name_for: Option<FieldId>,
    name_error: Option<String>,
    export_path: String,
    status: String,
    dark: bool,
    persist_theme: bool,
}

impl PageFormApp {
    fn new(demo: bool, storage: Option<&dyn eframe::Storage>) -> Self {
        let doc = if demo {
            Document::sample()
        } else {
            Document::letter()
        };
        let page = doc.page();
        let stored = storage.and_then(|storage| storage.get_string(crate::theme::STORAGE_KEY));
        Self {
            view: ViewTransform::new(page.height),
            doc,
            tool: Tool::Select,
            selection: None,
            dots_visible: true,
            guides_visible: true,
            margins_visible: true,
            header_visible: false,
            footer_visible: false,
            page_numbers_visible: false,
            grid_size: GridSize::Medium,
            snap: true,
            fitted: false,
            canvas_rect: None,
            drag: None,
            name_buf: String::new(),
            caption_buf: String::new(),
            options_buf: String::new(),
            name_for: None,
            name_error: None,
            export_path: "sample-form.pdf".to_string(),
            status: if demo {
                "Sample page with full_name and agree".to_string()
            } else {
                "Blank US Letter page".to_string()
            },
            dark: crate::theme::is_dark(stored.as_deref()),
            persist_theme: false,
        }
    }

    /// Snap spacing is the minor step of the active preset, hidden grid included.
    fn snap_grid(&self) -> Option<f32> {
        self.snap.then(|| self.grid_size.minor_pt())
    }

    fn min_size(&self) -> f32 {
        if self.snap {
            self.grid_size.minor_pt()
        } else {
            9.0
        }
    }

    fn pointer_pdf(&self, ctx: &egui::Context) -> Option<PdfPoint> {
        let pos = ctx.input(|input| input.pointer.latest_pos())?;
        Some(self.view.screen_to_pdf(pos.x, pos.y))
    }

    fn on_page(&self, point: PdfPoint) -> bool {
        let page = self.doc.page();
        point.x >= 0.0 && point.y >= 0.0 && point.x <= page.width && point.y <= page.height
    }

    fn place_rect(&self, start: PdfPoint, current: PdfPoint, kind: FieldKind) -> RectPt {
        let (x0, y0) = self.view.pdf_to_screen(start);
        let (x1, y1) = self.view.pdf_to_screen(current);
        let dx = x0 - x1;
        let dy = y0 - y1;
        let page = self.doc.page();
        if dx * dx + dy * dy < 16.0 {
            let (w, h) = default_size(kind, self.grid_size);
            place_default(start, w, h, self.snap_grid(), &page)
        } else {
            rect_from_drag(start, current, self.snap_grid(), self.min_size(), &page)
        }
    }

    fn export_current(&mut self) {
        if self.export_path.trim().is_empty() {
            self.status = "Enter a file path to export".to_string();
            return;
        }
        match export_pdf(&self.doc) {
            Ok(bytes) => match std::fs::write(&self.export_path, &bytes) {
                Ok(()) => {
                    self.status = format!("Exported {} ({} bytes)", self.export_path, bytes.len());
                }
                Err(err) => self.status = format!("Could not write {}: {err}", self.export_path),
            },
            Err(err) => self.status = format!("Export failed: {err}"),
        }
    }

    fn set_dark(&mut self, dark: bool) {
        if self.dark != dark {
            self.dark = dark;
            self.persist_theme = true;
        }
    }

    fn delete_selection(&mut self) {
        let Some(id) = self.selection else {
            return;
        };
        if self.doc.remove(id) {
            self.status = "Deleted field".to_string();
        }
        self.selection = None;
        self.drag = None;
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| self.file_menu(ui));
            ui.menu_button("Edit", |ui| self.edit_menu(ui));
            ui.menu_button("Insert", |ui| self.insert_menu(ui));
            ui.menu_button("View", |ui| self.view_menu(ui));
        });
    }

    fn file_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(200.0);
        let export = ui.button("Export PDF").on_hover_text(
            "Write a fillable PDF to the path in the toolbar. Guides and margins stay on the canvas.",
        );
        if export.clicked() {
            self.export_current();
            ui.close();
        }
        ui.separator();
        if ui.button(quit_label()).clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn edit_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(200.0);
        if menu_command(ui, "Select", Some("Esc"), self.tool == Tool::Select) {
            self.tool = Tool::Select;
        }
        ui.separator();
        let delete = ui.add_enabled(
            self.selection.is_some(),
            egui::Button::new("Delete").shortcut_text("Del"),
        );
        if delete.clicked() {
            self.delete_selection();
            ui.close();
        }
        ui.separator();
        ui.menu_button("Preferences", |ui| self.preferences_menu(ui));
    }

    fn preferences_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(240.0);
        let mut show = self.doc.radio_labels();
        if ui
            .checkbox(&mut show, "Radio labels")
            .on_hover_text(
                "Text beside each radio button. Off hides those labels on the canvas and in the export.",
            )
            .changed()
        {
            self.doc.set_radio_labels(show);
            self.status = if show {
                "Radio labels on".to_string()
            } else {
                "Radio labels off".to_string()
            };
        }
    }

    fn insert_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(240.0);
        for kind in FieldKind::INSERT {
            let button = ui.button(kind.label()).on_hover_text(insert_tip(kind));
            if button.clicked() {
                self.insert_at_view(kind);
                ui.close();
            }
        }
    }

    /// Insert menu drops a field. The toolbar tools stay armed for drawing.
    fn insert_at_view(&mut self, kind: FieldKind) {
        let rect = self.default_insert_rect(kind);
        let id = self.place_kind(kind, rect);
        self.selection = Some(id);
        self.drag = None;
        self.status = format!("Placed {}", kind.label());
    }

    /// Menu and toolbar share this. A selected radio makes the next radio join its group.
    fn place_kind(&mut self, kind: FieldKind, rect: RectPt) -> FieldId {
        if kind == FieldKind::Radio {
            let group = self.selected_radio_group();
            self.doc.add_radio(rect, group.as_deref())
        } else {
            self.doc.add_field(kind, rect)
        }
    }

    fn selected_radio_group(&self) -> Option<String> {
        self.selection.and_then(|id| {
            self.doc.field(id).and_then(|field| {
                (field.kind() == FieldKind::Radio).then(|| field.name().to_string())
            })
        })
    }

    /// Default size, centered on the visible page, snapped when snap is on.
    /// A later insert steps down, then right, so it does not cover the last one.
    fn default_insert_rect(&self, kind: FieldKind) -> RectPt {
        let (w, h) = default_size(kind, self.grid_size);
        let page = self.doc.page();
        let grid = self.snap_grid();
        let first = anchor_for_center(self.visible_page_center(), w, h);
        let mut anchor = first;
        let mut placed = place_default(first, w, h, grid, &page);
        for _ in 0..48 {
            placed = place_default(anchor, w, h, grid, &page);
            if !self
                .doc
                .fields()
                .iter()
                .any(|field| rects_overlap(field.rect(), placed))
            {
                return placed;
            }
            let below = PdfPoint {
                x: placed.x,
                y: placed.y,
            };
            let stepped = place_default(below, w, h, grid, &page);
            if same_rect(stepped, placed) {
                let right = PdfPoint {
                    x: placed.right(),
                    y: first.y,
                };
                let stepped_right = place_default(right, w, h, grid, &page);
                if same_rect(stepped_right, placed) {
                    return placed;
                }
                anchor = right;
            } else {
                anchor = below;
            }
        }
        placed
    }

    /// Center of the page area currently inside the canvas. The page center if
    /// the canvas is not ready yet or the page is panned fully out of view.
    fn visible_page_center(&self) -> PdfPoint {
        let page = self.doc.page();
        let page_center = PdfPoint {
            x: page.width * 0.5,
            y: page.height * 0.5,
        };
        let Some(canvas) = self.canvas_rect else {
            return page_center;
        };
        let screen = self.view.pdf_rect_to_screen(RectPt {
            x: 0.0,
            y: 0.0,
            w: page.width,
            h: page.height,
        });
        let left = canvas.left().max(screen.x);
        let right = canvas.right().min(screen.x + screen.w);
        let top = canvas.top().max(screen.y);
        let bottom = canvas.bottom().min(screen.y + screen.h);
        if right - left < 1.0 || bottom - top < 1.0 {
            return page_center;
        }
        self.view
            .screen_to_pdf((left + right) * 0.5, (top + bottom) * 0.5)
    }

    fn view_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(200.0);
        ui.checkbox(&mut self.dots_visible, "Grid dots");
        ui.checkbox(&mut self.guides_visible, "Guide lines");
        ui.checkbox(&mut self.margins_visible, "Print-safe margin");
        ui.checkbox(&mut self.header_visible, "Header")
            .on_hover_text("Header band on the canvas. Not written into the PDF.");
        ui.checkbox(&mut self.footer_visible, "Footer")
            .on_hover_text("Footer band on the canvas. Not written into the PDF.");
        ui.checkbox(&mut self.page_numbers_visible, "Page numbers")
            .on_hover_text(
                "Page number on the canvas. This page reads 1. Not written into the PDF.",
            );
        ui.checkbox(&mut self.snap, "Snap");
        ui.separator();
        ui.label("Grid size");
        for size in GridSize::ALL {
            ui.radio_value(&mut self.grid_size, size, size.label());
        }
        ui.separator();
        ui.label("Theme");
        let before = self.dark;
        ui.radio_value(&mut self.dark, true, "Dark");
        ui.radio_value(&mut self.dark, false, "Light");
        if self.dark != before {
            self.persist_theme = true;
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.tool, Tool::Select, "Select");
            for kind in FieldKind::INSERT {
                ui.selectable_value(&mut self.tool, Tool::for_kind(kind), kind.label())
                    .on_hover_text(toolbar_tip(kind));
            }
            ui.separator();
            ui.toggle_value(&mut self.dots_visible, "Dots")
                .on_hover_text("Grid dots");
            ui.toggle_value(&mut self.guides_visible, "Guides")
                .on_hover_text("Guide lines. Canvas only.");
            ui.toggle_value(&mut self.margins_visible, "Margins")
                .on_hover_text("Print-safe margin. Canvas only.");
            ui.toggle_value(&mut self.header_visible, "Header")
                .on_hover_text("Header band. Canvas only.");
            ui.toggle_value(&mut self.footer_visible, "Footer")
                .on_hover_text("Footer band. Canvas only.");
            ui.toggle_value(&mut self.page_numbers_visible, "Page #")
                .on_hover_text("Page number. Canvas only.");
            for size in GridSize::ALL {
                ui.selectable_value(&mut self.grid_size, size, size.label());
            }
            ui.toggle_value(&mut self.snap, "Snap");
            ui.label(self.grid_size.minor_label());
            ui.separator();
            if ui.button("Fit").clicked() {
                if let Some(rect) = self.canvas_rect {
                    self.fit(rect);
                }
            }
            ui.label(format!("{}%", (self.view.zoom * 100.0).round() as i32));
            ui.separator();
            ui.label("Export");
            ui.add(
                egui::TextEdit::singleline(&mut self.export_path)
                    .desired_width(160.0)
                    .hint_text("sample-form.pdf"),
            );
            if ui.button("Export PDF").clicked() {
                self.export_current();
            }
            ui.separator();
            if ui
                .button(crate::theme::toggle_label(self.dark))
                .on_hover_text(crate::theme::toggle_tip(self.dark))
                .clicked()
            {
                self.set_dark(!self.dark);
            }
        });
    }

    fn properties(&mut self, ui: &mut egui::Ui) {
        ui.heading("Page");
        let page = self.doc.page();
        ui.label(format!(
            "US Letter · {} × {} pt",
            page.width as i32, page.height as i32
        ));
        ui.label("One page. Additional pages are a later milestone.");
        ui.add_space(8.0);
        ui.label(format!(
            "{} grid · minor {:.1} pt ({}) · major {:.0} pt",
            self.grid_size.label(),
            self.grid_size.minor_pt(),
            self.grid_size.minor_label(),
            self.grid_size.major_pt()
        ));
        ui.label("Dots cover the whole page. Guide lines, the print-safe outline, header, footer, and page number are canvas only and are not exported.");
        ui.label(format!(
            "Print safe area is inset {:.0} pt (0.5 in).",
            PRINT_SAFE_MARGIN_PT
        ));
        ui.label("Snap uses the minor spacing, even if dots or guides are hidden, so columns and rows stay even.");
        ui.label("Insert drops a default-sized item in the view. The matching toolbar tool draws on the page.");
        ui.label("Edit → Preferences turns radio labels on or off.");
        ui.separator();
        ui.heading("Field");

        if self.name_for != self.selection {
            self.name_for = self.selection;
            self.name_buf = self
                .selection
                .and_then(|id| self.doc.field(id).map(|field| field.name().to_string()))
                .unwrap_or_default();
            self.caption_buf = self
                .selection
                .and_then(|id| self.doc.field(id).map(|field| field.caption().to_string()))
                .unwrap_or_default();
            self.options_buf = self
                .selection
                .and_then(|id| self.doc.field(id).map(|field| field.options().join(", ")))
                .unwrap_or_default();
            self.name_error = None;
        }

        let Some(id) = self.selection else {
            ui.label("Nothing selected.");
            ui.label("Insert drops a default item. A toolbar tool draws on the page.");
            return;
        };
        let (kind, rect, group_name) = {
            let Some(field) = self.doc.field(id) else {
                self.selection = None;
                return;
            };
            (field.kind(), field.rect(), field.name().to_string())
        };
        ui.label(format!("Type  {}", kind.label()));
        ui.label(kind_note(kind));
        if kind.is_form_field() {
            let name_label = if kind == FieldKind::Radio {
                "Group"
            } else {
                "Name"
            };
            ui.label(name_label);
            let edit = ui.add(egui::TextEdit::singleline(&mut self.name_buf).desired_width(200.0));
            if edit.changed() {
                self.name_error = self
                    .doc
                    .set_name(id, &self.name_buf)
                    .err()
                    .map(|err| err.to_string());
            }
        }
        if matches!(
            kind,
            FieldKind::StaticText | FieldKind::TextBox | FieldKind::Radio
        ) {
            ui.label(if kind == FieldKind::Radio {
                "Label"
            } else {
                "Text"
            });
            let edit =
                ui.add(egui::TextEdit::singleline(&mut self.caption_buf).desired_width(200.0));
            if edit.changed() {
                self.name_error = self
                    .doc
                    .set_caption(id, &self.caption_buf)
                    .err()
                    .map(|err| err.to_string());
            }
            if kind == FieldKind::Radio && !self.doc.radio_labels() {
                ui.label("Hidden until Edit → Preferences → Radio labels is on.");
            }
        }
        if kind == FieldKind::Dropdown {
            ui.label("Options");
            let edit =
                ui.add(egui::TextEdit::singleline(&mut self.options_buf).desired_width(200.0));
            if edit.changed() {
                let options = self
                    .options_buf
                    .split(',')
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
        if kind == FieldKind::Radio {
            let members: Vec<(String, String)> = self
                .doc
                .fields()
                .iter()
                .filter(|member| member.kind() == FieldKind::Radio && member.name() == group_name)
                .map(|member| (member.on_state().to_string(), member.caption().to_string()))
                .collect();
            ui.add_space(4.0);
            ui.label("Options in this group");
            for (state, caption) in &members {
                ui.label(format!("{state}  {caption}"));
            }
            if ui.button("Add option to group").clicked() {
                self.insert_at_view(FieldKind::Radio);
            }
        }
        if let Some(error) = &self.name_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.add_space(6.0);
        ui.label(format!("X  {} pt", fmt_pt(rect.x)));
        ui.label(format!("Y  {} pt from bottom", fmt_pt(rect.y)));
        ui.label(format!("W  {} pt", fmt_pt(rect.w)));
        ui.label(format!("H  {} pt", fmt_pt(rect.h)));
        ui.add_space(8.0);
        ui.label("Delete removes the selected item.");
    }

    fn fit(&mut self, rect: Rect) {
        let margin = 28.0;
        let page = self.doc.page();
        let avail_w = (rect.width() - margin * 2.0).max(1.0);
        let avail_h = (rect.height() - margin * 2.0).max(1.0);
        let zoom = (avail_w / page.width)
            .min(avail_h / page.height)
            .clamp(0.2, 8.0);
        self.view.zoom = zoom;
        self.view.page_height = page.height;
        let pw = page.width * zoom;
        let ph = page.height * zoom;
        self.view.origin_x = rect.center().x - pw * 0.5;
        self.view.origin_y = rect.center().y - ph * 0.5;
        self.fitted = true;
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        self.canvas_rect = Some(response.rect);
        if !self.fitted {
            self.fit(response.rect);
        }
        self.handle_input(&response, ui.ctx());
        self.paint(&painter, response.rect);
        self.hover_cursor(&response, ui.ctx());
    }

    fn handle_input(&mut self, response: &egui::Response, ctx: &egui::Context) {
        if response.hovered() {
            let scroll_y = ctx.input(|input| input.raw_scroll_delta.y);
            if scroll_y.abs() > 0.0 {
                if let Some(pos) = ctx.input(|input| input.pointer.hover_pos()) {
                    let factor = if scroll_y > 0.0 { 1.1 } else { 1.0 / 1.1 };
                    let zoom = (self.view.zoom * factor).clamp(0.2, 8.0);
                    self.view.zoom_at(pos.x, pos.y, zoom);
                }
            }
        }

        if !ctx.wants_keyboard_input() {
            if ctx.input(|input| input.key_pressed(Key::Escape)) {
                self.drag = None;
                self.tool = Tool::Select;
            }
            if ctx
                .input(|input| input.key_pressed(Key::Delete) || input.key_pressed(Key::Backspace))
            {
                self.delete_selection();
            }
        }

        let primary_pressed = ctx.input(|input| input.pointer.primary_pressed());
        let primary_down = ctx.input(|input| input.pointer.primary_down());
        let primary_released = ctx.input(|input| input.pointer.primary_released());
        let middle_pressed = ctx.input(|input| input.pointer.button_pressed(PointerButton::Middle));
        let space = ctx.input(|input| input.key_down(Key::Space));

        if middle_pressed && response.hovered() {
            self.drag = Some(Drag::Pan);
        } else if primary_pressed && response.hovered() {
            if space {
                self.drag = Some(Drag::Pan);
            } else {
                self.begin_primary(ctx);
            }
        }

        if matches!(self.drag, Some(Drag::Pan)) {
            let panning = ctx.input(|input| {
                input.pointer.button_down(PointerButton::Middle)
                    || (input.key_down(Key::Space) && input.pointer.primary_down())
            });
            if panning {
                let delta = ctx.input(|input| input.pointer.delta());
                self.view.origin_x += delta.x;
                self.view.origin_y += delta.y;
            } else {
                self.drag = None;
            }
        }

        if primary_down {
            self.update_drag(ctx);
        }

        if primary_released {
            if let Some(Drag::Place { start, kind }) = self.drag {
                if let Some(current) = self.pointer_pdf(ctx) {
                    let id = self.finish_place(start, current, kind);
                    self.selection = Some(id);
                    self.tool = Tool::Select;
                    self.status = format!("Placed {}", kind.label());
                }
            }
            if !ctx.input(|input| input.pointer.button_down(PointerButton::Middle)) {
                self.drag = None;
            }
        }
    }

    fn begin_primary(&mut self, ctx: &egui::Context) {
        let Some(pdf) = self.pointer_pdf(ctx) else {
            return;
        };
        match self.tool {
            Tool::Select => {
                let radius = 8.0 / self.view.zoom;
                if let Some(id) = self.selection {
                    if let Some(field) = self.doc.field(id) {
                        if let Some(index) = field.hit_line_end(pdf, radius) {
                            self.drag = Some(Drag::LineEnd { id, index });
                            return;
                        }
                        if field.line_ends().is_none() {
                            if let Some(handle) = hit_handle(field.rect(), pdf, radius) {
                                self.drag = Some(Drag::Resize {
                                    id,
                                    handle,
                                    origin: field.rect(),
                                });
                                return;
                            }
                        }
                    }
                }
                if let Some(field) = self
                    .doc
                    .fields()
                    .iter()
                    .rev()
                    .find(|field| field.contains_point(pdf, radius))
                {
                    let id = field.id();
                    let rect = field.rect();
                    self.selection = Some(id);
                    self.drag = Some(Drag::Move {
                        id,
                        grab_x: pdf.x - rect.x,
                        grab_y: pdf.y - rect.y,
                    });
                } else {
                    self.selection = None;
                    self.drag = None;
                }
            }
            _ => {
                if !self.on_page(pdf) {
                    return;
                }
                if let Some(kind) = self.tool.place_kind() {
                    self.drag = Some(Drag::Place { start: pdf, kind });
                }
            }
        }
    }

    fn finish_place(&mut self, start: PdfPoint, current: PdfPoint, kind: FieldKind) -> FieldId {
        if kind == FieldKind::Line {
            let (x0, y0) = self.view.pdf_to_screen(start);
            let (x1, y1) = self.view.pdf_to_screen(current);
            let dx = x0 - x1;
            let dy = y0 - y1;
            if dx * dx + dy * dy >= 16.0 {
                let grid = self.snap_grid();
                return self
                    .doc
                    .add_line(snap_point(start, grid), snap_point(current, grid));
            }
        }
        let rect = self.place_rect(start, current, kind);
        self.place_kind(kind, rect)
    }

    fn update_drag(&mut self, ctx: &egui::Context) {
        let Some(pdf) = self.pointer_pdf(ctx) else {
            return;
        };
        let page = self.doc.page();
        let grid = self.snap_grid();
        match self.drag {
            Some(Drag::Move { id, grab_x, grab_y }) => {
                if let Some(field) = self.doc.field(id) {
                    let rect = move_rect(field.rect(), pdf, grab_x, grab_y, grid, &page);
                    self.doc.update_rect(id, rect);
                }
            }
            Some(Drag::Resize { id, handle, origin }) => {
                let rect = resize_rect(origin, handle, pdf, grid, self.min_size(), &page);
                self.doc.update_rect(id, rect);
            }
            Some(Drag::LineEnd { id, index }) => {
                self.doc.set_line_end(id, index, snap_point(pdf, grid));
            }
            _ => {}
        }
    }

    fn paint(&self, painter: &egui::Painter, canvas: Rect) {
        let colors = crate::theme::canvas_colors(self.dark);
        painter.rect_filled(canvas, 0.0, colors.desk);
        let page = self.doc.page();
        let page_rect = to_egui(self.view.pdf_rect_to_screen(RectPt {
            x: 0.0,
            y: 0.0,
            w: page.width,
            h: page.height,
        }));
        let shadow = page_rect.translate(Vec2::new(3.0, 4.0));
        painter.rect_filled(shadow, 0.0, colors.shadow);
        painter.rect_filled(page_rect, 0.0, colors.page);

        if self.guides_visible {
            self.paint_guides(painter, page, page_rect, &colors);
        }
        if self.dots_visible {
            self.paint_dots(painter, page, &colors);
        }

        for field in self.doc.fields() {
            self.paint_field(painter, field, &colors);
        }

        if let Some(Drag::Place { start, kind }) = self.drag {
            if let Some(current) = self.pointer_pdf_from_painter(painter) {
                if kind == FieldKind::Line {
                    let (x0, y0) = self.view.pdf_to_screen(start);
                    let (x1, y1) = self.view.pdf_to_screen(current);
                    painter.line_segment(
                        [Pos2::new(x0, y0), Pos2::new(x1, y1)],
                        Stroke::new(1.5_f32, colors.selection),
                    );
                } else {
                    let rect = self.place_rect(start, current, kind);
                    let screen = to_egui(self.view.pdf_rect_to_screen(rect));
                    stroke_rect(painter, screen, Stroke::new(1.5_f32, colors.selection));
                }
            }
        }

        if let Some(id) = self.selection {
            if let Some(field) = self.doc.field(id) {
                if let Some((start, end)) = field.line_ends() {
                    for point in [start, end] {
                        let (x, y) = self.view.pdf_to_screen(point);
                        let handle_rect = Rect::from_center_size(Pos2::new(x, y), Vec2::splat(8.0));
                        painter.rect_filled(handle_rect, 0.0, colors.handle_fill);
                        stroke_rect(painter, handle_rect, Stroke::new(1.5_f32, colors.selection));
                    }
                } else {
                    for handle in Handle::ALL {
                        let at = handle.point(field.rect());
                        let (x, y) = self.view.pdf_to_screen(at);
                        let handle_rect = Rect::from_center_size(Pos2::new(x, y), Vec2::splat(8.0));
                        painter.rect_filled(handle_rect, 0.0, colors.handle_fill);
                        stroke_rect(painter, handle_rect, Stroke::new(1.5_f32, colors.selection));
                    }
                }
            }
        }

        if self.margins_visible {
            let safe = to_egui(self.view.pdf_rect_to_screen(print_safe_rect(&page)));
            dash_rect(painter, safe, Stroke::new(1.0_f32, colors.margin));
        }
        self.paint_page_chrome(painter, page, &colors);

        stroke_rect(painter, page_rect, Stroke::new(1.0_f32, colors.page_edge));
    }

    fn paint_field(
        &self,
        painter: &egui::Painter,
        field: &pageform_core::Field,
        colors: &crate::theme::CanvasColors,
    ) {
        let selected = self.selection == Some(field.id());
        let width = if selected { 2.0_f32 } else { 1.0 };
        let stroke_color = if selected {
            colors.selection
        } else if matches!(
            field.kind(),
            FieldKind::Line | FieldKind::Rectangle | FieldKind::StaticText
        ) {
            colors.shape_stroke
        } else if matches!(field.kind(), FieldKind::Checkbox | FieldKind::Radio) {
            colors.check_stroke
        } else {
            colors.text_stroke
        };
        let font = FontId::proportional((12.0 * self.view.zoom).clamp(8.0, 22.0));
        if let Some((start, end)) = field.line_ends() {
            let (x0, y0) = self.view.pdf_to_screen(start);
            let (x1, y1) = self.view.pdf_to_screen(end);
            painter.line_segment(
                [Pos2::new(x0, y0), Pos2::new(x1, y1)],
                Stroke::new(width.max(1.25), stroke_color),
            );
            return;
        }

        let screen = to_egui(self.view.pdf_rect_to_screen(field.rect()));
        let filled = !matches!(
            field.kind(),
            FieldKind::StaticText | FieldKind::Rectangle | FieldKind::Line | FieldKind::Radio
        );
        if filled {
            painter.rect_filled(screen, 0.0, colors.field_fill);
        }
        if field.kind() != FieldKind::Radio && (field.kind() != FieldKind::StaticText || selected) {
            stroke_rect(painter, screen, Stroke::new(width, stroke_color));
        }
        match field.kind() {
            FieldKind::Text | FieldKind::Signature | FieldKind::Date | FieldKind::Dropdown
                if screen.height() > 12.0 =>
            {
                let label = match field.kind() {
                    FieldKind::Text => field.name(),
                    FieldKind::Signature => field.caption(),
                    FieldKind::Date => DATE_HINT,
                    FieldKind::Dropdown => {
                        field.options().first().map(String::as_str).unwrap_or("")
                    }
                    _ => "",
                };
                painter.text(
                    screen.left_center() + Vec2::new(6.0, 0.0),
                    Align2::LEFT_CENTER,
                    label,
                    font.clone(),
                    colors.field_text,
                );
                if field.kind() == FieldKind::Dropdown {
                    painter.text(
                        screen.right_center() - Vec2::new(8.0, 0.0),
                        Align2::RIGHT_CENTER,
                        "v",
                        font,
                        colors.field_text,
                    );
                }
            }
            FieldKind::StaticText => {
                painter.text(
                    screen.left_center() + Vec2::new(2.0, 0.0),
                    Align2::LEFT_CENTER,
                    field.caption(),
                    font,
                    colors.label_text,
                );
            }
            FieldKind::Radio => {
                let center = screen.center();
                let radius = (screen.width().min(screen.height()) * 0.5 - 1.5).max(2.0);
                painter.circle_filled(center, radius, colors.field_fill);
                painter.circle_stroke(center, radius, Stroke::new(width, stroke_color));
                if self.doc.radio_labels() && !field.caption().is_empty() {
                    painter.text(
                        screen.right_center() + Vec2::new(6.0, 0.0),
                        Align2::LEFT_CENTER,
                        field.caption(),
                        font,
                        colors.label_text,
                    );
                }
            }
            FieldKind::Table => {
                let stroke = Stroke::new(1.0_f32, colors.text_stroke);
                for column in 1..TABLE_COLUMNS {
                    let x =
                        screen.left() + screen.width() * (column as f32) / (TABLE_COLUMNS as f32);
                    painter.line_segment(
                        [Pos2::new(x, screen.top()), Pos2::new(x, screen.bottom())],
                        stroke,
                    );
                }
                for row in 1..TABLE_ROWS {
                    let y = screen.top() + screen.height() * (row as f32) / (TABLE_ROWS as f32);
                    painter.line_segment(
                        [Pos2::new(screen.left(), y), Pos2::new(screen.right(), y)],
                        stroke,
                    );
                }
                painter.text(
                    screen.left_top() + Vec2::new(4.0, 4.0),
                    Align2::LEFT_TOP,
                    "placeholder",
                    font,
                    colors.field_text,
                );
            }
            FieldKind::TextBox => {
                if screen.width() > 8.0 {
                    let galley = painter.layout(
                        field.caption().to_string(),
                        font,
                        colors.field_text,
                        (screen.width() - 8.0).max(8.0),
                    );
                    painter.galley(
                        screen.left_top() + Vec2::new(4.0, 4.0),
                        galley,
                        colors.field_text,
                    );
                }
            }
            FieldKind::Image => {
                painter.line_segment(
                    [screen.left_top(), screen.right_bottom()],
                    Stroke::new(1.0_f32, colors.text_stroke),
                );
                painter.line_segment(
                    [screen.right_top(), screen.left_bottom()],
                    Stroke::new(1.0_f32, colors.text_stroke),
                );
                painter.text(
                    screen.center(),
                    Align2::CENTER_CENTER,
                    "Image",
                    font,
                    colors.field_text,
                );
            }
            FieldKind::Checkbox | FieldKind::Rectangle | FieldKind::Line => {}
            _ => {}
        }
    }

    fn paint_page_chrome(
        &self,
        painter: &egui::Painter,
        page: Page,
        colors: &crate::theme::CanvasColors,
    ) {
        let band = PRINT_SAFE_MARGIN_PT;
        let stroke = Stroke::new(1.0_f32, colors.margin);
        let font = FontId::proportional(13.0);
        if self.header_visible {
            let y = page.height - band;
            let (x0, sy) = self.view.pdf_to_screen(PdfPoint { x: band, y });
            let (x1, _) = self.view.pdf_to_screen(PdfPoint {
                x: page.width - band,
                y,
            });
            dash_line(painter, Pos2::new(x0, sy), Pos2::new(x1, sy), stroke);
            let (_, label_y) = self.view.pdf_to_screen(PdfPoint {
                x: page.width * 0.5,
                y: page.height - band * 0.45,
            });
            let (label_x, _) = self.view.pdf_to_screen(PdfPoint {
                x: page.width * 0.5,
                y: page.height,
            });
            painter.text(
                Pos2::new(label_x, label_y),
                Align2::CENTER_CENTER,
                "Header",
                font.clone(),
                colors.margin,
            );
        }
        if self.footer_visible {
            let y = band;
            let (x0, sy) = self.view.pdf_to_screen(PdfPoint { x: band, y });
            let (x1, _) = self.view.pdf_to_screen(PdfPoint {
                x: page.width - band,
                y,
            });
            dash_line(painter, Pos2::new(x0, sy), Pos2::new(x1, sy), stroke);
            let (label_x, label_y) = self.view.pdf_to_screen(PdfPoint {
                x: band + 8.0,
                y: band * 0.45,
            });
            painter.text(
                Pos2::new(label_x, label_y),
                Align2::LEFT_CENTER,
                "Footer",
                font.clone(),
                colors.margin,
            );
        }
        if self.page_numbers_visible {
            let (x, y) = self.view.pdf_to_screen(PdfPoint {
                x: page.width * 0.5,
                y: band * 0.45,
            });
            painter.text(
                Pos2::new(x, y),
                Align2::CENTER_CENTER,
                "1",
                font,
                colors.margin,
            );
        }
    }

    fn pointer_pdf_from_painter(&self, painter: &egui::Painter) -> Option<PdfPoint> {
        let pos = painter.ctx().input(|input| input.pointer.latest_pos())?;
        Some(self.view.screen_to_pdf(pos.x, pos.y))
    }

    fn paint_dots(&self, painter: &egui::Painter, page: Page, colors: &crate::theme::CanvasColors) {
        let minor = self.grid_size.minor_pt();
        let major = self.grid_size.major_pt();
        let draw_minor = minor * self.view.zoom >= 6.0;
        let step = if draw_minor { minor } else { major };
        for x in grid_lines(page.width, step) {
            for y in grid_lines(page.height, step) {
                let major_dot = on_step(x, major) && on_step(y, major);
                let (sx, sy) = self.view.pdf_to_screen(PdfPoint { x, y });
                let radius = if major_dot { 2.2 } else { 1.35 };
                let color = if major_dot {
                    colors.major_dot
                } else {
                    colors.minor_dot
                };
                painter.circle_filled(Pos2::new(sx, sy), radius, color);
            }
        }
    }

    fn paint_guides(
        &self,
        painter: &egui::Painter,
        page: Page,
        page_rect: Rect,
        colors: &crate::theme::CanvasColors,
    ) {
        let minor = self.grid_size.minor_pt();
        let major = self.grid_size.major_pt();
        let draw_minor = minor * self.view.zoom >= 6.0;
        for x in grid_lines(page.width, minor) {
            let major_line = on_step(x, major);
            if !major_line && !draw_minor {
                continue;
            }
            let (sx, _) = self.view.pdf_to_screen(PdfPoint { x, y: 0.0 });
            let color = if major_line {
                colors.major_guide
            } else {
                colors.minor_guide
            };
            painter.line_segment(
                [
                    Pos2::new(sx, page_rect.top()),
                    Pos2::new(sx, page_rect.bottom()),
                ],
                Stroke::new(1.0_f32, color),
            );
        }
        for y in grid_lines(page.height, minor) {
            let major_line = on_step(y, major);
            if !major_line && !draw_minor {
                continue;
            }
            let (_, sy) = self.view.pdf_to_screen(PdfPoint { x: 0.0, y });
            let color = if major_line {
                colors.major_guide
            } else {
                colors.minor_guide
            };
            painter.line_segment(
                [
                    Pos2::new(page_rect.left(), sy),
                    Pos2::new(page_rect.right(), sy),
                ],
                Stroke::new(1.0_f32, color),
            );
        }
    }

    fn hover_cursor(&self, response: &egui::Response, ctx: &egui::Context) {
        if !response.hovered() {
            return;
        }
        let cursor = if ctx.input(|input| input.key_down(Key::Space)) {
            CursorIcon::Grab
        } else if self.tool.place_kind().is_some() {
            CursorIcon::Crosshair
        } else if let Some(pos) = response.hover_pos() {
            let pdf = self.view.screen_to_pdf(pos.x, pos.y);
            let radius = 8.0 / self.view.zoom;
            if let Some(id) = self.selection {
                if let Some(field) = self.doc.field(id) {
                    if field.hit_line_end(pdf, radius).is_some() {
                        CursorIcon::Grab
                    } else if field.line_ends().is_none() {
                        if let Some(handle) = hit_handle(field.rect(), pdf, radius) {
                            handle_cursor(handle)
                        } else if self.field_at(pdf) {
                            CursorIcon::Grab
                        } else {
                            return;
                        }
                    } else if self.field_at(pdf) {
                        CursorIcon::Grab
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            } else if self.field_at(pdf) {
                CursorIcon::Grab
            } else {
                return;
            }
        } else {
            return;
        };
        response.ctx.set_cursor_icon(cursor);
    }

    fn field_at(&self, pdf: PdfPoint) -> bool {
        let radius = 8.0 / self.view.zoom;
        self.doc
            .fields()
            .iter()
            .any(|field| field.contains_point(pdf, radius))
    }
}

impl eframe::App for PageFormApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(
            crate::theme::STORAGE_KEY,
            crate::theme::storage_value(self.dark).to_owned(),
        );
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        crate::theme::apply(ctx, self.dark);
        if self.persist_theme {
            if let Some(storage) = frame.storage_mut() {
                storage.set_string(
                    crate::theme::STORAGE_KEY,
                    crate::theme::storage_value(self.dark).to_owned(),
                );
                storage.flush();
            }
            self.persist_theme = false;
        }
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.menu_bar(ui);
        });
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(2.0);
            self.toolbar(ui);
            ui.add_space(2.0);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.separator();
                ui.label("Scroll to zoom · middle-drag or Space-drag to pan");
            });
        });
        egui::SidePanel::right("properties")
            .resizable(false)
            .exact_width(260.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                self.properties(ui);
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            self.canvas(ui);
        });
    }
}

fn to_egui(rect: ScreenRect) -> Rect {
    Rect::from_min_size(Pos2::new(rect.x, rect.y), Vec2::new(rect.w, rect.h))
}

fn dash_rect(painter: &egui::Painter, rect: Rect, stroke: Stroke) {
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    for index in 0..4 {
        dash_line(painter, corners[index], corners[(index + 1) % 4], stroke);
    }
}

fn dash_line(painter: &egui::Painter, start: Pos2, end: Pos2, stroke: Stroke) {
    let delta = end - start;
    let len = delta.length();
    if len < 1.0 {
        return;
    }
    let dir = delta / len;
    let dash = 7.0;
    let gap = 5.0;
    let mut traveled = 0.0;
    while traveled < len {
        let stop = (traveled + dash).min(len);
        painter.line_segment([start + dir * traveled, start + dir * stop], stroke);
        traveled += dash + gap;
    }
}

fn stroke_rect(painter: &egui::Painter, rect: Rect, stroke: Stroke) {
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    for pair in corners.windows(2) {
        painter.line_segment([pair[0], pair[1]], stroke);
    }
    painter.line_segment([corners[3], corners[0]], stroke);
}

fn handle_cursor(handle: Handle) -> CursorIcon {
    match handle {
        Handle::N | Handle::S => CursorIcon::ResizeVertical,
        Handle::E | Handle::W => CursorIcon::ResizeHorizontal,
        Handle::Ne | Handle::Sw => CursorIcon::ResizeNeSw,
        Handle::Nw | Handle::Se => CursorIcon::ResizeNwSe,
    }
}

fn grid_lines(span: f32, step: f32) -> impl Iterator<Item = f32> {
    let count = (span / step).floor() as i32;
    (0..=count)
        .map(move |index| index as f32 * step)
        .filter(move |value| *value <= span + 0.01)
}

fn on_step(value: f32, step: f32) -> bool {
    ((value / step).round() * step - value).abs() < 0.05
}

/// File → Quit on Linux. File → Exit on Windows.
fn quit_label() -> &'static str {
    if cfg!(target_os = "windows") {
        "Exit"
    } else {
        "Quit"
    }
}

fn anchor_for_center(center: PdfPoint, w: f32, h: f32) -> PdfPoint {
    PdfPoint {
        x: center.x - w * 0.5,
        y: center.y + h * 0.5,
    }
}

fn same_rect(a: RectPt, b: RectPt) -> bool {
    (a.x - b.x).abs() < 0.5
        && (a.y - b.y).abs() < 0.5
        && (a.w - b.w).abs() < 0.5
        && (a.h - b.h).abs() < 0.5
}

fn rects_overlap(a: RectPt, b: RectPt) -> bool {
    a.x < b.right() - 0.5 && b.x < a.right() - 0.5 && a.y < b.top() - 0.5 && b.y < a.top() - 0.5
}

fn menu_command(ui: &mut egui::Ui, label: &str, shortcut: Option<&str>, selected: bool) -> bool {
    let mut button = egui::Button::new(label).selected(selected);
    if let Some(shortcut) = shortcut {
        button = button.shortcut_text(shortcut);
    }
    let clicked = ui.add(button).clicked();
    if clicked {
        ui.close();
    }
    clicked
}

fn insert_tip(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Text => "Drop a one-line fillable text field at the center of the visible page.",
        FieldKind::StaticText => "Drop a static label. It is not a fillable field.",
        FieldKind::Checkbox => "Drop a default-sized checkbox at the center of the visible page.",
        FieldKind::Radio => {
            "Drop a radio button. If a radio is selected, it joins that group. Labels sit beside each button."
        }
        FieldKind::Table => "Drop a table placeholder (a small grid). Cells are not editable yet.",
        FieldKind::Signature => {
            "Drop an AcroForm signature field. Capturing a signature in the app is not available yet."
        }
        FieldKind::Date => "Drop a date field (YYYY-MM-DD text).",
        FieldKind::Dropdown => "Drop a combo drop-down. This is not a list box.",
        FieldKind::Line => "Drop a horizontal line. Drag with the toolbar tool to set the angle.",
        FieldKind::Rectangle => "Drop a rectangle. Drag with the toolbar tool to size it.",
        FieldKind::TextBox => "Drop a static text box. It is not a fillable field.",
        FieldKind::Image => "Drop an image placeholder. Choosing a file is not available yet.",
    }
}

fn toolbar_tip(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Text => "Arm the text field tool, then drag on the page.",
        FieldKind::StaticText => {
            "Arm the static text tool, then drag on the page. Not a fillable field."
        }
        FieldKind::Checkbox => "Arm the checkbox tool, then drag on the page.",
        FieldKind::Radio => {
            "Arm the radio tool, then drag on the page. A selected radio joins that group."
        }
        FieldKind::Table => "Arm the table tool. Placeholder grid; cells are not editable yet.",
        FieldKind::Signature => "Arm the signature tool. No ink capture in the app yet.",
        FieldKind::Date => "Arm the date tool, then drag on the page.",
        FieldKind::Dropdown => "Arm the drop-down tool. Combo box, not a list box.",
        FieldKind::Line => "Arm the line tool, then drag from one point to another.",
        FieldKind::Rectangle => "Arm the rectangle tool, then drag on the page.",
        FieldKind::TextBox => "Arm the text box tool. Static text, not a fillable field.",
        FieldKind::Image => "Arm the image tool. Placeholder box until a file can be chosen.",
    }
}

fn kind_note(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Text => "One-line fillable text field.",
        FieldKind::StaticText => "Static label. Not a fillable field.",
        FieldKind::Checkbox => "Fillable checkbox.",
        FieldKind::Radio => "One button in a radio group. The first two labels are Yes and No.",
        FieldKind::Table => "Table placeholder. A 3×2 grid. Cells are not editable yet.",
        FieldKind::Signature => {
            "Unsigned signature field. Drawing a signature is not available yet."
        }
        FieldKind::Date => "Fillable text, 10 characters, format YYYY-MM-DD. Not a calendar.",
        FieldKind::Dropdown => "Combo drop-down. Not a list box. Options are comma-separated.",
        FieldKind::Line => "Line on the page. Not a fillable field.",
        FieldKind::Rectangle => "Rectangle on the page. Not a fillable field.",
        FieldKind::TextBox => "Static text box. Not a fillable field.",
        FieldKind::Image => "Image placeholder. Choosing a file is not available yet.",
    }
}

fn fmt_pt(value: f32) -> String {
    if (value - value.round()).abs() < 0.05 {
        format!("{}", value.round() as i32)
    } else {
        format!("{value:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quit_label_matches_the_desktop() {
        let label = quit_label();
        if cfg!(target_os = "windows") {
            assert_eq!(label, "Exit");
        } else {
            assert_eq!(label, "Quit");
        }
    }

    #[test]
    fn delete_removes_the_selected_field() {
        let mut app = PageFormApp::new(true, None);
        let id = app.doc.fields()[0].id();
        app.selection = Some(id);
        app.delete_selection();
        assert!(app.doc.field(id).is_none());
        assert!(app.selection.is_none());
        assert!(app.drag.is_none());
        assert_eq!(app.status, "Deleted field");
        assert_eq!(app.doc.fields().len(), 1);
    }

    #[test]
    fn delete_with_nothing_selected_leaves_the_page() {
        let mut app = PageFormApp::new(false, None);
        app.delete_selection();
        assert!(app.doc.fields().is_empty());
        assert_eq!(app.status, "Blank US Letter page");
    }

    #[test]
    fn insert_menu_drops_a_default_field_at_the_page_center() {
        let mut app = PageFormApp::new(false, None);
        app.tool = Tool::Checkbox;
        app.insert_at_view(FieldKind::Text);
        assert_eq!(app.tool, Tool::Checkbox);
        assert_eq!(app.doc.fields().len(), 1);
        let field = &app.doc.fields()[0];
        assert_eq!(field.kind(), FieldKind::Text);
        assert_eq!(app.selection, Some(field.id()));
        let rect = field.rect();
        let (w, h) = default_size(FieldKind::Text, GridSize::Medium);
        assert!((rect.w - w).abs() < 0.1);
        assert!((rect.h - h).abs() < 0.1);
        assert!((rect.x + rect.w * 0.5 - 306.0).abs() < 0.1);
        assert!((rect.y + rect.h * 0.5 - 396.0).abs() < 0.1);
        assert!(on_step(rect.x, GridSize::Medium.minor_pt()));
        assert!(on_step(rect.y, GridSize::Medium.minor_pt()));
    }

    #[test]
    fn insert_menu_steps_past_a_field_already_in_the_view() {
        let mut app = PageFormApp::new(false, None);
        app.insert_at_view(FieldKind::Text);
        app.insert_at_view(FieldKind::Checkbox);
        app.insert_at_view(FieldKind::Text);
        let fields = app.doc.fields();
        assert_eq!(fields.len(), 3);
        assert!(!rects_overlap(fields[0].rect(), fields[1].rect()));
        assert!(!rects_overlap(fields[0].rect(), fields[2].rect()));
        assert!(!rects_overlap(fields[1].rect(), fields[2].rect()));
        assert!(fields[1].rect().top() <= fields[0].rect().y + 0.5);
    }

    #[test]
    fn insert_menu_uses_the_visible_page_not_the_page_center() {
        let mut app = PageFormApp::new(false, None);
        app.view.zoom = 2.0;
        app.view.origin_x = 0.0;
        app.view.origin_y = 0.0;
        app.view.page_height = app.doc.page().height;
        app.canvas_rect = Some(Rect::from_min_max(
            Pos2::new(0.0, 0.0),
            Pos2::new(200.0, 200.0),
        ));
        let center = app.visible_page_center();
        assert!((center.x - 50.0).abs() < 0.1);
        assert!((center.y - 742.0).abs() < 0.1);
        app.insert_at_view(FieldKind::Checkbox);
        let rect = app.doc.fields()[0].rect();
        let (w, h) = default_size(FieldKind::Checkbox, GridSize::Medium);
        assert!((rect.w - w).abs() < 0.1);
        assert!((rect.h - h).abs() < 0.1);
        assert!((rect.x + rect.w * 0.5 - center.x).abs() < app.grid_size.minor_pt());
        assert!((rect.y + rect.h * 0.5 - center.y).abs() < app.grid_size.minor_pt());
        assert!(rect.x >= 0.0 && rect.y >= 0.0);
        assert!(rect.right() <= app.doc.page().width + 0.1);
        assert!(rect.top() <= app.doc.page().height + 0.1);
    }

    #[test]
    fn insert_radio_joins_the_selected_group_and_labels_can_hide() {
        let mut app = PageFormApp::new(false, None);
        assert!(app.doc.radio_labels());
        app.insert_at_view(FieldKind::Radio);
        app.insert_at_view(FieldKind::Radio);
        assert_eq!(app.doc.fields().len(), 2);
        assert_eq!(app.doc.fields()[0].name(), app.doc.fields()[1].name());
        assert_eq!(app.doc.fields()[0].caption(), "Yes");
        assert_eq!(app.doc.fields()[1].caption(), "No");
        assert!(app.doc.fields()[0].is_form_field());
        app.doc.set_radio_labels(false);
        assert!(!app.doc.radio_labels());
        assert_eq!(app.tool, Tool::Select);
    }

    #[test]
    fn static_text_and_image_are_not_form_fields() {
        let mut app = PageFormApp::new(false, None);
        app.tool = Tool::Text;
        app.insert_at_view(FieldKind::StaticText);
        app.insert_at_view(FieldKind::Image);
        app.insert_at_view(FieldKind::Line);
        app.insert_at_view(FieldKind::Rectangle);
        app.insert_at_view(FieldKind::TextBox);
        assert_eq!(app.tool, Tool::Text);
        assert!(app.doc.fields().iter().all(|field| !field.is_form_field()));
        assert_eq!(app.doc.fields()[0].kind(), FieldKind::StaticText);
        assert_eq!(app.doc.fields()[0].caption(), "Label");
        assert_eq!(app.doc.fields()[1].kind(), FieldKind::Image);
    }

    #[test]
    fn header_footer_and_page_numbers_start_off() {
        let app = PageFormApp::new(false, None);
        assert!(!app.header_visible);
        assert!(!app.footer_visible);
        assert!(!app.page_numbers_visible);
        assert!(app.dark);
        assert_eq!(
            FieldKind::INSERT.map(|kind| kind.label()),
            [
                "Text Field",
                "Text",
                "Checkbox",
                "Radio",
                "Table",
                "Signature",
                "Date",
                "Drop-down",
                "Line",
                "Square/Rectangle",
                "Text Box",
                "Image",
            ]
        );
    }

    #[test]
    fn theme_switch_is_marked_for_save_and_dark_stays_default() {
        let mut app = PageFormApp::new(false, None);
        assert!(app.dark);
        assert!(!app.persist_theme);
        app.set_dark(false);
        assert!(!app.dark);
        assert!(app.persist_theme);
        app.persist_theme = false;
        app.set_dark(false);
        assert!(!app.persist_theme);
        app.set_dark(true);
        assert!(app.dark);
        assert!(app.persist_theme);
    }
}
