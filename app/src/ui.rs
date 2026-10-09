//! egui canvas. Geometry and PDF export live in `pageform-core`.

use eframe::egui::{
    self, Align2, CursorIcon, FontId, Key, PointerButton, Pos2, Rect, Sense, Stroke, Vec2,
};
use pageform_core::{
    default_size, export_pdf, hit_handle, move_rect, place_default, print_safe_rect,
    rect_from_drag, resize_rect, Document, FieldKind, GridSize, Handle, Page, PdfPoint, RectPt,
    ScreenRect, ViewTransform,
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
    Checkbox,
}

#[derive(Clone, Copy)]
enum Drag {
    Pan,
    Place {
        start: PdfPoint,
        kind: FieldKind,
    },
    Move {
        id: pageform_core::FieldId,
        grab_x: f32,
        grab_y: f32,
    },
    Resize {
        id: pageform_core::FieldId,
        handle: Handle,
        origin: RectPt,
    },
}

struct PageFormApp {
    doc: Document,
    tool: Tool,
    selection: Option<pageform_core::FieldId>,
    dots_visible: bool,
    guides_visible: bool,
    margins_visible: bool,
    grid_size: GridSize,
    snap: bool,
    view: ViewTransform,
    fitted: bool,
    canvas_rect: Option<Rect>,
    drag: Option<Drag>,
    name_buf: String,
    name_for: Option<pageform_core::FieldId>,
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
            grid_size: GridSize::Medium,
            snap: true,
            fitted: false,
            canvas_rect: None,
            drag: None,
            name_buf: String::new(),
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
    }

    fn insert_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(220.0);
        let text = ui
            .button("Text")
            .on_hover_text("Place a default-sized text field at the center of the visible page.");
        if text.clicked() {
            self.insert_at_view(FieldKind::Text);
            ui.close();
        }
        let checkbox = ui
            .button("Checkbox")
            .on_hover_text("Place a default-sized checkbox at the center of the visible page.");
        if checkbox.clicked() {
            self.insert_at_view(FieldKind::Checkbox);
            ui.close();
        }
    }

    /// Insert menu drops a field. The toolbar tools stay armed for drawing.
    fn insert_at_view(&mut self, kind: FieldKind) {
        let rect = self.default_insert_rect(kind);
        let id = self.doc.add_field(kind, rect);
        self.selection = Some(id);
        self.drag = None;
        self.status = format!("Placed {}", kind.label().to_lowercase());
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
            ui.selectable_value(&mut self.tool, Tool::Text, "Text field");
            ui.selectable_value(&mut self.tool, Tool::Checkbox, "Checkbox");
            ui.separator();
            ui.toggle_value(&mut self.dots_visible, "Dots")
                .on_hover_text("Grid dots");
            ui.toggle_value(&mut self.guides_visible, "Guides")
                .on_hover_text("Guide lines. Canvas only.");
            ui.toggle_value(&mut self.margins_visible, "Margins")
                .on_hover_text("Print-safe margin. Canvas only.");
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
        ui.label("Dots cover the whole page. Guide lines and the print-safe outline are canvas only and are not exported.");
        ui.label(format!(
            "Print safe area is inset {:.0} pt (0.5 in).",
            pageform_core::PRINT_SAFE_MARGIN_PT
        ));
        ui.label("Snap uses the minor spacing, even if dots or guides are hidden, so columns and rows stay even.");
        let (text_w, text_h) = default_size(FieldKind::Text, self.grid_size);
        let (check, _) = default_size(FieldKind::Checkbox, self.grid_size);
        ui.label(format!(
            "Click places text {}×{} pt or a {} pt checkbox.",
            fmt_pt(text_w),
            fmt_pt(text_h),
            fmt_pt(check)
        ));
        ui.separator();
        ui.heading("Field");

        if self.name_for != self.selection {
            self.name_for = self.selection;
            self.name_buf = self
                .selection
                .and_then(|id| self.doc.field(id).map(|field| field.name().to_string()))
                .unwrap_or_default();
            self.name_error = None;
        }

        let Some(id) = self.selection else {
            ui.label("No field selected.");
            ui.label("Toolbar Text field or Checkbox draws on the page. Insert drops a default field in the view.");
            return;
        };
        let Some(field) = self.doc.field(id) else {
            self.selection = None;
            return;
        };
        let kind = field.kind();
        let rect = field.rect();
        ui.label(format!("Type  {}", kind.label()));
        ui.label("Name");
        let edit = ui.add(egui::TextEdit::singleline(&mut self.name_buf).desired_width(200.0));
        if edit.changed() {
            self.name_error = self
                .doc
                .set_name(id, &self.name_buf)
                .err()
                .map(|err| err.to_string());
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
        ui.label("Delete removes the selected field.");
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
                    let rect = self.place_rect(start, current, kind);
                    let id = self.doc.add_field(kind, rect);
                    self.selection = Some(id);
                    self.tool = Tool::Select;
                    self.status = format!("Placed {}", kind.label().to_lowercase());
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
            Tool::Text | Tool::Checkbox => {
                if !self.on_page(pdf) {
                    return;
                }
                let kind = if self.tool == Tool::Text {
                    FieldKind::Text
                } else {
                    FieldKind::Checkbox
                };
                self.drag = Some(Drag::Place { start: pdf, kind });
            }
            Tool::Select => {
                let radius = 8.0 / self.view.zoom;
                if let Some(id) = self.selection {
                    if let Some(field) = self.doc.field(id) {
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
                if let Some(field) = self
                    .doc
                    .fields()
                    .iter()
                    .rev()
                    .find(|field| field.rect().contains(pdf))
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
        }
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
            let screen = to_egui(self.view.pdf_rect_to_screen(field.rect()));
            let selected = self.selection == Some(field.id());
            painter.rect_filled(screen, 0.0, colors.field_fill);
            let color = if selected {
                colors.selection
            } else if field.kind() == FieldKind::Text {
                colors.text_stroke
            } else {
                colors.check_stroke
            };
            let width = if selected { 2.0_f32 } else { 1.0 };
            stroke_rect(painter, screen, Stroke::new(width, color));
            if field.kind() == FieldKind::Text && screen.height() > 14.0 {
                painter.text(
                    screen.left_center() + Vec2::new(6.0, 0.0),
                    Align2::LEFT_CENTER,
                    field.name(),
                    FontId::proportional((12.0 * self.view.zoom).clamp(8.0, 22.0)),
                    colors.field_text,
                );
            }
        }

        if let Some(Drag::Place { start, kind }) = self.drag {
            if let Some(current) = self.pointer_pdf_from_painter(painter) {
                let rect = self.place_rect(start, current, kind);
                let screen = to_egui(self.view.pdf_rect_to_screen(rect));
                stroke_rect(painter, screen, Stroke::new(1.5_f32, colors.selection));
            }
        }

        if let Some(id) = self.selection {
            if let Some(field) = self.doc.field(id) {
                for handle in Handle::ALL {
                    let at = handle.point(field.rect());
                    let (x, y) = self.view.pdf_to_screen(at);
                    let handle_rect = Rect::from_center_size(Pos2::new(x, y), Vec2::splat(8.0));
                    painter.rect_filled(handle_rect, 0.0, colors.handle_fill);
                    stroke_rect(painter, handle_rect, Stroke::new(1.5_f32, colors.selection));
                }
            }
        }

        if self.margins_visible {
            let safe = to_egui(self.view.pdf_rect_to_screen(print_safe_rect(&page)));
            dash_rect(painter, safe, Stroke::new(1.0_f32, colors.margin));
        }

        stroke_rect(painter, page_rect, Stroke::new(1.0_f32, colors.page_edge));
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
        } else if matches!(self.tool, Tool::Text | Tool::Checkbox) {
            CursorIcon::Crosshair
        } else if let Some(pos) = response.hover_pos() {
            let pdf = self.view.screen_to_pdf(pos.x, pos.y);
            if let Some(id) = self.selection {
                if let Some(field) = self.doc.field(id) {
                    if let Some(handle) = hit_handle(field.rect(), pdf, 8.0 / self.view.zoom) {
                        handle_cursor(handle)
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
        self.doc
            .fields()
            .iter()
            .any(|field| field.rect().contains(pdf))
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
