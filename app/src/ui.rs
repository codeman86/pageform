//! egui canvas. Geometry and PDF export live in `pageform-core`.

#[path = "icons.rs"]
mod icons;
#[path = "properties.rs"]
mod properties;

use eframe::egui::{
    self, Align2, CursorIcon, FontId, Key, PointerButton, Pos2, Rect, Sense, Stroke, Vec2,
};
use pageform_core::{
    default_size, export_pdf, hit_handle, move_rect, place_default, rect_from_drag, resize_rect,
    snap_point, AlignEdge, BorderStyleKind, CheckboxStyle, Document, Field, FieldId, FieldKind,
    FontFamily, GridSize, Handle, Page, PdfPoint, ProjectKind, RectPt, Rgb, ScreenRect, TextAlign,
    ViewTransform, TABLE_COLUMNS, TABLE_ROWS,
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
    Square,
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
            Tool::Square | Tool::Rectangle => Some(FieldKind::Rectangle),
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

/// Which string a canvas text editor writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InlineTarget {
    /// Static text, text box, radio or checkbox label, signature word.
    Caption,
    /// Fillable field's default value.
    DefaultValue,
}

struct InlineEdit {
    id: FieldId,
    target: InlineTarget,
    buffer: String,
    original: String,
    multiline: bool,
    request_focus: bool,
}

#[derive(Clone)]
enum Drag {
    Pan,
    Place {
        start: PdfPoint,
        kind: FieldKind,
    },
    Move {
        grab_id: FieldId,
        grab_x: f32,
        grab_y: f32,
        origins: Vec<(FieldId, RectPt)>,
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
    Marquee {
        start: PdfPoint,
        extend: bool,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

enum NativePick {
    Path(std::path::PathBuf),
    Cancel,
    Fallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PromptKind {
    ExportPdf,
    SaveProject,
    OpenProject,
    OpenTemplate,
}

#[derive(Clone)]
enum PendingFileAction {
    NewBlank,
    Open,
    OpenPath(String),
    NewFromTemplate,
    Exit,
}

#[derive(Clone)]
struct FilePrompt {
    kind: PromptKind,
    directory: String,
    file_name: String,
    /// The chosen path already exists. The next Save replaces it.
    replace: bool,
    /// Save As writes a `.pgt` template instead of a `.pgd` document.
    save_as_template: bool,
}

struct PageFormApp {
    doc: Document,
    tool: Tool,
    /// Last entry is the primary item (handles, properties, match-size reference).
    selection: Vec<FieldId>,
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
    clipboard: Vec<Field>,
    paste_steps: i32,
    /// The Insert menu click is also over the canvas. Ignore that pointer
    /// press so it does not clear the selection just made.
    suppress_place_click: bool,
    name_buf: String,
    caption_buf: String,
    tooltip_buf: String,
    value_buf: String,
    options_buf: String,
    name_for: Option<FieldId>,
    name_error: Option<String>,
    export_path: String,
    project_path: Option<String>,
    recent: Vec<String>,
    unsaved: Option<PendingFileAction>,
    after_save: Option<PendingFileAction>,
    allow_close: bool,
    last_title: String,
    /// A project save just succeeded. Continue New, Open, or Exit afterwards.
    saved_follow_up: bool,
    status: String,
    /// Status text last observed, so a change restarts the fade timer.
    status_seen: String,
    status_at: f64,
    dark: bool,
    persist_theme: bool,
    properties_open: bool,
    properties_pinned: bool,
    persist_panel: bool,
    context_field: Option<FieldId>,
    inline: Option<InlineEdit>,
    status_before_inline: Option<String>,
    /// False until the user pans or zooms, so the page stays centered as the canvas resizes.
    navigated: bool,
    canvas_size: Option<Vec2>,
    show_preferences: bool,
    show_about: bool,
    show_help: bool,
    file_prompt: Option<FilePrompt>,
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
        let open = storage.and_then(|storage| storage.get_string(crate::prefs::OPEN_KEY));
        let pinned = storage.and_then(|storage| storage.get_string(crate::prefs::PIN_KEY));
        let recent = crate::prefs::parse_recent(
            storage
                .and_then(|storage| storage.get_string(crate::prefs::RECENT_KEY))
                .as_deref(),
        );
        Self {
            view: ViewTransform::new(page.height),
            doc,
            tool: Tool::Select,
            selection: Vec::new(),
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
            clipboard: Vec::new(),
            paste_steps: 0,
            suppress_place_click: false,
            name_buf: String::new(),
            caption_buf: String::new(),
            tooltip_buf: String::new(),
            value_buf: String::new(),
            options_buf: String::new(),
            name_for: None,
            name_error: None,
            export_path: "sample-form.pdf".to_string(),
            project_path: None,
            recent,
            unsaved: None,
            after_save: None,
            allow_close: false,
            last_title: String::new(),
            saved_follow_up: false,
            status: if demo {
                "Sample page with full_name and agree".to_string()
            } else {
                "Blank US Letter page".to_string()
            },
            status_seen: if demo {
                "Sample page with full_name and agree".to_string()
            } else {
                "Blank US Letter page".to_string()
            },
            status_at: 0.0,
            dark: crate::theme::is_dark(stored.as_deref()),
            persist_theme: false,
            properties_open: crate::prefs::is_open(open.as_deref()),
            properties_pinned: crate::prefs::is_pinned(pinned.as_deref()),
            persist_panel: false,
            context_field: None,
            inline: None,
            status_before_inline: None,
            navigated: false,
            canvas_size: None,
            show_preferences: false,
            show_about: false,
            show_help: false,
            file_prompt: None,
        }
    }

    /// Clear a status message a few seconds after it was set. The edit prompt
    /// stays until the edit finishes. Returns the time left so the window can
    /// repaint without waiting for the next input event.
    fn fade_status(&mut self, time: f64) -> Option<f64> {
        if self.status != self.status_seen {
            self.status_seen.clone_from(&self.status);
            self.status_at = time;
        }
        if self.inline.is_some() || self.status.is_empty() {
            return None;
        }
        let remaining = 4.0 - (time - self.status_at);
        if remaining <= 0.0 {
            self.status.clear();
            self.status_seen.clear();
            None
        } else {
            Some(remaining)
        }
    }

    fn select_all(&mut self) {
        self.selection = self.doc.fields().iter().map(|field| field.id()).collect();
        let count = self.selection.len();
        self.status = if count == 1 {
            "Selected 1 item".to_string()
        } else {
            format!("Selected {count} items")
        };
    }

    fn copy_selection(&mut self) {
        self.clipboard = self
            .selection
            .iter()
            .filter_map(|id| self.doc.field(*id).cloned())
            .collect();
        self.paste_steps = 0;
        let count = self.clipboard.len();
        self.status = if count == 1 {
            "Copied 1 item".to_string()
        } else {
            format!("Copied {count} items")
        };
    }

    fn cut_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.copy_selection();
        let count = self.clipboard.len();
        self.delete_selection();
        self.status = if count == 1 {
            "Cut 1 item".to_string()
        } else {
            format!("Cut {count} items")
        };
    }

    fn paste_clipboard(&mut self) {
        if self.clipboard.is_empty() {
            self.status = "Nothing to paste".to_string();
            return;
        }
        self.paste_steps += 1;
        let step = self.grid_size.minor_pt() * self.paste_steps as f32;
        let mut ids = Vec::new();
        for field in self.clipboard.clone() {
            ids.push(self.doc.paste_copy(&field, step, -step));
        }
        self.selection = ids;
        let count = self.selection.len();
        self.status = if count == 1 {
            "Pasted 1 item".to_string()
        } else {
            format!("Pasted {count} items")
        };
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
        let page = self.doc.page();
        let square = self.tool == Tool::Square && kind == FieldKind::Rectangle;
        if is_click(&self.view, start, current) {
            let (mut w, mut h) = default_size(kind, self.grid_size);
            if square {
                let side = w.min(h);
                w = side;
                h = side;
            }
            place_default(start, w, h, self.snap_grid(), &page)
        } else if square {
            square_from_drag(start, current, self.snap_grid(), self.min_size(), &page)
        } else {
            rect_from_drag(start, current, self.snap_grid(), self.min_size(), &page)
        }
    }

    fn window_title(&self) -> String {
        let name = self
            .project_path
            .as_ref()
            .and_then(|path| std::path::Path::new(path).file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled");
        if self.doc.is_dirty() {
            format!("{name} * - PageForm")
        } else {
            format!("{name} - PageForm")
        }
    }

    fn request_new_blank(&mut self) {
        self.ask_or_run(PendingFileAction::NewBlank);
    }

    fn new_blank(&mut self) {
        self.doc = Document::letter();
        self.project_path = None;
        self.selection.clear();
        self.clipboard.clear();
        self.paste_steps = 0;
        self.inline = None;
        self.drag = None;
        self.fitted = false;
        self.navigated = false;
        self.view.page_height = self.doc.page().height;
        self.status = "Blank US Letter page".to_string();
    }

    fn ask_or_run(&mut self, action: PendingFileAction) {
        if self.doc.is_dirty() {
            self.unsaved = Some(action);
        } else {
            self.run_file_action(action);
        }
    }

    fn run_file_action(&mut self, action: PendingFileAction) {
        match action {
            PendingFileAction::NewBlank => self.new_blank(),
            PendingFileAction::Open => self.begin_open(),
            PendingFileAction::OpenPath(path) => {
                self.read_project_file(std::path::Path::new(&path))
            }
            PendingFileAction::NewFromTemplate => self.begin_new_from_template(),
            PendingFileAction::Exit => self.allow_close = true,
        }
    }

    fn export_current(&mut self) {
        match pick_native(
            true,
            "Export PDF",
            &[("PDF", &["pdf"])],
            &export_file_name(&self.export_path),
        ) {
            NativePick::Path(mut path) => {
                if path.extension().is_none() {
                    path.set_extension("pdf");
                }
                self.export_path = path.display().to_string();
                self.write_export();
            }
            NativePick::Cancel => self.status = "Export cancelled".to_string(),
            NativePick::Fallback => self.open_builtin_prompt(PromptKind::ExportPdf),
        }
    }

    fn begin_open(&mut self) {
        match pick_native(false, "Open PageForm", OPEN_FILTERS, "") {
            NativePick::Path(path) => self.read_project_file(&path),
            NativePick::Cancel => self.status = "Open cancelled".to_string(),
            NativePick::Fallback => self.open_builtin_prompt(PromptKind::OpenProject),
        }
    }

    fn begin_new_from_template(&mut self) {
        match pick_native(false, "New from Template", TEMPLATE_FILTERS, "") {
            NativePick::Path(path) => self.read_project_file_as_untitled(&path),
            NativePick::Cancel => self.status = "New from Template cancelled".to_string(),
            NativePick::Fallback => self.open_builtin_prompt(PromptKind::OpenTemplate),
        }
    }

    fn save_project(&mut self) {
        // No linked document: Save As, defaulting to a new `.pgd`. A `.pgt` is
        // never the target of plain Save.
        if let Some(path) = self.linked_document_path() {
            self.write_project_file(std::path::Path::new(&path));
        } else {
            self.save_project_as();
        }
    }

    /// The `.pgd` or legacy `.pageform` this document would overwrite.
    /// `None` for a new document and for anything opened from a template.
    fn linked_document_path(&self) -> Option<String> {
        self.project_path
            .clone()
            .filter(|path| !is_template_path(std::path::Path::new(path)))
    }

    fn save_project_as(&mut self) {
        let name = suggested_project_name(self.project_path.as_deref(), false);
        match pick_native(true, "Save PageForm", SAVE_FILTERS, &name) {
            NativePick::Path(path) => self.save_chosen_project(native_project_save_path(path)),
            NativePick::Cancel => {
                self.after_save = None;
                self.status = "Save cancelled".to_string();
            }
            NativePick::Fallback => self.open_builtin_prompt(PromptKind::SaveProject),
        }
    }

    /// A `.pgt` is stored only when Save As already resolved to that extension,
    /// which is the Template type. An existing template asks before replacement.
    fn save_chosen_project(&mut self, path: std::path::PathBuf) {
        if is_template_path(&path) && path.exists() {
            self.confirm_template_replace(path);
            return;
        }
        self.write_project_file(&path);
    }

    fn confirm_template_replace(&mut self, path: std::path::PathBuf) {
        let directory = path
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(|dir| dir.display().to_string())
            .unwrap_or_else(default_export_dir);
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled.pgt")
            .to_string();
        self.file_prompt = Some(FilePrompt {
            kind: PromptKind::SaveProject,
            directory,
            file_name,
            replace: true,
            save_as_template: true,
        });
        self.status = "That template already exists. Save again to replace it.".to_string();
    }

    fn open_builtin_prompt(&mut self, kind: PromptKind) {
        let (directory, file_name, status) = match kind {
            PromptKind::ExportPdf => (
                parent_dir(&self.export_path),
                export_file_name(&self.export_path),
                "Choose where to save the PDF.",
            ),
            PromptKind::SaveProject => {
                let name = suggested_project_name(self.project_path.as_deref(), false);
                let directory = self
                    .project_path
                    .as_deref()
                    .map(parent_dir)
                    .unwrap_or_else(default_export_dir);
                (
                    directory,
                    name,
                    "Choose Document (.pgd) or Template (.pgt).",
                )
            }
            PromptKind::OpenProject => (
                default_export_dir(),
                String::new(),
                "Enter a .pgd or .pgt path. Legacy .pageform files still open.",
            ),
            PromptKind::OpenTemplate => (
                default_export_dir(),
                String::new(),
                "Choose a .pgt template. It opens as a new untitled document.",
            ),
        };
        self.file_prompt = Some(FilePrompt {
            kind,
            directory,
            file_name,
            replace: false,
            save_as_template: false,
        });
        self.status = status.to_string();
    }

    fn export_dialog(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.file_prompt.clone() else {
            return;
        };
        let mut prompt = prompt;
        let original_dir = prompt.directory.clone();
        let original_name = prompt.file_name.clone();
        let mut close = false;
        let mut save = false;
        let mut open_recent: Option<String> = None;
        let kind = prompt.kind;
        let recent = self.recent.clone();
        let response = egui::Modal::new(egui::Id::new("pageform_file")).show(ctx, |ui| {
            let submit = ui.input(|input| input.key_pressed(egui::Key::Enter));
            let cancel_key = ui.input(|input| input.key_pressed(egui::Key::Escape));
            ui.set_min_width(480.0);
            ui.heading(match kind {
                PromptKind::ExportPdf => "Export PDF",
                PromptKind::SaveProject => "Save PageForm",
                PromptKind::OpenProject => "Open PageForm",
                PromptKind::OpenTemplate => "New from Template",
            });
            ui.add_space(8.0);
            if is_open_prompt(kind) {
                ui.label("Path");
                let hint = if kind == PromptKind::OpenTemplate {
                    "/path/to/form.pgt"
                } else {
                    "/path/to/form.pgd"
                };
                ui.add(
                    egui::TextEdit::singleline(&mut prompt.file_name)
                        .desired_width(f32::INFINITY)
                        .hint_text(hint),
                );
                let shown: Vec<String> = if kind == PromptKind::OpenTemplate {
                    recent
                        .iter()
                        .filter(|path| is_template_path(std::path::Path::new(path)))
                        .cloned()
                        .collect()
                } else {
                    recent.clone()
                };
                if !shown.is_empty() {
                    ui.add_space(8.0);
                    ui.label("Recent");
                    for path in &shown {
                        if ui.button(path).clicked() {
                            open_recent = Some(path.clone());
                        }
                    }
                }
            } else {
                ui.label("Folder");
                ui.add(
                    egui::TextEdit::singleline(&mut prompt.directory).desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);
                ui.label("File name");
                ui.add(
                    egui::TextEdit::singleline(&mut prompt.file_name).desired_width(f32::INFINITY),
                );
                if kind == PromptKind::SaveProject {
                    ui.add_space(6.0);
                    ui.label("Type");
                    let was_template = prompt.save_as_template;
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut prompt.save_as_template, false, "Document (.pgd)");
                        ui.radio_value(&mut prompt.save_as_template, true, "Template (.pgt)");
                    });
                    if prompt.save_as_template != was_template {
                        prompt.file_name =
                            retarget_project_name(&prompt.file_name, prompt.save_as_template);
                    }
                }
                let path = if kind == PromptKind::ExportPdf {
                    ensure_extension(
                        export_file_path(&prompt.directory, &prompt.file_name),
                        "pdf",
                    )
                } else {
                    project_save_path(
                        &prompt.directory,
                        &prompt.file_name,
                        prompt.save_as_template,
                    )
                };
                ui.add_space(6.0);
                ui.label(path.display().to_string());
                if prompt.replace {
                    ui.add_space(6.0);
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 160, 80),
                        "That file already exists. Save again to replace it.",
                    );
                }
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let label = if is_open_prompt(kind) {
                    "Open"
                } else if prompt.replace {
                    "Replace"
                } else {
                    "Save"
                };
                if ui.button(label).clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
            if submit {
                save = true;
            }
            if cancel_key {
                close = true;
            }
        });
        if prompt.directory != original_dir || prompt.file_name != original_name {
            prompt.replace = false;
        }
        if let Some(path) = open_recent {
            self.file_prompt = None;
            if kind == PromptKind::OpenTemplate {
                self.read_project_file_as_untitled(std::path::Path::new(&path));
            } else {
                self.read_project_file(std::path::Path::new(&path));
            }
            return;
        }
        if close || response.should_close() {
            self.file_prompt = None;
            self.after_save = None;
            self.status = match kind {
                PromptKind::OpenProject => "Open cancelled".to_string(),
                PromptKind::OpenTemplate => "New from Template cancelled".to_string(),
                PromptKind::SaveProject => "Save cancelled".to_string(),
                PromptKind::ExportPdf => "Export cancelled".to_string(),
            };
            return;
        }
        if save {
            if is_open_prompt(kind) {
                let path = prompt.file_name.trim().to_string();
                let as_untitled = kind == PromptKind::OpenTemplate;
                self.file_prompt = None;
                if path.is_empty() {
                    self.status = "Enter a file path to open".to_string();
                } else if as_untitled {
                    self.read_project_file_as_untitled(std::path::Path::new(&path));
                } else {
                    self.read_project_file(std::path::Path::new(&path));
                }
                return;
            }
            let path = if kind == PromptKind::ExportPdf {
                ensure_extension(
                    export_file_path(&prompt.directory, &prompt.file_name),
                    "pdf",
                )
            } else {
                project_save_path(
                    &prompt.directory,
                    &prompt.file_name,
                    prompt.save_as_template,
                )
            };
            if path.exists() && !prompt.replace {
                prompt.replace = true;
                self.file_prompt = Some(prompt);
                return;
            }
            self.file_prompt = None;
            match kind {
                PromptKind::ExportPdf => {
                    self.export_path = path.display().to_string();
                    self.write_export();
                }
                PromptKind::SaveProject => self.write_project_file(&path),
                PromptKind::OpenProject | PromptKind::OpenTemplate => {}
            }
            return;
        }
        self.file_prompt = Some(prompt);
    }

    fn write_project_file(&mut self, path: &std::path::Path) {
        let template = is_template_path(path);
        let kind = if template {
            ProjectKind::Template
        } else {
            ProjectKind::Document
        };
        match self.doc.to_project_ron(kind) {
            Ok(text) => match std::fs::write(path, text) {
                Ok(()) => {
                    self.remember_recent(path);
                    if template {
                        self.after_save = None;
                        self.saved_follow_up = false;
                        self.status = if self.project_path.is_none() {
                            format!(
                                "Saved template {}. Save will store a new .pgd document.",
                                path.display()
                            )
                        } else {
                            format!("Saved template {}.", path.display())
                        };
                    } else {
                        self.project_path = Some(path.display().to_string());
                        self.doc.mark_clean();
                        self.status = format!("Saved {}", path.display());
                        self.saved_follow_up = true;
                    }
                }
                Err(err) => {
                    self.after_save = None;
                    self.status = format!("Could not write {}: {err}", path.display());
                }
            },
            Err(err) => {
                self.after_save = None;
                self.status = format!("Could not save: {err}");
            }
        }
    }

    fn read_project_file(&mut self, path: &std::path::Path) {
        self.load_project(path, false);
    }

    /// File > New from Template. The chosen file stays unchanged.
    fn read_project_file_as_untitled(&mut self, path: &std::path::Path) {
        self.load_project(path, true);
    }

    fn load_project(&mut self, path: &std::path::Path, force_untitled: bool) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => {
                self.status = format!("Could not open {}: {err}", path.display());
                return;
            }
        };
        match Document::from_project_ron(&text) {
            Ok(loaded) => {
                let untitled = force_untitled
                    || is_template_path(path)
                    || loaded.kind == ProjectKind::Template;
                self.doc = loaded.document;
                self.selection.clear();
                self.inline = None;
                self.drag = None;
                self.fitted = false;
                self.navigated = false;
                self.view.page_height = self.doc.page().height;
                self.remember_recent(path);
                if untitled {
                    self.project_path = None;
                    self.doc.mark_dirty();
                    self.status = format!(
                        "Opened {} as a new document. Save stores a .pgd and does not change this file.",
                        path.display()
                    );
                } else {
                    self.project_path = Some(path.display().to_string());
                    self.status = format!("Opened {}", path.display());
                }
            }
            Err(err) => self.status = err,
        }
    }

    fn remember_recent(&mut self, path: &std::path::Path) {
        let text = path.display().to_string();
        self.recent.retain(|item| item != &text);
        self.recent.insert(0, text);
        self.recent.truncate(8);
        self.persist_panel = true;
    }

    fn print_current(&mut self) {
        let bytes = match export_pdf(&self.doc) {
            Ok(bytes) => bytes,
            Err(err) => {
                self.status = format!("Print failed: {err}");
                return;
            }
        };
        let path = std::env::temp_dir().join(format!("pageform-print-{}.pdf", std::process::id()));
        if let Err(err) = std::fs::write(&path, bytes) {
            self.status = format!("Could not write a temporary PDF: {err}");
            return;
        }
        self.status = send_to_printer(&path);
    }

    fn unsaved_dialog(&mut self, ctx: &egui::Context) {
        let Some(action) = self.unsaved.clone() else {
            return;
        };
        let name = self
            .project_path
            .as_deref()
            .map(export_file_name)
            .unwrap_or_else(|| "Untitled".to_string());
        let mut choice: Option<UnsavedChoice> = None;
        let response = egui::Modal::new(egui::Id::new("pageform_unsaved")).show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.heading("Unsaved changes");
            ui.add_space(8.0);
            ui.label(format!("Save changes to {name} before continuing?"));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    choice = Some(UnsavedChoice::Save);
                }
                if ui.button("Don't Save").clicked() {
                    choice = Some(UnsavedChoice::Discard);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(UnsavedChoice::Cancel);
                }
            });
        });
        if response.should_close() {
            choice = Some(UnsavedChoice::Cancel);
        }
        match choice {
            Some(UnsavedChoice::Save) => {
                self.unsaved = None;
                self.after_save = Some(action);
                self.save_project();
            }
            Some(UnsavedChoice::Discard) => {
                self.unsaved = None;
                self.doc.mark_clean();
                self.run_file_action(action);
            }
            Some(UnsavedChoice::Cancel) => self.unsaved = None,
            None => {}
        }
    }

    fn write_export(&mut self) {
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
        if self.selection.is_empty() {
            return;
        }
        let ids = std::mem::take(&mut self.selection);
        let count = ids.len();
        for id in ids {
            self.doc.remove(id);
        }
        self.drag = None;
        self.status = if count == 1 {
            "Deleted field".to_string()
        } else {
            format!("Deleted {count} items")
        };
        self.dismiss_unpinned();
    }

    fn primary(&self) -> Option<FieldId> {
        self.selection.last().copied()
    }

    fn set_properties_open(&mut self, open: bool) {
        if self.properties_open != open {
            self.properties_open = open;
            self.persist_panel = true;
        }
    }

    fn set_properties_pinned(&mut self, pinned: bool) {
        if self.properties_pinned != pinned {
            self.properties_pinned = pinned;
            self.persist_panel = true;
            self.status = if pinned {
                "Properties pinned".to_string()
            } else {
                "Properties unpinned. Click empty canvas to close.".to_string()
            };
        }
    }

    fn open_properties(&mut self) {
        self.set_properties_open(true);
    }

    /// Start canvas editing for an item that shows text. Returns false for shapes and other items.
    fn begin_inline(&mut self, id: FieldId) -> bool {
        if self.inline.as_ref().is_some_and(|edit| edit.id == id) {
            return true;
        }
        let started = self.doc.field(id).and_then(|field| {
            inline_target(field.kind(), field.style().multiline).map(|(target, multiline)| {
                let text = match target {
                    InlineTarget::Caption => field.caption().to_string(),
                    InlineTarget::DefaultValue => field.style().default_value.clone(),
                };
                (target, multiline, text)
            })
        });
        let Some((target, multiline, text)) = started else {
            return false;
        };
        if self.inline.is_some() {
            self.finish_inline();
        }
        self.inline = Some(InlineEdit {
            id,
            target,
            buffer: text.clone(),
            original: text,
            multiline,
            request_focus: true,
        });
        self.name_for = None;
        self.sync_field_buffers();
        if self.status_before_inline.is_none() {
            self.status_before_inline = Some(self.status.clone());
        }
        self.status = if multiline {
            "Editing text. Enter finishes, Shift+Enter adds a line, Esc cancels.".to_string()
        } else {
            "Editing text. Enter or a click outside finishes, Esc cancels.".to_string()
        };
        true
    }

    fn inline_hides_text(&self, id: FieldId) -> bool {
        self.inline.as_ref().is_some_and(|edit| edit.id == id)
    }

    fn apply_inline(&mut self) {
        let Some(edit) = self.inline.as_ref() else {
            return;
        };
        let id = edit.id;
        let target = edit.target;
        let text = edit.buffer.clone();
        match target {
            InlineTarget::Caption => {
                let _ = self.doc.set_caption(id, &text);
            }
            InlineTarget::DefaultValue => self.doc.set_default_value(id, &text),
        }
        if self.name_for == Some(id) {
            match target {
                InlineTarget::Caption => self.caption_buf.clone_from(&text),
                InlineTarget::DefaultValue => self.value_buf.clone_from(&text),
            }
        }
    }

    fn cancel_inline(&mut self) {
        let Some(edit) = self.inline.take() else {
            return;
        };
        match edit.target {
            InlineTarget::Caption => {
                let _ = self.doc.set_caption(edit.id, &edit.original);
            }
            InlineTarget::DefaultValue => self.doc.set_default_value(edit.id, &edit.original),
        }
        if self.name_for == Some(edit.id) {
            match edit.target {
                InlineTarget::Caption => self.caption_buf = edit.original,
                InlineTarget::DefaultValue => self.value_buf = edit.original,
            }
        }
        self.clear_inline_status();
    }

    fn finish_inline(&mut self) {
        self.apply_inline();
        self.inline = None;
        self.clear_inline_status();
    }

    fn clear_inline_status(&mut self) {
        if let Some(previous) = self.status_before_inline.take() {
            self.status = previous;
        } else if self.status.starts_with("Editing text.") {
            self.status.clear();
        }
    }

    /// Sidebar edits land in the canvas editor when both show the same item.
    fn note_sidebar_text(&mut self, id: FieldId, target: InlineTarget, text: &str) {
        if let Some(edit) = &mut self.inline {
            if edit.id == id && edit.target == target && edit.buffer != text {
                edit.buffer = text.to_string();
            }
        }
    }

    /// Unpinned panels close when the user is done with the selection.
    fn dismiss_unpinned(&mut self) {
        if self.properties_open && !self.properties_pinned {
            self.set_properties_open(false);
        }
    }

    fn hit_field_id(&self, pdf: PdfPoint) -> Option<FieldId> {
        let radius = 8.0 / self.view.zoom;
        self.doc
            .fields()
            .iter()
            .rev()
            .find(|field| field.contains_point(pdf, radius))
            .map(|field| field.id())
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            for label in MENU_BAR {
                match *label {
                    "File" => {
                        ui.menu_button(*label, |ui| self.file_menu(ui));
                    }
                    "Edit" => {
                        ui.menu_button(*label, |ui| self.edit_menu(ui));
                    }
                    "Insert" => {
                        ui.menu_button(*label, |ui| self.insert_menu(ui));
                    }
                    "View" => {
                        ui.menu_button(*label, |ui| self.view_menu(ui));
                    }
                    "Help" => {
                        ui.menu_button(*label, |ui| self.help_menu(ui));
                    }
                    _ => unreachable!("unknown menu {label}"),
                }
            }
        });
    }

    fn file_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(200.0);
        for label in FILE_MENU {
            if label.is_empty() {
                ui.separator();
                continue;
            }
            match *label {
                "Open" => {
                    if ui.button("Open").clicked() {
                        self.ask_or_run(PendingFileAction::Open);
                        ui.close();
                    }
                    if self.recent.is_empty() {
                        ui.add_enabled(false, egui::Button::new("Open Recent"))
                            .on_disabled_hover_text("No recent files");
                    } else {
                        ui.menu_button("Open Recent", |ui| {
                            ui.set_min_width(280.0);
                            for path in self.recent.clone() {
                                if ui.button(&path).clicked() {
                                    self.ask_or_run(PendingFileAction::OpenPath(path));
                                    ui.close();
                                }
                            }
                        });
                    }
                }
                "Save" => {
                    let shortcut = command_shortcut(ui, egui::Key::S);
                    if menu_command(ui, "Save", Some(&shortcut), false) {
                        self.save_project();
                    }
                }
                "Save As" => {
                    let shortcut = ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(
                        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                        egui::Key::S,
                    ));
                    if menu_command(ui, "Save As", Some(&shortcut), false) {
                        self.save_project_as();
                    }
                }
                "Print" => {
                    let shortcut = command_shortcut(ui, egui::Key::P);
                    if menu_command(ui, "Print", Some(&shortcut), false) {
                        self.print_current();
                    }
                }
                "Export PDF" => {
                    let tip = format!(
                        "Write a fillable PDF to {}. Guides and margins stay on the canvas.",
                        self.export_path
                    );
                    let export = ui.button(*label).on_hover_text(tip);
                    if export.clicked() {
                        self.export_current();
                        ui.close();
                    }
                }
                "Exit" => {
                    if ui.button(*label).clicked() {
                        self.ask_or_run(PendingFileAction::Exit);
                        ui.close();
                    }
                }
                "New" => {
                    ui.menu_button("New", |ui| self.new_menu(ui));
                }
                other => self.stub_item(ui, other),
            }
        }
    }

    fn edit_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(200.0);
        for label in EDIT_MENU {
            if label.is_empty() {
                ui.separator();
                continue;
            }
            match *label {
                "Select" => {
                    if menu_command(ui, label, Some("Esc"), self.tool == Tool::Select) {
                        self.tool = Tool::Select;
                    }
                }
                "Select All" => {
                    let shortcut = command_shortcut(ui, egui::Key::A);
                    if menu_command(ui, label, Some(&shortcut), false) {
                        self.select_all();
                    }
                }
                "Copy" => {
                    let shortcut = command_shortcut(ui, egui::Key::C);
                    if menu_command_enabled(ui, label, Some(&shortcut), !self.selection.is_empty())
                    {
                        self.copy_selection();
                        ui.ctx().copy_text(ITEM_CLIPBOARD_MARKER.to_string());
                    }
                }
                "Cut" => {
                    let shortcut = command_shortcut(ui, egui::Key::X);
                    if menu_command_enabled(ui, label, Some(&shortcut), !self.selection.is_empty())
                    {
                        self.cut_selection();
                        ui.ctx().copy_text(ITEM_CLIPBOARD_MARKER.to_string());
                    }
                }
                "Paste" => {
                    let shortcut = command_shortcut(ui, egui::Key::V);
                    if menu_command_enabled(ui, label, Some(&shortcut), !self.clipboard.is_empty())
                    {
                        self.paste_clipboard();
                    }
                }
                "Delete" => {
                    let delete = ui.add_enabled(
                        !self.selection.is_empty(),
                        egui::Button::new(*label).shortcut_text("Del"),
                    );
                    if delete.clicked() {
                        self.delete_selection();
                        ui.close();
                    }
                }
                "Align" => {
                    ui.menu_button("Align", |ui| self.align_menu(ui));
                }
                "Preferences" => {
                    if ui.button(*label).clicked() {
                        self.show_preferences = true;
                        ui.close();
                    }
                }
                other => self.stub_item(ui, other),
            }
        }
    }

    fn help_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(160.0);
        for label in HELP_MENU {
            match *label {
                "Help" => {
                    if ui.button(*label).clicked() {
                        self.show_help = true;
                        ui.close();
                    }
                }
                "About" => {
                    if ui.button(*label).clicked() {
                        self.show_about = true;
                        ui.close();
                    }
                }
                other => self.stub_item(ui, other),
            }
        }
    }

    fn align_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(220.0);
        self.align_commands(ui, true);
    }

    /// Align, distribute, and match size. `close_menu` closes the menu after a command.
    fn align_commands(&mut self, ui: &mut egui::Ui, close_menu: bool) {
        let count = self.selection.len();
        let align_ok = count >= 2;
        let distribute_ok = count >= 3;
        let need = if align_ok {
            "Apply to the selection."
        } else {
            "Select two or more items."
        };
        for (label, edge) in ALIGN_EDGES {
            let response = ui
                .add_enabled(align_ok, egui::Button::new(*label))
                .on_hover_text(need)
                .on_disabled_hover_text("Select two or more items.");
            if response.clicked() {
                let ids = self.selection.clone();
                self.doc.align_fields(&ids, *edge);
                self.status = format!("Aligned {label}");
                if close_menu {
                    ui.close();
                }
            }
        }
        ui.separator();
        for (label, horizontal) in [
            ("Distribute horizontally", true),
            ("Distribute vertically", false),
        ] {
            let response = ui
                .add_enabled(distribute_ok, egui::Button::new(label))
                .on_disabled_hover_text("Select three or more items.");
            if response.clicked() {
                let ids = self.selection.clone();
                self.doc.distribute_fields(&ids, horizontal);
                self.status = label.to_string();
                if close_menu {
                    ui.close();
                }
            }
        }
        for (label, width, height) in [("Match width", true, false), ("Match height", false, true)]
        {
            let response = ui
                .add_enabled(align_ok, egui::Button::new(label))
                .on_disabled_hover_text("Select two or more items.");
            if response.clicked() {
                let ids = self.selection.clone();
                let reference = ids.len() - 1;
                self.doc.match_field_size(&ids, reference, width, height);
                self.status = label.to_string();
                if close_menu {
                    ui.close();
                }
            }
        }
    }

    fn new_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(180.0);
        for item in NEW_MENU {
            if ui.button(*item).clicked() {
                if *item == "New Blank Document" {
                    self.request_new_blank();
                } else if *item == "New from Template" {
                    self.ask_or_run(PendingFileAction::NewFromTemplate);
                } else {
                    self.mark_not_implemented(item);
                }
                ui.close();
            }
        }
    }

    fn toolbar_new(&mut self, ui: &mut egui::Ui) {
        let response = icons::icon_menu(ui, icons::Icon::New, false, "New");
        egui::Popup::menu(&response).show(|ui| self.new_menu(ui));
    }

    fn stub_item(&mut self, ui: &mut egui::Ui, label: &str) {
        let button = ui.button(label).on_hover_text("Not implemented yet");
        if button.clicked() {
            self.mark_not_implemented(label);
            ui.close();
        }
    }

    fn mark_not_implemented(&mut self, command: &str) {
        self.status = format!("{command} is not implemented yet");
    }

    fn insert_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(240.0);
        for kind in sorted_insert_kinds() {
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
        self.selection = vec![id];
        self.drag = None;
        self.suppress_place_click = true;
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
        self.primary().and_then(|id| {
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
        let mut open = self.properties_open;
        if ui
            .checkbox(&mut open, "Properties")
            .on_hover_text("Show or hide the properties panel.")
            .changed()
        {
            self.set_properties_open(open);
        }
        ui.separator();
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

    fn preferences_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_preferences {
            return;
        }
        let mut close = false;
        let response = egui::Modal::new(egui::Id::new("pageform_preferences")).show(ctx, |ui| {
            ui.set_min_width(560.0);
            ui.set_min_height(280.0);
            ui.heading(egui::RichText::new("Preferences").size(28.0));
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(12.0);
            ui.label(egui::RichText::new("Theme").size(18.0));
            ui.add_space(6.0);
            let before = self.dark;
            ui.radio_value(&mut self.dark, true, "Dark");
            ui.radio_value(&mut self.dark, false, "Light");
            if self.dark != before {
                self.persist_theme = true;
            }
            ui.add_space(8.0);
            ui.label("Saved on this computer. Dark is the default.");
            ui.add_space(18.0);
            ui.label(egui::RichText::new("Properties panel").size(18.0));
            ui.add_space(6.0);
            let mut panel_open = self.properties_open;
            if ui
                .checkbox(&mut panel_open, "Open by default")
                .on_hover_text("Show the properties panel the next time PageForm opens.")
                .changed()
            {
                self.set_properties_open(panel_open);
            }
            let mut pinned = self.properties_pinned;
            if ui
                .checkbox(&mut pinned, "Pinned by default")
                .on_hover_text(
                    "Pinned panels stay open. Unpinned panels close when you click empty canvas.",
                )
                .changed()
            {
                self.set_properties_pinned(pinned);
            }
            ui.add_space(8.0);
            ui.label(
                "Saved on this computer. The panel starts hidden and unpinned.",
            );
            ui.add_space(18.0);
            let mut show_labels = self.doc.radio_labels();
            if ui
                .checkbox(&mut show_labels, "Radio labels")
                .on_hover_text(
                    "Text beside each radio button. Off hides those labels on the canvas and in the export.",
                )
                .changed()
            {
                self.doc.set_radio_labels(show_labels);
                self.status = if show_labels {
                    "Radio labels on".to_string()
                } else {
                    "Radio labels off".to_string()
                };
            }
            ui.add_space(22.0);
            if ui.button("Close").clicked() {
                close = true;
            }
        });
        if close || response.should_close() {
            self.show_preferences = false;
        }
    }

    fn about_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut close = false;
        let response = egui::Modal::new(egui::Id::new("pageform_about")).show(ctx, |ui| {
            ui.set_min_width(460.0);
            ui.vertical_centered(|ui| {
                placeholder_icon(ui);
                ui.add_space(8.0);
                ui.heading("PageForm");
                ui.add_space(4.0);
                ui.label(ABOUT_TAGLINE);
                ui.add_space(6.0);
                ui.label(format!("Version {}", app_version()));
                ui.add_space(12.0);
                if ui.button("OK").clicked() {
                    close = true;
                }
            });
        });
        if close || response.should_close() {
            self.show_about = false;
        }
    }

    /// Separate OS window when the backend supports viewports, so the FAQ can
    /// sit on another screen. A titled egui window is the fallback.
    fn help_window(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }
        let dark = self.dark;
        let host = ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("pageform-help"),
            egui::ViewportBuilder::default()
                .with_title("PageForm Help")
                .with_inner_size([520.0, 640.0])
                .with_min_inner_size([380.0, 320.0])
                .with_close_button(true)
                .with_decorations(true),
            |ctx, class| {
                if class == egui::ViewportClass::Embedded {
                    return HelpHost::Embedded;
                }
                crate::theme::apply(ctx, dark);
                egui::CentralPanel::default().show(ctx, |ui| {
                    help_faq(ui, dark);
                });
                let close = ctx.input(|input| input.viewport().close_requested());
                HelpHost::Native { close }
            },
        );
        match host {
            HelpHost::Embedded => {
                egui::Window::new("PageForm Help")
                    .collapsible(true)
                    .resizable(true)
                    .default_size([520.0, 640.0])
                    .open(&mut self.show_help)
                    .show(ctx, |ui| {
                        help_faq(ui, self.dark);
                    });
            }
            HelpHost::Native { close } => {
                if close {
                    self.show_help = false;
                }
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            self.toolbar_documents(ui);
            ui.separator();
            self.toolbar_tools(ui);
            ui.separator();
            self.toolbar_view(ui);
        });
    }

    fn toolbar_documents(&mut self, ui: &mut egui::Ui) {
        for label in icons::DOC_COMMANDS {
            match *label {
                "New" => self.toolbar_new(ui),
                "Open"
                    if icons::icon_button(
                        ui,
                        icons::Icon::Open,
                        false,
                        "Open a .pgd or .pgt file",
                    )
                    .clicked() =>
                {
                    self.ask_or_run(PendingFileAction::Open);
                }
                "Save"
                    if icons::icon_button(ui, icons::Icon::Save, false, "Save the project")
                        .clicked() =>
                {
                    self.save_project();
                }
                "Print"
                    if icons::icon_button(ui, icons::Icon::Print, false, "Print the page")
                        .clicked() =>
                {
                    self.print_current();
                }
                _ => {}
            }
        }
    }

    fn toolbar_tools(&mut self, ui: &mut egui::Ui) {
        let select =
            icons::icon_button(ui, icons::Icon::Select, self.tool == Tool::Select, "Select");
        if select.clicked() {
            self.tool = Tool::Select;
        }
        if icons::icon_button(ui, icons::Icon::InsertPage, false, "Insert Page").clicked() {
            self.mark_not_implemented("Insert Page");
        }
        for label in icons::INSERT_TOOLS {
            if *label == "Shapes" {
                self.shapes_menu(ui);
            } else if let Some(kind) = kind_for_toolbar_label(label) {
                let tool = Tool::for_kind(kind);
                let response =
                    icons::icon_button(ui, icon_for_kind(kind), self.tool == tool, kind.label())
                        .on_hover_text(toolbar_tip(kind));
                if response.clicked() {
                    self.tool = tool;
                }
            }
        }
    }

    fn shapes_menu(&mut self, ui: &mut egui::Ui) {
        let response = icons::icon_menu(
            ui,
            icons::Icon::Shapes,
            self.tool == Tool::Square || self.tool == Tool::Rectangle,
            "Click to place a default shape, or drag to size it.",
        );
        egui::Popup::menu(&response).show(|ui| {
            ui.set_min_width(160.0);
            if ui
                .selectable_label(self.tool == Tool::Square, "Square")
                .on_hover_text("Click to place a default square, or drag to size it. The sides stay equal.")
                .clicked()
            {
                self.tool = Tool::Square;
                ui.close();
            }
            if ui
                .selectable_label(self.tool == Tool::Rectangle, "Rectangle")
                .on_hover_text(
                    "Click to place a default rectangle, or drag to size it. Width and height are independent.",
                )
                .clicked()
            {
                self.tool = Tool::Rectangle;
                ui.close();
            }
        });
    }

    fn toolbar_view(&mut self, ui: &mut egui::Ui) {
        let response = icons::icon_menu(ui, icons::Icon::Grid, false, "Grid size");
        let current = self.grid_size;
        egui::Popup::menu(&response).show(|ui| {
            ui.set_min_width(140.0);
            for size in GridSize::ALL {
                if ui
                    .selectable_label(current == size, size.label())
                    .on_hover_text(size.minor_label())
                    .clicked()
                {
                    self.grid_size = size;
                    ui.close();
                }
            }
        });
        for ((label, icon), detail) in icons::VIEW_TOGGLES
            .iter()
            .copied()
            .zip(icons::VIEW_ICONS)
            .zip([
                "Grid dots",
                "Guide lines. Canvas only.",
                "Print-safe margin. Canvas only.",
                "Page number. Canvas only.",
                "Snap to the minor grid spacing.",
            ])
        {
            self.toggle_view(ui, icon, label, detail);
        }
        let theme_icon = if self.dark {
            icons::Icon::Moon
        } else {
            icons::Icon::Sun
        };
        let theme_tip = format!(
            "{} - {}",
            crate::theme::toggle_label(self.dark),
            crate::theme::toggle_tip(self.dark)
        );
        if icons::icon_button(ui, theme_icon, false, &theme_tip).clicked() {
            self.set_dark(!self.dark);
        }
        let properties_tip = if self.properties_open {
            "Properties. Click to hide the panel."
        } else {
            "Properties. Click to show the panel."
        };
        if icons::icon_button(
            ui,
            icons::Icon::Properties,
            self.properties_open,
            properties_tip,
        )
        .clicked()
        {
            self.set_properties_open(!self.properties_open);
        }
    }

    fn toggle_view(&mut self, ui: &mut egui::Ui, icon: icons::Icon, tip: &str, detail: &str) {
        let on = match tip {
            "Dots" => self.dots_visible,
            "Guides" => self.guides_visible,
            "Margins" => self.margins_visible,
            "Page numbers" => self.page_numbers_visible,
            "Snap" => self.snap,
            _ => return,
        };
        if icons::icon_button(ui, icon, on, tip)
            .on_hover_text(detail)
            .clicked()
        {
            match tip {
                "Dots" => self.dots_visible = !self.dots_visible,
                "Guides" => self.guides_visible = !self.guides_visible,
                "Margins" => self.margins_visible = !self.margins_visible,
                "Page numbers" => self.page_numbers_visible = !self.page_numbers_visible,
                "Snap" => self.snap = !self.snap,
                _ => {}
            }
        }
    }

    fn finish_marquee(&mut self, start: PdfPoint, current: PdfPoint, extend: bool) {
        if is_click(&self.view, start, current) {
            if !extend {
                self.selection.clear();
                self.dismiss_unpinned();
            }
            return;
        }
        let band = marquee_rect(start, current);
        let hits: Vec<FieldId> = self
            .doc
            .fields()
            .iter()
            .filter(|field| rects_hit(field.rect(), band))
            .map(|field| field.id())
            .collect();
        if extend {
            for id in hits {
                if !self.selection.contains(&id) {
                    self.selection.push(id);
                }
            }
        } else {
            self.selection = hits;
        }
        let count = self.selection.len();
        self.status = if count == 1 {
            "Selected 1 item".to_string()
        } else {
            format!("Selected {count} items")
        };
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

    /// Keep a fresh page centered in the canvas, including after the sidebar opens.
    fn keep_page_centered(&mut self, rect: Rect) {
        let size = rect.size();
        let size_changed = self.canvas_size.is_none_or(|previous| {
            (previous.x - size.x).abs() > 0.5 || (previous.y - size.y).abs() > 0.5
        });
        if rect.width() > 32.0
            && rect.height() > 32.0
            && (!self.fitted || (!self.navigated && size_changed))
        {
            self.fit(rect);
        }
        self.canvas_size = Some(size);
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        self.canvas_rect = Some(response.rect);
        self.keep_page_centered(response.rect);
        self.handle_input(&response, ui.ctx());
        self.paint(&painter, response.rect);
        self.hover_cursor(&response, ui.ctx());
        self.field_context_menu(&response);
        self.inline_editor(ui);
    }

    /// Shortcuts are read before widgets so the first Ctrl/Cmd chord is not lost.
    /// Copy, cut, and paste arrive as events, not as key presses.
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let mut copy = false;
        let mut cut = false;
        let mut paste = false;
        ctx.input_mut(|input| {
            input.events.retain(|event| match event {
                egui::Event::Copy => {
                    copy = true;
                    false
                }
                egui::Event::Cut => {
                    cut = true;
                    false
                }
                egui::Event::Paste(_) => {
                    paste = true;
                    false
                }
                _ => true,
            });
        });
        // egui emits Paste only when the OS clipboard already has text, and it
        // does not also emit Key::V. Accept either, but only paste once.
        let paste_key = ctx.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::V,
            ))
        });
        if cut {
            self.cut_selection();
            ctx.copy_text(ITEM_CLIPBOARD_MARKER.to_string());
        } else if copy {
            self.copy_selection();
            ctx.copy_text(ITEM_CLIPBOARD_MARKER.to_string());
        } else if paste || paste_key {
            self.paste_clipboard();
        }
        let save_as = egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::S,
        );
        let save = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S);
        let print = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::P);
        let select_all = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::A);
        if ctx.input_mut(|input| input.consume_shortcut(&save_as)) {
            self.save_project_as();
        } else if ctx.input_mut(|input| input.consume_shortcut(&save)) {
            self.save_project();
        } else if ctx.input_mut(|input| input.consume_shortcut(&print)) {
            self.print_current();
        } else if ctx.input_mut(|input| input.consume_shortcut(&select_all)) {
            self.select_all();
        }
    }

    fn handle_input(&mut self, response: &egui::Response, ctx: &egui::Context) {
        if response.hovered() {
            let scroll_y = ctx.input(|input| input.raw_scroll_delta.y);
            if scroll_y.abs() > 0.0 {
                if let Some(pos) = ctx.input(|input| input.pointer.hover_pos()) {
                    let factor = if scroll_y > 0.0 { 1.1 } else { 1.0 / 1.1 };
                    let zoom = (self.view.zoom * factor).clamp(0.2, 8.0);
                    self.view.zoom_at(pos.x, pos.y, zoom);
                    self.navigated = true;
                }
            }
        }

        if !ctx.wants_keyboard_input()
            && ctx
                .input(|input| input.key_pressed(Key::Delete) || input.key_pressed(Key::Backspace))
        {
            self.delete_selection();
        }
        if !ctx.wants_keyboard_input() && ctx.input(|input| input.key_pressed(Key::Escape)) {
            self.drag = None;
            self.tool = Tool::Select;
        }

        let primary_pressed = ctx.input(|input| input.pointer.primary_pressed());
        let primary_down = ctx.input(|input| input.pointer.primary_down());
        let primary_released = ctx.input(|input| input.pointer.primary_released());
        let middle_pressed = ctx.input(|input| input.pointer.button_pressed(PointerButton::Middle));
        let space = ctx.input(|input| input.key_down(Key::Space));

        if response.double_clicked() && self.tool == Tool::Select {
            if let Some(pdf) = self.pointer_pdf(ctx) {
                if let Some(id) = self.hit_text_item(pdf) {
                    self.selection = vec![id];
                    self.drag = None;
                    self.begin_inline(id);
                    return;
                }
            }
        }
        if response.secondary_clicked() {
            if let Some(pdf) = self.pointer_pdf(ctx) {
                if let Some(id) = self.hit_field_id(pdf) {
                    if !self.selection.contains(&id) {
                        self.selection = vec![id];
                    }
                    self.context_field = Some(id);
                } else {
                    self.context_field = None;
                }
            }
        }

        let (ignore_primary, still_suppress) = ignore_place_click(
            self.suppress_place_click,
            ctx.input(|input| input.pointer.primary_down()),
        );
        self.suppress_place_click = still_suppress;

        if middle_pressed && response.hovered() {
            self.drag = Some(Drag::Pan);
        } else if primary_pressed && response.hovered() && !ignore_primary {
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
                if delta != Vec2::ZERO {
                    self.navigated = true;
                }
                self.view.origin_x += delta.x;
                self.view.origin_y += delta.y;
            } else {
                self.drag = None;
            }
        }

        if primary_down && !ignore_primary {
            self.update_drag(ctx);
        }

        if primary_released && !ignore_primary {
            if let Some(Drag::Place { start, kind }) = self.drag.clone() {
                if let Some(current) = self.pointer_pdf(ctx) {
                    let id = self.finish_place(start, current, kind);
                    self.selection = vec![id];
                    self.tool = Tool::Select;
                    self.status = format!("Placed {}", kind.label());
                }
            }
            if let Some(Drag::Marquee { start, extend }) = self.drag.clone() {
                if let Some(current) = self.pointer_pdf(ctx) {
                    self.finish_marquee(start, current, extend);
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
                if let Some(id) = self.primary() {
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
                if let Some(id) = self.hit_field_id(pdf) {
                    let rect = self.doc.field(id).map(|field| field.rect());
                    let extend = ctx.input(|input| {
                        input.modifiers.shift || input.modifiers.command || input.modifiers.ctrl
                    });
                    self.selection = selection_after_click(&self.selection, id, extend);
                    if let Some(rect) = rect.filter(|_| self.selection.contains(&id)) {
                        let origins = self
                            .selection
                            .iter()
                            .filter_map(|item| {
                                self.doc.field(*item).map(|field| (*item, field.rect()))
                            })
                            .collect();
                        self.drag = Some(Drag::Move {
                            grab_id: id,
                            grab_x: pdf.x - rect.x,
                            grab_y: pdf.y - rect.y,
                            origins,
                        });
                    } else {
                        self.drag = None;
                    }
                } else {
                    let extend = ctx.input(|input| {
                        input.modifiers.shift || input.modifiers.command || input.modifiers.ctrl
                    });
                    self.drag = Some(Drag::Marquee { start: pdf, extend });
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
        if kind == FieldKind::Line && !is_click(&self.view, start, current) {
            let grid = self.snap_grid();
            return self
                .doc
                .add_line(snap_point(start, grid), snap_point(current, grid));
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
        match self.drag.clone() {
            Some(Drag::Move {
                grab_id,
                grab_x,
                grab_y,
                origins,
            }) => {
                let Some(origin) = origins
                    .iter()
                    .find(|(id, _)| *id == grab_id)
                    .map(|(_, rect)| *rect)
                else {
                    return;
                };
                let moved = move_rect(origin, pdf, grab_x, grab_y, grid, &page);
                let dx = moved.x - origin.x;
                let dy = moved.y - origin.y;
                for (id, origin) in origins {
                    self.doc.set_bounds(
                        id,
                        RectPt {
                            x: origin.x + dx,
                            y: origin.y + dy,
                            w: origin.w,
                            h: origin.h,
                        },
                    );
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

    fn hit_text_item(&self, pdf: PdfPoint) -> Option<FieldId> {
        if let Some(id) = self.hit_field_id(pdf) {
            let editable = self.doc.field(id).is_some_and(|field| {
                inline_target(field.kind(), field.style().multiline).is_some()
            });
            if editable {
                return Some(id);
            }
        }
        self.doc.fields().iter().rev().find_map(|field| {
            if !matches!(field.kind(), FieldKind::Radio | FieldKind::Checkbox) {
                return None;
            }
            adjacent_label_rect(field.rect())
                .contains(pdf)
                .then_some(field.id())
        })
    }

    fn inline_editor(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.inline.as_ref().map(|edit| edit.id) else {
            return;
        };
        let Some(field) = self.doc.field(id) else {
            self.inline = None;
            return;
        };
        let kind = field.kind();
        let style = field.style().clone();
        let multiline = self.inline.as_ref().is_some_and(|edit| edit.multiline);
        let rect = inline_screen_rect(&self.view, field.rect(), kind, style.font_size, multiline);
        let font = canvas_font(&style, self.view.zoom);
        let colors = crate::theme::canvas_colors(self.dark);
        let light_fill = style
            .fill
            .is_some_and(|fill| fill.r + fill.g + fill.b > 1.5);
        let color = canvas_text_color(
            style.text_color,
            light_fill && !matches!(kind, FieldKind::Radio | FieldKind::Checkbox),
            &colors,
        );
        let cover = if let Some(fill) = style.fill {
            if matches!(kind, FieldKind::Radio | FieldKind::Checkbox) {
                colors.page
            } else {
                canvas_fill(fill, &colors)
            }
        } else {
            colors.page
        };
        ui.painter().rect_filled(rect, 0.0, cover);
        let request_focus = self.inline.as_ref().is_some_and(|edit| edit.request_focus);
        let edit_id = egui::Id::new(("canvas-inline", id.0));
        let hint = inline_hint(kind, &style, field.name());
        let horizontal = match style.align {
            TextAlign::Left => egui::Align::LEFT,
            TextAlign::Center => egui::Align::Center,
            TextAlign::Right => egui::Align::RIGHT,
        };
        let label_beside = matches!(kind, FieldKind::Radio | FieldKind::Checkbox);
        // paint_label insets single-line text by 4 px and centers it. Multiline
        // starts 4 px from the top. Radio and checkbox captions start at the rect.
        let margin = if label_beside {
            egui::Margin::ZERO
        } else if multiline {
            egui::Margin::symmetric(4, 4)
        } else {
            egui::Margin::symmetric(4, 0)
        };

        let response = {
            let edit = self.inline.as_mut().unwrap();
            let mut widget = if multiline {
                egui::TextEdit::multiline(&mut edit.buffer).return_key(egui::KeyboardShortcut::new(
                    egui::Modifiers::SHIFT,
                    egui::Key::Enter,
                ))
            } else {
                egui::TextEdit::singleline(&mut edit.buffer).return_key(
                    egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Enter),
                )
            };
            widget = widget
                .id(edit_id)
                .font(font)
                .text_color(color)
                .frame(false)
                .margin(margin)
                .min_size(rect.size())
                .horizontal_align(horizontal)
                .vertical_align(if multiline {
                    egui::Align::Min
                } else {
                    egui::Align::Center
                })
                .desired_width(rect.width());
            if !hint.is_empty() {
                widget = widget.hint_text(hint);
            }
            ui.put(rect, widget)
        };
        if request_focus {
            response.request_focus();
            if let Some(edit) = &mut self.inline {
                edit.request_focus = false;
            }
        }
        if response.changed() {
            self.apply_inline();
        }
        let escape = ui.input(|input| input.key_pressed(Key::Escape));
        let enter = ui.input(|input| input.key_pressed(Key::Enter) && !input.modifiers.shift);
        if escape && (response.has_focus() || response.lost_focus()) {
            self.cancel_inline();
            ui.memory_mut(|memory| memory.surrender_focus(edit_id));
        } else if multiline && enter && response.has_focus() {
            self.finish_inline();
            ui.memory_mut(|memory| memory.surrender_focus(edit_id));
        } else if response.lost_focus() {
            self.finish_inline();
        }
    }

    fn field_context_menu(&mut self, response: &egui::Response) {
        let popup_id = response.id.with("popup");
        if response.secondary_clicked() && self.context_field.is_none() {
            egui::Popup::close_id(&response.ctx, popup_id);
            return;
        }
        let open = egui::Popup::is_id_open(&response.ctx, popup_id);
        if self.context_field.is_some() && (response.secondary_clicked() || open) {
            egui::Popup::context_menu(response).show(|ui| {
                ui.set_min_width(200.0);
                if ui.button("Properties").clicked() {
                    self.open_properties();
                    ui.close();
                }
                if self.selection.len() >= 2 {
                    ui.separator();
                    self.align_commands(ui, true);
                }
            });
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

        if let Some(Drag::Marquee { start, .. }) = &self.drag {
            let start = *start;
            if let Some(current) = self.pointer_pdf_from_painter(painter) {
                if !is_click(&self.view, start, current) {
                    let band = to_egui(self.view.pdf_rect_to_screen(marquee_rect(start, current)));
                    painter.rect_filled(
                        band,
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(
                            colors.selection.r(),
                            colors.selection.g(),
                            colors.selection.b(),
                            40,
                        ),
                    );
                    stroke_rect(painter, band, Stroke::new(1.0_f32, colors.selection));
                }
            }
        }

        if let Some(Drag::Place { start, kind }) = &self.drag {
            let start = *start;
            let kind = *kind;
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

        for id in &self.selection {
            let Some(field) = self.doc.field(*id) else {
                continue;
            };
            if field.line_ends().is_none() {
                let screen = to_egui(self.view.pdf_rect_to_screen(field.rect())).expand(3.0);
                stroke_rect(painter, screen, Stroke::new(1.5_f32, colors.selection));
            }
        }
        if let Some(id) = self.primary() {
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
            let safe = to_egui(self.view.pdf_rect_to_screen(self.doc.margin_rect()));
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
        let style = field.style();
        let zoom = self.view.zoom;
        let font = canvas_font(style, zoom);
        let light_fill = style
            .fill
            .is_some_and(|fill| fill.r + fill.g + fill.b > 1.5);
        let text_color = canvas_text_color(style.text_color, light_fill, colors);
        let screen = if field.line_ends().is_none() {
            Some(to_egui(self.view.pdf_rect_to_screen(field.rect())))
        } else {
            None
        };

        if let Some((start, end)) = field.line_ends() {
            let (x0, y0) = self.view.pdf_to_screen(start);
            let (x1, y1) = self.view.pdf_to_screen(end);
            let color = canvas_stroke_color(style.stroke_color, false, colors, false);
            let width = shape_stroke_px(style.stroke_width, zoom, style.stroke_color.is_black());
            let stroke = Stroke::new(width, color);
            if style.border_style == BorderStyleKind::Dashed {
                dash_line(painter, Pos2::new(x0, y0), Pos2::new(x1, y1), stroke);
            } else {
                painter.line_segment([Pos2::new(x0, y0), Pos2::new(x1, y1)], stroke);
            }
            return;
        }
        let screen = screen.expect("box field");
        let kind = field.kind();

        if kind == FieldKind::Radio {
            let center = screen.center();
            let radius = (screen.width().min(screen.height()) * 0.5 - 1.5).max(2.0);
            if let Some(fill) = style.fill {
                painter.circle_filled(center, radius, canvas_fill(fill, colors));
            }
            let width = (style.border_width * zoom).clamp(0.8, 8.0);
            painter.circle_stroke(
                center,
                radius,
                Stroke::new(
                    width,
                    canvas_stroke_color(style.border_color, light_fill, colors, true),
                ),
            );
            if style.checked() {
                painter.circle_filled(center, radius * 0.45, text_color);
            }
            if self.doc.radio_labels()
                && !field.caption().is_empty()
                && !self.inline_hides_text(field.id())
            {
                painter.text(
                    screen.right_center() + Vec2::new(6.0, 0.0),
                    Align2::LEFT_CENTER,
                    field.caption(),
                    font,
                    text_color,
                );
            }
            return;
        }

        if let Some(fill) = style.fill {
            if kind != FieldKind::StaticText {
                painter.rect_filled(screen, 0.0, canvas_fill(fill, colors));
            }
        }
        let outline_pt = if matches!(
            kind,
            FieldKind::Rectangle | FieldKind::Image | FieldKind::Table
        ) {
            style.stroke_width
        } else {
            style.border_width
        };
        let outline_color = if matches!(
            kind,
            FieldKind::Rectangle | FieldKind::Image | FieldKind::Table
        ) {
            canvas_stroke_color(style.stroke_color, light_fill, colors, false)
        } else {
            canvas_stroke_color(
                style.border_color,
                light_fill,
                colors,
                kind == FieldKind::Checkbox,
            )
        };
        if outline_pt > 0.05 {
            let stand_in = match kind {
                FieldKind::StaticText => style.border_color.is_black() && style.fill.is_none(),
                FieldKind::Rectangle | FieldKind::Image | FieldKind::Table => {
                    style.stroke_color.is_black() && !light_fill
                }
                _ => false,
            };
            paint_outline(
                painter,
                screen,
                shape_stroke_px(outline_pt, zoom, stand_in),
                outline_color,
                style.border_style,
            );
        }

        match kind {
            FieldKind::Text | FieldKind::Signature | FieldKind::Date | FieldKind::Dropdown
                if screen.height() > 10.0 =>
            {
                let owned;
                let label = match kind {
                    FieldKind::Text if !style.default_value.is_empty() => {
                        style.default_value.as_str()
                    }
                    FieldKind::Text => field.name(),
                    FieldKind::Signature => field.caption(),
                    FieldKind::Date if !style.default_value.is_empty() => {
                        style.default_value.as_str()
                    }
                    FieldKind::Date => style.date_format.as_str(),
                    FieldKind::Dropdown if !style.default_value.is_empty() => {
                        owned = style.default_value.clone();
                        owned.as_str()
                    }
                    FieldKind::Dropdown => {
                        field.options().first().map(String::as_str).unwrap_or("")
                    }
                    _ => "",
                };
                if !self.inline_hides_text(field.id()) {
                    paint_label(
                        painter,
                        screen,
                        label,
                        &font,
                        text_color,
                        style.align,
                        style.multiline && kind == FieldKind::Text,
                    );
                }
                if kind == FieldKind::Dropdown {
                    painter.text(
                        screen.right_center() - Vec2::new(8.0, 0.0),
                        Align2::RIGHT_CENTER,
                        "v",
                        font.clone(),
                        text_color,
                    );
                }
            }
            FieldKind::StaticText => {
                if !self.inline_hides_text(field.id()) {
                    paint_label(
                        painter,
                        screen,
                        field.caption(),
                        &font,
                        text_color,
                        style.align,
                        false,
                    );
                }
            }
            FieldKind::Table => {
                let stroke =
                    Stroke::new((style.stroke_width * zoom).clamp(0.6, 6.0), outline_color);
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
            }
            FieldKind::TextBox => {
                if !self.inline_hides_text(field.id()) {
                    paint_label(
                        painter,
                        screen,
                        field.caption(),
                        &font,
                        text_color,
                        style.align,
                        style.multiline,
                    );
                }
            }
            FieldKind::Image => {
                painter.line_segment(
                    [screen.left_top(), screen.right_bottom()],
                    Stroke::new(1.0_f32, outline_color),
                );
                painter.line_segment(
                    [screen.right_top(), screen.left_bottom()],
                    Stroke::new(1.0_f32, outline_color),
                );
                painter.text(
                    screen.center(),
                    Align2::CENTER_CENTER,
                    if field.caption().is_empty() {
                        "Image"
                    } else {
                        field.caption()
                    },
                    font.clone(),
                    text_color,
                );
            }
            FieldKind::Checkbox if style.checked() => {
                paint_check_mark(painter, screen, style.checkbox_style, text_color);
            }
            FieldKind::Checkbox | FieldKind::Rectangle | FieldKind::Line | FieldKind::Radio => {}
            _ => {}
        }
        if kind == FieldKind::Checkbox
            && !field.caption().is_empty()
            && !self.inline_hides_text(field.id())
        {
            painter.text(
                screen.right_center() + Vec2::new(6.0, 0.0),
                Align2::LEFT_CENTER,
                field.caption(),
                font.clone(),
                text_color,
            );
        }
    }

    fn paint_page_chrome(
        &self,
        painter: &egui::Painter,
        page: Page,
        colors: &crate::theme::CanvasColors,
    ) {
        let margins = self.doc.margins();
        let stroke = Stroke::new(1.0_f32, colors.margin);
        let font = FontId::proportional(13.0);
        if self.header_visible {
            let y = page.height - margins.top;
            let (x0, sy) = self.view.pdf_to_screen(PdfPoint { x: margins.left, y });
            let (x1, _) = self.view.pdf_to_screen(PdfPoint {
                x: page.width - margins.right,
                y,
            });
            dash_line(painter, Pos2::new(x0, sy), Pos2::new(x1, sy), stroke);
            let header = self.doc.header_text();
            if !header.is_empty() {
                let (_, label_y) = self.view.pdf_to_screen(PdfPoint {
                    x: page.width * 0.5,
                    y: page.height - margins.top * 0.45,
                });
                let (label_x, _) = self.view.pdf_to_screen(PdfPoint {
                    x: page.width * 0.5,
                    y: page.height,
                });
                painter.text(
                    Pos2::new(label_x, label_y),
                    Align2::CENTER_CENTER,
                    header,
                    font.clone(),
                    colors.margin,
                );
            }
        }
        if self.footer_visible {
            let y = margins.bottom;
            let (x0, sy) = self.view.pdf_to_screen(PdfPoint { x: margins.left, y });
            let (x1, _) = self.view.pdf_to_screen(PdfPoint {
                x: page.width - margins.right,
                y,
            });
            dash_line(painter, Pos2::new(x0, sy), Pos2::new(x1, sy), stroke);
            let footer = self.doc.footer_text();
            if !footer.is_empty() {
                let (label_x, label_y) = self.view.pdf_to_screen(PdfPoint {
                    x: margins.left + 8.0,
                    y: margins.bottom * 0.45,
                });
                painter.text(
                    Pos2::new(label_x, label_y),
                    Align2::LEFT_CENTER,
                    footer,
                    font.clone(),
                    colors.margin,
                );
            }
        }
        if self.page_numbers_visible {
            let (x, y) = self.view.pdf_to_screen(PdfPoint {
                x: page.width * 0.5,
                y: margins.bottom * 0.45,
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
            if let Some(id) = self.primary() {
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
        write_preferences(self, storage);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.handle_shortcuts(ctx);
        let now = ctx.input(|input| input.time);
        if let Some(remaining) = self.fade_status(now) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(remaining));
        }
        let title = self.window_title();
        if title != self.last_title {
            self.last_title = title.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        if ctx.input(|input| input.viewport().close_requested())
            && !self.allow_close
            && self.doc.is_dirty()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.unsaved.is_none() {
                self.unsaved = Some(PendingFileAction::Exit);
            }
        }
        crate::theme::apply(ctx, self.dark);
        if self.persist_theme || self.persist_panel {
            if let Some(storage) = frame.storage_mut() {
                write_preferences(self, storage);
                storage.flush();
            }
            self.persist_theme = false;
            self.persist_panel = false;
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
        if self.properties_open {
            egui::SidePanel::right("properties")
                .resizable(true)
                .default_width(300.0)
                .width_range(260.0..=460.0)
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    self.properties_panel(ui);
                    ui.add_space(8.0);
                });
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            self.canvas(ui);
        });
        self.preferences_dialog(ctx);
        self.about_dialog(ctx);
        self.help_window(ctx);
        self.export_dialog(ctx);
        self.unsaved_dialog(ctx);
        if self.saved_follow_up {
            self.saved_follow_up = false;
            if let Some(action) = self.after_save.take() {
                self.run_file_action(action);
            }
        }
        if self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn write_preferences(app: &PageFormApp, storage: &mut dyn eframe::Storage) {
    storage.set_string(
        crate::theme::STORAGE_KEY,
        crate::theme::storage_value(app.dark).to_owned(),
    );
    storage.set_string(
        crate::prefs::OPEN_KEY,
        crate::prefs::open_value(app.properties_open).to_owned(),
    );
    storage.set_string(
        crate::prefs::PIN_KEY,
        crate::prefs::pin_value(app.properties_pinned).to_owned(),
    );
    storage.set_string(
        crate::prefs::RECENT_KEY,
        crate::prefs::format_recent(&app.recent),
    );
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

const MENU_BAR: &[&str] = &["File", "Edit", "Insert", "View", "Help"];

/// Text placed on the OS clipboard so the next Ctrl/Cmd+V emits `Event::Paste`.
const ITEM_CLIPBOARD_MARKER: &str = "PageForm items";

const FILE_MENU: &[&str] = &[
    "New",
    "Open",
    "",
    "Save",
    "Save As",
    "",
    "Print",
    "Export PDF",
    "",
    "Exit",
];

const NEW_MENU: &[&str] = &["New Blank Document", "New from Template"];

const EDIT_MENU: &[&str] = &[
    "Select",
    "Select All",
    "",
    "Copy",
    "Cut",
    "Paste",
    "Delete",
    "",
    "Align",
    "",
    "Preferences",
];

const ALIGN_EDGES: &[(&str, AlignEdge)] = &[
    ("Align left", AlignEdge::Left),
    ("Align center", AlignEdge::Center),
    ("Align right", AlignEdge::Right),
    ("Align top", AlignEdge::Top),
    ("Align middle", AlignEdge::Middle),
    ("Align bottom", AlignEdge::Bottom),
];

const HELP_MENU: &[&str] = &["Help", "About"];

const ABOUT_TAGLINE: &str = "Open-Source Native Form and Document Editor Written in Rust";

const HELP_FAQ: &[(&str, &str)] = &[
    (
        "What is PageForm?",
        "PageForm is an open-source native form and document editor written in Rust. This version edits one US Letter page.",
    ),
    (
        "How do I place something?",
        "The Insert menu drops a default-sized item at the center of the visible page. A toolbar icon arms that tool: click the page for the default size, or drag to size it. Square/Rectangle is the Shapes icon.",
    ),
    (
        "How do I select, move, and delete?",
        "Edit > Select, or the Select tool. Drag on empty canvas to draw a box around the items you want. Drag a selected item to move it, and drag a handle to resize it. Edit > Delete, or the Delete key, removes the selection. Edit > Align, or a right-click, lines up two or more selected items.",
    ),
    (
        "Where are theme and radio labels?",
        "Edit > Preferences. Dark is the default, and the choice is saved. Radio labels shows or hides the text beside each radio button. The toolbar theme button switches Dark and Light as well. Preferences also stores whether the properties panel opens and whether it is pinned.",
    ),
    (
        "How do I open properties?",
        "The properties panel starts hidden. Open it from View > Properties, the toolbar properties icon, or a right-click Properties command. The header has a close button and a pin. Pinned panels stay open. Unpinned panels close when you click empty canvas. Shift-click or Ctrl-click selects more than one item.",
    ),
    (
        "How do I edit text on the page?",
        "Double-click static text, a text box, a radio or checkbox label, a signature word, or the default text in a text, date, or drop-down field. Type in the box. Enter, or a click outside, finishes. Esc restores the previous text. On a multi-line item, Shift+Enter starts a new line and Enter finishes. The sidebar text field shows the same words while you type.",
    ),
    (
        "What do New, Open, Save, Save As, and Print do?",
        "File > New Blank Document starts a blank page and asks before discarding unsaved work. New from Template opens a .pgt template as a new untitled document and does not change the template. Open reads .pgd documents, .pgt templates, and older .pageform files. Save uses Ctrl or Cmd+S and writes the current .pgd. A new document, including one opened from a template, has no linked file, so Save asks for a new .pgd name and cannot overwrite the template. Save As uses Ctrl or Cmd+Shift+S. A template file is written only when Template (.pgt) is chosen, and an existing template asks before it is replaced. Saving a template does not link that file, so the next Save still asks for a .pgd. Open and Save use the system file dialog, or PageForm's own path window when that dialog is unavailable. Open Recent lists the last .pgd and .pgt files. The window title shows the file name and a * when there are unsaved changes. Exit asks before closing a changed file. Print uses Ctrl or Cmd+P, writes a temporary PDF, and hands it to the system. Export PDF still writes a fillable PDF. Guides, margins, the header, the footer, and the page number stay on the canvas.",
    ),
    (
        "Is there a list box?",
        "No. Drop-down is a combo box.",
    ),
    (
        "What is still a placeholder?",
        "Table is a 3 by 2 grid and the cells are not editable. Image is a crossed box until a file can be chosen. Signature is a real unsigned field; the app does not capture ink.",
    ),
];

enum HelpHost {
    Embedded,
    Native { close: bool },
}

fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
fn menu_entries<'a>(labels: &'a [&'a str]) -> Vec<&'a str> {
    labels
        .iter()
        .copied()
        .filter(|label| !label.is_empty())
        .collect()
}

fn sorted_insert_kinds() -> [FieldKind; 12] {
    let mut kinds = FieldKind::INSERT;
    kinds.sort_by(|left, right| left.label().cmp(right.label()));
    kinds
}

fn placeholder_icon(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(72.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, 10.0, egui::Color32::from_rgb(32, 36, 44));
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        "E",
        FontId::proportional(40.0),
        egui::Color32::WHITE,
    );
}

fn parent_dir(path: &str) -> String {
    std::path::Path::new(path)
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.display().to_string())
        .filter(|dir| !dir.is_empty())
        .unwrap_or_else(default_export_dir)
}

fn ensure_extension(mut path: std::path::PathBuf, ext: &str) -> std::path::PathBuf {
    if path.extension().is_none() {
        path.set_extension(ext);
    }
    path
}

const OPEN_FILTERS: &[(&str, &[&str])] = &[("PageForm", &["pgd", "pgt", "pageform"])];

const SAVE_FILTERS: &[(&str, &[&str])] = &[
    ("PageForm Document", &["pgd"]),
    ("PageForm Template", &["pgt"]),
];

const TEMPLATE_FILTERS: &[(&str, &[&str])] = &[("PageForm Template", &["pgt"])];

fn is_open_prompt(kind: PromptKind) -> bool {
    matches!(kind, PromptKind::OpenProject | PromptKind::OpenTemplate)
}

fn extension_is(path: &std::path::Path, ext: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(ext))
}

fn is_template_path(path: &std::path::Path) -> bool {
    extension_is(path, "pgt")
}

fn retarget_project_name(file_name: &str, template: bool) -> String {
    let trimmed = file_name.trim();
    if trimmed.is_empty() {
        return if template {
            "Untitled.pgt".to_string()
        } else {
            "Untitled.pgd".to_string()
        };
    }
    let ext = if template { "pgt" } else { "pgd" };
    std::path::PathBuf::from(trimmed)
        .with_extension(ext)
        .display()
        .to_string()
}

fn suggested_project_name(project_path: Option<&str>, template: bool) -> String {
    match project_path {
        Some(path) => retarget_project_name(&export_file_name(path), template),
        None => retarget_project_name("Untitled.pgd", template),
    }
}

fn project_save_path(directory: &str, file_name: &str, template: bool) -> std::path::PathBuf {
    let name = retarget_project_name(file_name, template);
    let mut path = std::path::PathBuf::from(directory.trim());
    if path.as_os_str().is_empty() {
        path = std::path::PathBuf::from(default_export_dir());
    }
    path.push(name);
    path
}

/// A missing extension becomes `.pgd`. `.pgt`, `.pgd`, and legacy `.pageform` stay.
fn native_project_save_path(mut path: std::path::PathBuf) -> std::path::PathBuf {
    if path.extension().is_none() {
        path.set_extension("pgd");
    }
    path
}

fn pick_native(
    save: bool,
    title: &str,
    filters: &[(&str, &[&str])],
    file_name: &str,
) -> NativePick {
    if !native_save_dialog_available() {
        return NativePick::Fallback;
    }
    let started = std::time::Instant::now();
    let mut dialog = rfd::FileDialog::new().set_title(title);
    for (name, extensions) in filters {
        dialog = dialog.add_filter(*name, extensions);
    }
    let dialog = if file_name.is_empty() {
        dialog
    } else {
        dialog.set_file_name(file_name)
    };
    let chosen = if save {
        dialog.save_file()
    } else {
        dialog.pick_file()
    };
    match chosen {
        Some(path) => NativePick::Path(path),
        None if started.elapsed() < std::time::Duration::from_millis(500) => NativePick::Fallback,
        None => NativePick::Cancel,
    }
}

#[cfg(windows)]
fn send_to_printer(path: &std::path::Path) -> String {
    send_to_printer_windows(path)
}

#[cfg(not(windows))]
fn send_to_printer(path: &std::path::Path) -> String {
    send_to_printer_unix(path)
}

#[cfg(windows)]
fn send_to_printer_windows(path: &std::path::Path) -> String {
    use std::os::windows::ffi::OsStrExt;
    let operation: Vec<u16> = std::ffi::OsStr::new("print")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let file: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let code = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if (code as isize) <= 32 {
        format!(
            "Windows could not print {} (ShellExecute returned {}). Export a PDF and print it from a viewer.",
            path.display(),
            code as isize
        )
    } else {
        format!("Sent {} to the printer.", path.display())
    }
}

#[cfg(windows)]
extern "system" {
    fn ShellExecuteW(
        hwnd: *mut std::ffi::c_void,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> *mut std::ffi::c_void;
}

#[cfg(not(windows))]
fn send_to_printer_unix(path: &std::path::Path) -> String {
    if unix_command("lp") {
        match std::process::Command::new("lp").arg(path).status() {
            Ok(status) if status.success() => format!("Sent {} to the printer.", path.display()),
            Ok(status) => format!(
                "lp could not print {} ({status}). Export a PDF and print it from a viewer.",
                path.display()
            ),
            Err(err) => format!("Could not run lp: {err}"),
        }
    } else if unix_command("xdg-open") {
        match std::process::Command::new("xdg-open").arg(path).spawn() {
            Ok(_) => format!("Opened {} so you can print it.", path.display()),
            Err(err) => format!("Could not open the PDF: {err}"),
        }
    } else {
        format!(
            "No print command found. Install lp or xdg-open, or export {} and print it from a viewer.",
            path.display()
        )
    }
}

#[cfg(not(windows))]
fn unix_command(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

/// `(ignore this primary click, keep suppressing while the button is held)`.
fn ignore_place_click(suppress: bool, primary_down: bool) -> (bool, bool) {
    if suppress {
        (true, primary_down)
    } else {
        (false, false)
    }
}

fn command_shortcut(ui: &egui::Ui, key: egui::Key) -> String {
    ui.ctx()
        .format_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, key))
}

fn marquee_rect(start: PdfPoint, current: PdfPoint) -> RectPt {
    let left = start.x.min(current.x);
    let right = start.x.max(current.x);
    let bottom = start.y.min(current.y);
    let top = start.y.max(current.y);
    RectPt {
        x: left,
        y: bottom,
        w: (right - left).max(0.0),
        h: (top - bottom).max(0.0),
    }
}

fn rects_hit(a: RectPt, b: RectPt) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.top() && b.y < a.top()
}

fn export_file_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("form.pdf")
        .to_string()
}

fn default_export_dir() -> String {
    std::env::current_dir()
        .map(|dir| dir.display().to_string())
        .unwrap_or_else(|_| ".".to_string())
}

fn export_file_path(directory: &str, file_name: &str) -> std::path::PathBuf {
    let mut name = file_name.trim().to_string();
    if name.is_empty() {
        name = "form.pdf".to_string();
    }
    let mut path = std::path::PathBuf::from(directory.trim());
    if path.as_os_str().is_empty() {
        path = std::path::PathBuf::from(default_export_dir());
    }
    path.push(name);
    if path.extension().is_none() {
        path.set_extension("pdf");
    }
    path
}

#[cfg(target_os = "linux")]
fn command_on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

/// rfd's Linux backend needs a desktop portal, zenity, or kdialog. Windows and
/// macOS use the system dialog.
fn native_save_dialog_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        if command_on_path("zenity") || command_on_path("kdialog") {
            return true;
        }
        let portal = [
            "/usr/libexec/xdg-desktop-portal",
            "/usr/lib/xdg-desktop-portal",
            "/usr/bin/xdg-desktop-portal",
        ]
        .iter()
        .any(|path| std::path::Path::new(path).is_file())
            || command_on_path("xdg-desktop-portal");
        portal && std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

fn help_faq(ui: &mut egui::Ui, dark: bool) {
    let text = if dark {
        egui::Color32::from_rgb(248, 248, 246)
    } else {
        egui::Color32::from_rgb(16, 18, 22)
    };
    ui.style_mut().visuals.override_text_color = Some(text);
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::proportional(26.0));
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(18.0));
    ui.heading("PageForm Help");
    ui.label(egui::RichText::new("Frequently asked questions").size(18.0));
    ui.separator();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (question, answer) in HELP_FAQ {
            ui.label(egui::RichText::new(*question).size(20.0).strong());
            ui.label(egui::RichText::new(*answer).size(18.0));
            ui.add_space(12.0);
        }
    });
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

fn square_from_drag(
    start: PdfPoint,
    end: PdfPoint,
    grid: Option<f32>,
    min: f32,
    page: &pageform_core::Page,
) -> pageform_core::RectPt {
    let start = snap_point(start, grid);
    let end = snap_point(end, grid);
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let side = dx.abs().max(dy.abs()).max(min.max(1.0));
    let ex = start.x + if dx < 0.0 { -side } else { side };
    let ey = start.y + if dy < 0.0 { -side } else { side };
    rect_from_drag(start, PdfPoint { x: ex, y: ey }, None, min, page)
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

fn menu_command_enabled(
    ui: &mut egui::Ui,
    label: &str,
    shortcut: Option<&str>,
    enabled: bool,
) -> bool {
    let mut button = egui::Button::new(label);
    if let Some(shortcut) = shortcut {
        button = button.shortcut_text(shortcut);
    }
    let clicked = ui.add_enabled(enabled, button).clicked();
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
        FieldKind::Line => {
            "Drop a horizontal line. The toolbar tool places that line on a click, or sets the angle when you drag."
        }
        FieldKind::Rectangle => {
            "Drop a rectangle. The toolbar tool places the default size on a click, or sizes it when you drag."
        }
        FieldKind::TextBox => "Drop a static text box. It is not a fillable field.",
        FieldKind::Image => "Drop an image placeholder. Choosing a file is not available yet.",
    }
}

fn icon_for_kind(kind: FieldKind) -> icons::Icon {
    match kind {
        FieldKind::Text => icons::Icon::TextField,
        FieldKind::StaticText => icons::Icon::Text,
        FieldKind::Checkbox => icons::Icon::Checkbox,
        FieldKind::Radio => icons::Icon::Radio,
        FieldKind::Table => icons::Icon::Table,
        FieldKind::Signature => icons::Icon::Signature,
        FieldKind::Date => icons::Icon::Date,
        FieldKind::Dropdown => icons::Icon::Dropdown,
        FieldKind::Line => icons::Icon::Line,
        FieldKind::Rectangle => icons::Icon::Shapes,
        FieldKind::TextBox => icons::Icon::TextBox,
        FieldKind::Image => icons::Icon::Image,
    }
}

fn kind_for_toolbar_label(label: &str) -> Option<FieldKind> {
    FieldKind::INSERT
        .into_iter()
        .find(|kind| kind.label() == label)
}

fn toolbar_tip(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Text => "Click to place a default text field, or drag to size it.",
        FieldKind::StaticText => {
            "Click to place a default text label, or drag to size it. Not a fillable field."
        }
        FieldKind::Checkbox => "Click to place a default checkbox, or drag to size it.",
        FieldKind::Radio => {
            "Click to place a default radio button, or drag to size it. A selected radio joins that group."
        }
        FieldKind::Table => {
            "Click to place a default table, or drag to size it. Cells are not editable yet."
        }
        FieldKind::Signature => "Click to place a default signature field, or drag to size it. No ink capture yet.",
        FieldKind::Date => "Click to place a default date field, or drag to size it.",
        FieldKind::Dropdown => "Click to place a default drop-down, or drag to size it. Not a list box.",
        FieldKind::Line => "Click to place a default line, or drag to set its length and angle.",
        FieldKind::Rectangle => "Click to place a default rectangle, or drag to size it.",
        FieldKind::TextBox => "Click to place a default text box, or drag to size it. Not a fillable field.",
        FieldKind::Image => "Click to place a default image box, or drag to size it. Choosing a file is not available yet.",
    }
}

fn kind_note(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Text => "One-line fillable text field.",
        FieldKind::StaticText => "Static label. Not a fillable field.",
        FieldKind::Checkbox => "Fillable checkbox.",
        FieldKind::Radio => "One button in a radio group. The first two labels are Yes and No.",
        FieldKind::Table => "Table placeholder. A 3x2 grid. Cells are not editable yet.",
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

fn inline_target(kind: FieldKind, multiline: bool) -> Option<(InlineTarget, bool)> {
    match kind {
        FieldKind::StaticText | FieldKind::Radio | FieldKind::Checkbox | FieldKind::Signature => {
            Some((InlineTarget::Caption, false))
        }
        FieldKind::TextBox => Some((InlineTarget::Caption, multiline)),
        FieldKind::Text => Some((InlineTarget::DefaultValue, multiline)),
        FieldKind::Date | FieldKind::Dropdown => Some((InlineTarget::DefaultValue, false)),
        FieldKind::Line | FieldKind::Rectangle | FieldKind::Image | FieldKind::Table => None,
    }
}

fn adjacent_label_rect(rect: RectPt) -> RectPt {
    RectPt {
        x: rect.right() + 4.0,
        y: rect.y,
        w: 140.0,
        h: rect.h.max(12.0),
    }
}

fn inline_screen_rect(
    view: &ViewTransform,
    rect: RectPt,
    kind: FieldKind,
    _font_size: f32,
    _multiline: bool,
) -> Rect {
    if matches!(kind, FieldKind::Radio | FieldKind::Checkbox) {
        // Same origin as the caption painted at the control's right center + 6 px.
        let control = to_egui(view.pdf_rect_to_screen(rect));
        return Rect::from_min_size(
            Pos2::new(control.right() + 6.0, control.top()),
            Vec2::new((140.0 * view.zoom).max(80.0), control.height().max(12.0)),
        );
    }
    // The whole item, so the editor covers the painted text. Vertical alignment
    // inside the widget matches paint_label.
    to_egui(view.pdf_rect_to_screen(rect))
}

fn inline_hint(kind: FieldKind, style: &pageform_core::FieldStyle, name: &str) -> String {
    match kind {
        FieldKind::Text if style.default_value.is_empty() => name.to_string(),
        FieldKind::Date if style.default_value.is_empty() => style.date_format.clone(),
        _ => String::new(),
    }
}

fn selection_after_click(current: &[FieldId], hit: FieldId, extend: bool) -> Vec<FieldId> {
    if extend {
        let mut next = current.to_vec();
        if let Some(index) = next.iter().position(|id| *id == hit) {
            next.remove(index);
        } else {
            next.push(hit);
        }
        next
    } else if current.len() > 1 && current.contains(&hit) {
        current.to_vec()
    } else {
        vec![hit]
    }
}

fn rgb32(color: Rgb) -> egui::Color32 {
    egui::Color32::from_rgb(
        (color.r * 255.0).round().clamp(0.0, 255.0) as u8,
        (color.g * 255.0).round().clamp(0.0, 255.0) as u8,
        (color.b * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn shape_stroke_px(width_pt: f32, zoom: f32, stand_in: bool) -> f32 {
    let px = (width_pt * zoom).clamp(0.8, 14.0);
    if stand_in {
        px.max(2.0)
    } else {
        px
    }
}

fn is_click(view: &ViewTransform, start: PdfPoint, current: PdfPoint) -> bool {
    let (x0, y0) = view.pdf_to_screen(start);
    let (x1, y1) = view.pdf_to_screen(current);
    let dx = x0 - x1;
    let dy = y0 - y1;
    dx * dx + dy * dy < 12.0 * 12.0
}

fn canvas_font(style: &pageform_core::FieldStyle, zoom: f32) -> FontId {
    let size = (style.font_size * zoom).clamp(7.0, 48.0);
    match style.font {
        FontFamily::Courier => FontId::monospace(size),
        FontFamily::Helvetica | FontFamily::Times => FontId::proportional(size),
    }
}

/// Black strokes on the dark page use the theme color so they stay visible.
/// Any other color is drawn as stored. The PDF keeps the real value.
fn canvas_stroke_color(
    color: Rgb,
    light_fill: bool,
    colors: &crate::theme::CanvasColors,
    button: bool,
) -> egui::Color32 {
    if color.is_black() {
        if !light_fill {
            colors.shape_stroke
        } else if button {
            colors.check_stroke
        } else {
            colors.text_stroke
        }
    } else {
        rgb32(color)
    }
}

fn canvas_fill(color: Rgb, colors: &crate::theme::CanvasColors) -> egui::Color32 {
    if color.r >= 0.999 && color.g >= 0.999 && color.b >= 0.999 {
        colors.field_fill
    } else {
        rgb32(color)
    }
}

fn canvas_text_color(
    color: Rgb,
    light_fill: bool,
    colors: &crate::theme::CanvasColors,
) -> egui::Color32 {
    if color.is_black() {
        if light_fill {
            colors.field_text
        } else {
            colors.label_text
        }
    } else {
        rgb32(color)
    }
}

fn paint_outline(
    painter: &egui::Painter,
    rect: Rect,
    width: f32,
    color: egui::Color32,
    style: BorderStyleKind,
) {
    let stroke = Stroke::new(width, color);
    match style {
        BorderStyleKind::Dashed => dash_rect(painter, rect, stroke),
        BorderStyleKind::Underline => {
            painter.line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
        }
        BorderStyleKind::Beveled | BorderStyleKind::Inset => {
            stroke_rect(painter, rect, stroke);
            let light = egui::Color32::from_white_alpha(if style == BorderStyleKind::Beveled {
                180
            } else {
                40
            });
            painter.line_segment(
                [rect.left_top(), rect.right_top()],
                Stroke::new(width, light),
            );
            painter.line_segment(
                [rect.left_top(), rect.left_bottom()],
                Stroke::new(width, light),
            );
        }
        BorderStyleKind::Solid => stroke_rect(painter, rect, stroke),
    }
}

fn paint_label(
    painter: &egui::Painter,
    screen: Rect,
    text: &str,
    font: &FontId,
    color: egui::Color32,
    align: TextAlign,
    multiline: bool,
) {
    if text.is_empty() || screen.width() < 4.0 {
        return;
    }
    let (anchor, pos) = match align {
        TextAlign::Left => (
            Align2::LEFT_CENTER,
            screen.left_center() + Vec2::new(4.0, 0.0),
        ),
        TextAlign::Center => (Align2::CENTER_CENTER, screen.center()),
        TextAlign::Right => (
            Align2::RIGHT_CENTER,
            screen.right_center() - Vec2::new(4.0, 0.0),
        ),
    };
    if multiline && screen.height() > 16.0 {
        let mut job = egui::text::LayoutJob::default();
        job.append(text, 0.0, egui::TextFormat::simple(font.clone(), color));
        job.wrap.max_width = (screen.width() - 8.0).max(8.0);
        job.halign = match align {
            TextAlign::Left => egui::Align::LEFT,
            TextAlign::Center => egui::Align::Center,
            TextAlign::Right => egui::Align::RIGHT,
        };
        let galley = painter.layout_job(job);
        painter.galley(screen.left_top() + Vec2::new(4.0, 4.0), galley, color);
    } else {
        painter.text(pos, anchor, text, font.clone(), color);
    }
}

fn paint_check_mark(
    painter: &egui::Painter,
    screen: Rect,
    style: CheckboxStyle,
    color: egui::Color32,
) {
    let stroke = Stroke::new(1.6_f32, color);
    let center = screen.center();
    match style {
        CheckboxStyle::Check => {
            painter.line_segment(
                [
                    screen.left_center() + Vec2::new(screen.width() * 0.22, screen.height() * 0.05),
                    screen.center() + Vec2::new(-screen.width() * 0.05, screen.height() * 0.22),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    screen.center() + Vec2::new(-screen.width() * 0.05, screen.height() * 0.22),
                    screen.right_center()
                        + Vec2::new(-screen.width() * 0.18, -screen.height() * 0.22),
                ],
                stroke,
            );
        }
        CheckboxStyle::Cross => {
            painter.line_segment(
                [
                    screen.left_top() + Vec2::splat(4.0),
                    screen.right_bottom() - Vec2::splat(4.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    screen.right_top() + Vec2::new(-4.0, 4.0),
                    screen.left_bottom() + Vec2::new(4.0, -4.0),
                ],
                stroke,
            );
        }
        CheckboxStyle::Circle => {
            painter.circle_filled(center, screen.width().min(screen.height()) * 0.22, color);
        }
        CheckboxStyle::Diamond => {
            let r = screen.width().min(screen.height()) * 0.28;
            painter.add(egui::Shape::closed_line(
                vec![
                    center + Vec2::new(0.0, -r),
                    center + Vec2::new(r, 0.0),
                    center + Vec2::new(0.0, r),
                    center + Vec2::new(-r, 0.0),
                ],
                stroke,
            ));
        }
        CheckboxStyle::Square => {
            let side = screen.width().min(screen.height()) * 0.45;
            let rect = Rect::from_center_size(center, Vec2::splat(side));
            stroke_rect(painter, rect, stroke);
        }
        CheckboxStyle::Star => {
            let r = screen.width().min(screen.height()) * 0.28;
            painter.line_segment(
                [center - Vec2::new(0.0, r), center + Vec2::new(0.0, r)],
                stroke,
            );
            painter.line_segment(
                [center - Vec2::new(r, 0.0), center + Vec2::new(r, 0.0)],
                stroke,
            );
            painter.line_segment(
                [
                    center - Vec2::new(r * 0.7, r * 0.7),
                    center + Vec2::new(r * 0.7, r * 0.7),
                ],
                stroke,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::Storage;

    #[derive(Default)]
    struct MemStorage {
        values: std::collections::HashMap<String, String>,
    }

    impl eframe::Storage for MemStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.values.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.values.insert(key.to_string(), value);
        }

        fn flush(&mut self) {}
    }

    #[test]
    fn menu_bar_order_is_file_edit_insert_view_help() {
        assert_eq!(
            MENU_BAR,
            ["File", "Edit", "Insert", "View", "Help"].as_slice()
        );
        assert_eq!(
            menu_entries(FILE_MENU),
            [
                "New",
                "Open",
                "Save",
                "Save As",
                "Print",
                "Export PDF",
                "Exit"
            ]
        );
        assert_eq!(
            menu_entries(EDIT_MENU),
            [
                "Select",
                "Select All",
                "Copy",
                "Cut",
                "Paste",
                "Delete",
                "Align",
                "Preferences"
            ]
        );
        assert_eq!(menu_entries(HELP_MENU), ["Help", "About"]);
        assert_eq!(NEW_MENU, ["New Blank Document", "New from Template"]);
        assert_eq!(
            ABOUT_TAGLINE,
            "Open-Source Native Form and Document Editor Written in Rust"
        );
        assert!(!app_version().is_empty());
        assert!(HELP_FAQ.len() >= 5);
        for (question, answer) in HELP_FAQ {
            assert!(!question.contains('→'), "{question}");
            assert!(!answer.contains('→'), "{answer}");
            assert!(!answer.contains('×'), "{answer}");
        }
        for kind in FieldKind::INSERT {
            let tip = toolbar_tip(kind);
            assert!(tip.contains("Click"), "{tip}");
            assert!(tip.contains("drag"), "{tip}");
        }
    }

    #[test]
    fn insert_menu_is_alphabetical_and_the_toolbar_order_stays() {
        let menu: Vec<_> = sorted_insert_kinds()
            .into_iter()
            .map(FieldKind::label)
            .collect();
        let mut expected = menu.clone();
        expected.sort();
        assert_eq!(menu, expected);
        assert_eq!(
            menu,
            [
                "Checkbox",
                "Date",
                "Drop-down",
                "Image",
                "Line",
                "Radio",
                "Signature",
                "Square/Rectangle",
                "Table",
                "Text",
                "Text Box",
                "Text Field",
            ]
        );
        assert_eq!(icons::INSERT_TOOLS.len(), 12);
        assert!(icons::INSERT_TOOLS.contains(&"Shapes"));
        assert!(!icons::INSERT_TOOLS.contains(&"Square/Rectangle"));
        assert!(!icons::VIEW_TOGGLES.contains(&"Header"));
        assert!(!icons::VIEW_TOGGLES.contains(&"Footer"));
        assert_ne!(menu, icons::INSERT_TOOLS);
        assert_eq!(
            kind_for_toolbar_label("Checkbox"),
            Some(FieldKind::Checkbox)
        );
        assert_eq!(kind_for_toolbar_label("Shapes"), None);
        assert!(matches!(
            icon_for_kind(FieldKind::Rectangle),
            icons::Icon::Shapes
        ));
        assert!(matches!(
            icon_for_kind(FieldKind::Text),
            icons::Icon::TextField
        ));
        assert!(matches!(
            icon_for_kind(FieldKind::StaticText),
            icons::Icon::Text
        ));
    }

    #[test]
    fn toolbar_does_not_paint_removed_commands() {
        let ctx = egui::Context::default();
        let mut app = PageFormApp::new(false, None);
        let output = panel(&ctx, &mut app, |app, ui| app.toolbar(ui));
        let joined = galley_texts(&output).join("\n");
        for banned in ["Export PDF", "Header", "Footer", "Fit", "Square/Rectangle"] {
            assert!(
                !joined.contains(banned),
                "{banned} showed up on the toolbar: {joined}"
            );
        }
    }

    #[test]
    fn marquee_selects_items_in_the_drag_box_and_a_click_clears() {
        let mut app = PageFormApp::new(false, None);
        app.view.zoom = 1.0;
        let text = app.doc.add_field(
            FieldKind::Text,
            RectPt {
                x: 72.0,
                y: 700.0,
                w: 40.0,
                h: 18.0,
            },
        );
        let check = app.doc.add_field(
            FieldKind::Checkbox,
            RectPt {
                x: 200.0,
                y: 700.0,
                w: 18.0,
                h: 18.0,
            },
        );
        app.finish_marquee(
            PdfPoint { x: 60.0, y: 730.0 },
            PdfPoint { x: 230.0, y: 690.0 },
            false,
        );
        assert!(app.selection.contains(&text));
        assert!(app.selection.contains(&check));
        app.finish_marquee(
            PdfPoint { x: 10.0, y: 10.0 },
            PdfPoint { x: 12.0, y: 11.0 },
            false,
        );
        assert!(app.selection.is_empty());
    }

    #[test]
    fn status_fades_after_four_seconds_and_stays_while_editing() {
        let mut app = PageFormApp::new(false, None);
        app.status = "Placed Text".to_string();
        let remaining = app.fade_status(0.0).expect("timer");
        assert!((remaining - 4.0).abs() < 0.001);
        assert_eq!(app.status, "Placed Text");
        let remaining = app.fade_status(3.9).expect("timer");
        assert!((remaining - 0.1).abs() < 0.001);
        assert_eq!(app.status, "Placed Text");
        assert_eq!(app.fade_status(4.0), None);
        assert!(app.status.is_empty());

        app.insert_at_view(FieldKind::StaticText);
        let id = app.primary().unwrap();
        assert!(app.begin_inline(id));
        assert_eq!(app.fade_status(0.0), None);
        assert_eq!(app.fade_status(9.0), None);
        assert!(app.status.starts_with("Editing text."));
    }

    #[test]
    fn clipboard_events_and_select_all_run_on_the_first_pass() {
        let ctx = egui::Context::default();
        let mut app = PageFormApp::new(false, None);
        app.insert_at_view(FieldKind::Text);
        let placed = app.selection.clone();
        assert!(app.suppress_place_click);
        assert_eq!(ignore_place_click(true, false), (true, false));
        assert_eq!(ignore_place_click(true, true), (true, true));

        let mut input = screen_input();
        input.events.push(egui::Event::Copy);
        let _ = ctx.run(input, |ctx| app.handle_shortcuts(ctx));
        assert_eq!(app.clipboard.len(), 1);

        let mut input = screen_input();
        input.events.push(egui::Event::Cut);
        let _ = ctx.run(input, |ctx| app.handle_shortcuts(ctx));
        assert!(app.doc.fields().is_empty());

        let mut input = screen_input();
        input
            .events
            .push(egui::Event::Paste("not item data".into()));
        let _ = ctx.run(input, |ctx| app.handle_shortcuts(ctx));
        assert_eq!(app.doc.fields().len(), 1);
        assert_ne!(app.selection, placed);

        let count = app.doc.fields().len();
        let mut input = screen_input();
        input.events.push(egui::Event::Key {
            key: egui::Key::V,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        });
        let _ = ctx.run(input, |ctx| app.handle_shortcuts(ctx));
        assert_eq!(
            app.doc.fields().len(),
            count + 1,
            "Ctrl+V pastes without a Paste event"
        );

        let count = app.doc.fields().len();
        let mut input = screen_input();
        input.events.push(egui::Event::Key {
            key: egui::Key::V,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        });
        input
            .events
            .push(egui::Event::Paste(ITEM_CLIPBOARD_MARKER.into()));
        let _ = ctx.run(input, |ctx| app.handle_shortcuts(ctx));
        assert_eq!(
            app.doc.fields().len(),
            count + 1,
            "a paste event and Ctrl+V in one frame paste once"
        );

        app.doc.add_field(
            FieldKind::Checkbox,
            RectPt {
                x: 72.0,
                y: 600.0,
                w: 18.0,
                h: 18.0,
            },
        );
        let expected = app.doc.fields().len();
        app.selection.clear();
        let mut input = screen_input();
        input.events.push(egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        });
        let _ = ctx.run(input, |ctx| app.handle_shortcuts(ctx));
        assert_eq!(app.selection.len(), expected);
    }

    #[test]
    fn select_all_copy_cut_and_paste_round_trip() {
        let mut app = PageFormApp::new(false, None);
        app.insert_at_view(FieldKind::Text);
        let original = app.doc.fields()[0].rect();
        app.copy_selection();
        app.paste_clipboard();
        assert_eq!(app.doc.fields().len(), 2);
        let pasted = app.doc.field(app.primary().unwrap()).unwrap().rect();
        assert!((pasted.x - original.x).abs() > 1.0);
        app.select_all();
        assert_eq!(app.selection.len(), 2);
        app.cut_selection();
        assert!(app.doc.fields().is_empty());
        app.paste_clipboard();
        assert_eq!(app.doc.fields().len(), 2);
    }

    #[test]
    fn two_selected_items_show_align_and_distribute_in_the_sidebar() {
        let ctx = egui::Context::default();
        let mut app = PageFormApp::new(false, None);
        let first = app.doc.add_field(
            FieldKind::Text,
            RectPt {
                x: 72.0,
                y: 700.0,
                w: 72.0,
                h: 18.0,
            },
        );
        let second = app.doc.add_field(
            FieldKind::Text,
            RectPt {
                x: 200.0,
                y: 640.0,
                w: 90.0,
                h: 18.0,
            },
        );
        app.selection = vec![first, second];
        let output = panel(&ctx, &mut app, |app, ui| app.properties_panel(ui));
        let texts = galley_texts(&output);
        for label in [
            "Align",
            "Left",
            "Center",
            "Right",
            "Top",
            "Middle",
            "Bottom",
            "Horizontally",
            "Vertically",
            "Width",
            "Height",
        ] {
            assert!(
                texts.iter().any(|text| text.contains(label)),
                "{label} missing from {texts:?}"
            );
        }
    }

    #[test]
    fn builtin_export_path_adds_pdf_and_starts_from_the_fallback_name() {
        let path = export_file_path("/tmp/forms", "card");
        assert_eq!(path, std::path::PathBuf::from("/tmp/forms/card.pdf"));
        let kept = export_file_path("/tmp/forms", "card.PDF");
        assert_eq!(kept.extension().and_then(|ext| ext.to_str()), Some("PDF"));
        assert_eq!(export_file_name("sample-form.pdf"), "sample-form.pdf");
        assert_eq!(export_file_name(""), "form.pdf");
        let mut app = PageFormApp::new(false, None);
        app.open_builtin_prompt(PromptKind::ExportPdf);
        let prompt = app.file_prompt.expect("builtin dialog");
        assert_eq!(prompt.file_name, "sample-form.pdf");
        assert!(!prompt.replace);
    }

    #[test]
    fn save_then_open_restores_the_project_and_clears_the_dirty_flag() {
        let path =
            std::env::temp_dir().join(format!("pageform-roundtrip-{}.pgd", std::process::id()));
        let mut app = PageFormApp::new(false, None);
        assert!(!app.doc.is_dirty());
        assert_eq!(app.window_title(), "Untitled - PageForm");
        app.insert_at_view(FieldKind::Text);
        let id = app.primary().unwrap();
        app.doc.set_default_value(id, "Ada");
        assert!(app.doc.is_dirty());
        assert!(app.window_title().contains('*'));
        app.write_project_file(&path);
        assert!(!app.doc.is_dirty());
        assert_eq!(app.project_path.as_deref(), Some(path.to_str().unwrap()));
        app.doc.set_header_text("Changed");
        assert!(app.doc.is_dirty());
        app.read_project_file(&path);
        assert!(!app.doc.is_dirty());
        assert_eq!(app.doc.header_text(), "Header");
        assert_eq!(app.doc.fields().len(), 1);
        assert_eq!(app.doc.fields()[0].style().default_value, "Ada");
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("kind: Document"), "{saved}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_template_opens_as_untitled_and_save_does_not_overwrite_it() {
        let dir = std::env::temp_dir();
        let template = dir.join(format!("pageform-template-{}.pgt", std::process::id()));
        let document = dir.join(format!("pageform-from-template-{}.pgd", std::process::id()));
        let legacy = dir.join(format!("pageform-legacy-{}.pageform", std::process::id()));
        let mut app = PageFormApp::new(false, None);
        app.insert_at_view(FieldKind::Text);
        let id = app.primary().unwrap();
        app.doc.set_default_value(id, "Ada");
        app.write_project_file(&template);
        assert!(app.project_path.is_none());
        assert!(app.doc.is_dirty());
        assert!(app.window_title().starts_with("Untitled"));
        let template_bytes = std::fs::read(&template).unwrap();
        let template_text = String::from_utf8(template_bytes.clone()).unwrap();
        assert!(template_text.contains("kind: Template"), "{template_text}");

        app.new_blank();
        app.read_project_file(&template);
        assert!(app.project_path.is_none());
        assert!(app.doc.is_dirty());
        assert_eq!(app.window_title(), "Untitled * - PageForm");
        assert_eq!(app.doc.fields()[0].style().default_value, "Ada");
        assert!(app.linked_document_path().is_none());
        assert_eq!(std::fs::read(&template).unwrap(), template_bytes);
        assert_eq!(app.recent[0], template.display().to_string());

        app.read_project_file_as_untitled(&template);
        assert!(app.project_path.is_none());
        assert_eq!(app.window_title(), "Untitled * - PageForm");
        assert_eq!(std::fs::read(&template).unwrap(), template_bytes);

        app.write_project_file(&document);
        assert!(!app.doc.is_dirty());
        assert_eq!(
            app.project_path.as_deref(),
            Some(document.to_str().unwrap())
        );
        assert_eq!(std::fs::read(&template).unwrap(), template_bytes);
        let document_bytes = std::fs::read(&document).unwrap();
        app.write_project_file(&template);
        assert!(!app.doc.is_dirty());
        assert_eq!(
            app.project_path.as_deref(),
            Some(document.to_str().unwrap())
        );
        assert_eq!(std::fs::read(&document).unwrap(), document_bytes);
        assert!(app.recent.contains(&template.display().to_string()));
        assert!(app.recent.contains(&document.display().to_string()));

        let legacy_text = template_text.replacen("kind: Template,\n", "", 1);
        std::fs::write(&legacy, &legacy_text).unwrap();
        app.read_project_file(&legacy);
        assert!(!app.doc.is_dirty());
        assert_eq!(app.project_path.as_deref(), Some(legacy.to_str().unwrap()));
        assert!(app.window_title().contains(".pageform"));

        let mut fresh = PageFormApp::new(false, None);
        fresh.open_builtin_prompt(PromptKind::SaveProject);
        let prompt = fresh.file_prompt.clone().expect("save dialog");
        assert!(!prompt.save_as_template);
        assert_eq!(prompt.file_name, "Untitled.pgd");
        fresh.open_builtin_prompt(PromptKind::OpenTemplate);
        assert_eq!(
            fresh.file_prompt.expect("template dialog").kind,
            PromptKind::OpenTemplate
        );
        assert_eq!(
            suggested_project_name(None, false),
            "Untitled.pgd".to_string()
        );
        assert_eq!(
            project_save_path("/tmp/forms", "card.pgd", true),
            std::path::PathBuf::from("/tmp/forms/card.pgt")
        );
        assert_eq!(
            project_save_path("/tmp/forms", "card.pageform", false),
            std::path::PathBuf::from("/tmp/forms/card.pgd")
        );
        assert_eq!(
            native_project_save_path(std::path::PathBuf::from("/tmp/forms/card")),
            std::path::PathBuf::from("/tmp/forms/card.pgd")
        );
        assert!(is_template_path(&template));
        let suggested = suggested_project_name(app.project_path.as_deref(), false);
        assert!(suggested.ends_with(".pgd"), "{suggested}");
        assert!(!suggested.ends_with(".pageform"), "{suggested}");

        let _ = std::fs::remove_file(&template);
        let _ = std::fs::remove_file(&document);
        let _ = std::fs::remove_file(&legacy);
    }

    #[test]
    fn opening_a_template_leaves_no_linked_path() {
        let template =
            std::env::temp_dir().join(format!("pageform-unlinked-{}.pgt", std::process::id()));
        let mut source = PageFormApp::new(false, None);
        source.insert_at_view(FieldKind::Text);
        let id = source.primary().unwrap();
        source.doc.set_default_value(id, "Ada");
        let text = source
            .doc
            .to_project_ron(pageform_core::ProjectKind::Template)
            .unwrap();
        std::fs::write(&template, &text).unwrap();
        let original = std::fs::read(&template).unwrap();

        let mut app = PageFormApp::new(false, None);
        app.read_project_file(&template);
        assert!(
            app.project_path.is_none(),
            "opening a template must not link a path"
        );
        assert!(app.linked_document_path().is_none());
        assert_eq!(app.window_title(), "Untitled * - PageForm");
        assert_eq!(app.doc.fields()[0].style().default_value, "Ada");
        assert_eq!(
            suggested_project_name(app.project_path.as_deref(), false),
            "Untitled.pgd"
        );
        app.doc.set_header_text("Edited");
        app.open_builtin_prompt(PromptKind::SaveProject);
        let prompt = app.file_prompt.clone().expect("plain save asks for a name");
        assert_eq!(prompt.kind, PromptKind::SaveProject);
        assert!(!prompt.save_as_template);
        assert_eq!(prompt.file_name, "Untitled.pgd");
        let document_path = project_save_path(
            &prompt.directory,
            template.file_name().unwrap().to_str().unwrap(),
            false,
        );
        assert_eq!(
            document_path.extension().and_then(|ext| ext.to_str()),
            Some("pgd")
        );
        assert_ne!(document_path, template);
        assert_eq!(std::fs::read(&template).unwrap(), original);

        let _ = std::fs::remove_file(&template);
    }

    #[test]
    fn replacing_an_existing_template_asks_before_writing() {
        let ctx = egui::Context::default();
        let dir = std::env::temp_dir();
        let template = dir.join(format!("pageform-replace-{}.pgt", std::process::id()));
        let mut app = PageFormApp::new(false, None);
        app.insert_at_view(FieldKind::Text);
        let id = app.primary().unwrap();
        app.doc.set_default_value(id, "Ada");
        let original = app
            .doc
            .to_project_ron(pageform_core::ProjectKind::Template)
            .unwrap();
        std::fs::write(&template, &original).unwrap();
        app.doc.set_default_value(id, "Changed");

        app.open_builtin_prompt(PromptKind::SaveProject);
        {
            let prompt = app.file_prompt.as_mut().expect("save dialog");
            prompt.directory = dir.display().to_string();
            prompt.file_name = template
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .replace(".pgt", ".pgd");
        }
        let _ = show_settled(&ctx, &mut app, |app, ctx| {
            egui::CentralPanel::default().show(ctx, |_| {});
            app.export_dialog(ctx);
        });
        let at = text_center(
            &show_settled(&ctx, &mut app, |app, ctx| {
                egui::CentralPanel::default().show(ctx, |_| {});
                app.export_dialog(ctx);
            }),
            "Template (.pgt)",
        )
        .expect("template type");
        click_at(&ctx, &mut app, at, |app, ctx| {
            egui::CentralPanel::default().show(ctx, |_| {});
            app.export_dialog(ctx);
        });
        let prompt = app.file_prompt.clone().expect("dialog stays open");
        assert!(prompt.save_as_template);
        assert!(prompt.file_name.ends_with(".pgt"), "{}", prompt.file_name);

        let save_at = text_center(
            &show_settled(&ctx, &mut app, |app, ctx| {
                egui::CentralPanel::default().show(ctx, |_| {});
                app.export_dialog(ctx);
            }),
            "Save",
        )
        .expect("save");
        click_at(&ctx, &mut app, save_at, |app, ctx| {
            egui::CentralPanel::default().show(ctx, |_| {});
            app.export_dialog(ctx);
        });
        assert_eq!(std::fs::read_to_string(&template).unwrap(), original);
        assert!(app.project_path.is_none());
        let confirm = show_settled(&ctx, &mut app, |app, ctx| {
            egui::CentralPanel::default().show(ctx, |_| {});
            app.export_dialog(ctx);
        });
        let texts = galley_texts(&confirm);
        assert!(
            texts.iter().any(|text| text.contains("already exists")),
            "{texts:?}"
        );
        let replace_at = text_center(&confirm, "Replace").expect("replace");
        click_at(&ctx, &mut app, replace_at, |app, ctx| {
            egui::CentralPanel::default().show(ctx, |_| {});
            app.export_dialog(ctx);
        });
        let saved = std::fs::read_to_string(&template).unwrap();
        assert_ne!(saved, original);
        assert!(saved.contains("kind: Template"), "{saved}");
        assert!(saved.contains("Changed"), "{saved}");
        assert!(
            app.project_path.is_none(),
            "saving a template must not link it"
        );
        assert!(app.linked_document_path().is_none());

        let _ = std::fs::remove_file(&template);
    }

    #[test]
    fn save_as_dialog_switches_between_document_and_template() {
        let ctx = egui::Context::default();
        let mut app = PageFormApp::new(false, None);
        app.open_builtin_prompt(PromptKind::SaveProject);
        let output = show_settled(&ctx, &mut app, |app, ctx| {
            egui::CentralPanel::default().show(ctx, |_| {});
            app.export_dialog(ctx);
        });
        let texts = galley_texts(&output);
        assert!(
            texts.iter().any(|text| text.contains("Document (.pgd)")),
            "{texts:?}"
        );
        assert!(
            texts.iter().any(|text| text.contains("Template (.pgt)")),
            "{texts:?}"
        );
        assert!(
            texts.iter().any(|text| text.contains("Untitled.pgd")),
            "{texts:?}"
        );
        let at = text_center(&output, "Template (.pgt)").expect("template type");
        click_at(&ctx, &mut app, at, |app, ctx| {
            egui::CentralPanel::default().show(ctx, |_| {});
            app.export_dialog(ctx);
        });
        let prompt = app.file_prompt.clone().expect("dialog stays open");
        assert!(prompt.save_as_template);
        assert_eq!(prompt.file_name, "Untitled.pgt");
    }

    #[test]
    fn open_dialog_lists_documents_templates_and_legacy_recent_files() {
        let ctx = egui::Context::default();
        let mut app = PageFormApp::new(false, None);
        app.recent = vec![
            "/tmp/form.pgd".to_string(),
            "/tmp/form.pgt".to_string(),
            "/tmp/old.pageform".to_string(),
        ];
        app.open_builtin_prompt(PromptKind::OpenProject);
        let output = show_settled(&ctx, &mut app, |app, ctx| {
            egui::CentralPanel::default().show(ctx, |_| {});
            app.export_dialog(ctx);
        });
        let texts = galley_texts(&output);
        for path in ["/tmp/form.pgd", "/tmp/form.pgt", "/tmp/old.pageform"] {
            assert!(
                texts.iter().any(|text| text == path),
                "{path} missing from {texts:?}"
            );
        }
        app.open_builtin_prompt(PromptKind::OpenTemplate);
        let output = show_settled(&ctx, &mut app, |app, ctx| {
            egui::CentralPanel::default().show(ctx, |_| {});
            app.export_dialog(ctx);
        });
        let texts = galley_texts(&output);
        assert!(
            texts.iter().any(|text| text == "/tmp/form.pgt"),
            "{texts:?}"
        );
        assert!(
            texts.iter().all(|text| text != "/tmp/form.pgd"),
            "{texts:?}"
        );
    }

    #[test]
    fn a_short_move_places_the_default_size_and_a_drag_does_not() {
        let view = ViewTransform::new(792.0);
        let start = PdfPoint { x: 100.0, y: 400.0 };
        assert!(is_click(&view, start, PdfPoint { x: 104.0, y: 400.0 }));
        assert!(!is_click(&view, start, PdfPoint { x: 120.0, y: 400.0 }));

        let mut app = PageFormApp::new(false, None);
        app.view.zoom = 1.0;
        app.tool = Tool::Text;
        let clicked = app.place_rect(start, PdfPoint { x: 104.0, y: 402.0 }, FieldKind::Text);
        let (w, h) = default_size(FieldKind::Text, GridSize::Medium);
        assert!((clicked.w - w).abs() < 0.1);
        assert!((clicked.h - h).abs() < 0.1);

        app.tool = Tool::Checkbox;
        let check = app.place_rect(start, PdfPoint { x: 104.0, y: 402.0 }, FieldKind::Checkbox);
        let (check_w, check_h) = default_size(FieldKind::Checkbox, GridSize::Medium);
        assert!((check.w - check_w).abs() < 0.1);
        assert!((check.h - check_h).abs() < 0.1);

        let dragged = app.place_rect(start, PdfPoint { x: 180.0, y: 340.0 }, FieldKind::Text);
        assert!((dragged.w - w).abs() > 1.0 || (dragged.h - h).abs() > 1.0);
    }

    #[test]
    fn square_drag_keeps_equal_sides_and_rectangle_drag_does_not() {
        let page = Page::letter();
        let start = PdfPoint { x: 72.0, y: 200.0 };
        let end = PdfPoint { x: 180.0, y: 140.0 };
        let square = square_from_drag(start, end, None, 9.0, &page);
        assert!((square.w - square.h).abs() < 0.1);
        assert!((square.w - 108.0).abs() < 0.1);
        let rect = rect_from_drag(start, end, None, 9.0, &page);
        assert!((rect.w - 108.0).abs() < 0.1);
        assert!((rect.h - 60.0).abs() < 0.1);
        assert!((rect.w - rect.h).abs() > 1.0);
    }

    #[test]
    fn file_and_edit_stubs_report_in_the_status_bar() {
        let mut app = PageFormApp::new(false, None);
        assert!(!app.show_preferences);
        assert!(!app.show_about);
        assert!(!app.show_help);
        for command in [
            "New",
            "Open",
            "Save",
            "Save As",
            "Print",
            "New Blank Document",
            "New from Template",
            "Insert Page",
        ] {
            app.mark_not_implemented(command);
            assert_eq!(app.status, format!("{command} is not implemented yet"));
        }
    }

    #[test]
    fn properties_panel_starts_hidden_and_unpinned_closes_with_the_selection() {
        let mut app = PageFormApp::new(false, None);
        assert!(!app.properties_open);
        assert!(!app.properties_pinned);
        let mut storage = MemStorage::default();
        write_preferences(&app, &mut storage);
        let restored = PageFormApp::new(false, Some(&storage));
        assert!(!restored.properties_open);
        assert!(!restored.properties_pinned);
        storage.set_string(crate::prefs::OPEN_KEY, "open".to_string());
        storage.set_string(crate::prefs::PIN_KEY, "pinned".to_string());
        let remembered = PageFormApp::new(false, Some(&storage));
        assert!(remembered.properties_open);
        assert!(remembered.properties_pinned);
        app.open_properties();
        assert!(app.properties_open);
        assert!(app.persist_panel);
        app.insert_at_view(FieldKind::Text);
        app.dismiss_unpinned();
        assert!(!app.properties_open);
        app.open_properties();
        app.set_properties_pinned(true);
        app.persist_panel = false;
        app.delete_selection();
        assert!(app.properties_open);
        assert!(app.selection.is_empty());
        let first = FieldId(1);
        let second = FieldId(2);
        assert_eq!(
            selection_after_click(&[first], second, true),
            vec![first, second]
        );
        assert_eq!(
            selection_after_click(&[first, second], first, true),
            vec![second]
        );
        assert_eq!(
            selection_after_click(&[first, second], first, false),
            vec![first, second]
        );
        assert_eq!(selection_after_click(&[first], second, false), vec![second]);
    }

    #[test]
    fn canvas_text_edit_updates_the_field_and_escape_restores_it() {
        let mut app = PageFormApp::new(false, None);
        app.insert_at_view(FieldKind::StaticText);
        let id = app.primary().unwrap();
        assert!(app.begin_inline(id));
        assert!(app.status.starts_with("Editing text."));
        assert!(!app.properties_open);
        assert_eq!(app.inline.as_ref().map(|edit| edit.multiline), Some(false));
        app.inline.as_mut().unwrap().buffer = "Full name".to_string();
        app.apply_inline();
        assert_eq!(app.doc.field(id).unwrap().caption(), "Full name");
        assert_eq!(app.caption_buf, "Full name");
        app.cancel_inline();
        assert!(app.inline.is_none());
        assert_eq!(app.status, "Placed Text");
        assert!(!app.status.starts_with("Editing text."));
        assert_eq!(app.doc.field(id).unwrap().caption(), "Label");
        assert_eq!(app.caption_buf, "Label");

        app.insert_at_view(FieldKind::TextBox);
        let text_box = app.primary().unwrap();
        assert!(app.begin_inline(text_box));
        assert!(app.inline.as_ref().unwrap().multiline);
        app.inline.as_mut().unwrap().buffer = "Line one\nLine two".to_string();
        app.apply_inline();
        assert_eq!(
            app.doc.field(text_box).unwrap().caption(),
            "Line one\nLine two"
        );

        app.insert_at_view(FieldKind::Text);
        let text = app.primary().unwrap();
        assert!(app.begin_inline(text));
        app.inline.as_mut().unwrap().buffer = "Ada".to_string();
        app.apply_inline();
        assert_eq!(app.doc.field(text).unwrap().style().default_value, "Ada");
        assert_eq!(app.value_buf, "Ada");

        app.insert_at_view(FieldKind::Rectangle);
        let shape = app.primary().unwrap();
        assert!(!app.begin_inline(shape));
        assert_eq!(app.inline.as_ref().map(|edit| edit.id), Some(text));
        app.finish_inline();
        assert!(app.inline.is_none());
        assert!(
            !app.status.starts_with("Editing text."),
            "status was {}",
            app.status
        );
    }

    #[test]
    fn delete_removes_the_selected_field() {
        let mut app = PageFormApp::new(true, None);
        let id = app.doc.fields()[0].id();
        app.selection = vec![id];
        app.delete_selection();
        assert!(app.doc.field(id).is_none());
        assert!(app.selection.is_empty());
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
        assert_eq!(app.selection, vec![field.id()]);
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

    #[test]
    fn menus_and_dialogs_paint_their_items() {
        let ctx = egui::Context::default();
        let mut app = PageFormApp::new(false, None);

        let bar = panel(&ctx, &mut app, |app, ui| app.menu_bar(ui));
        assert_eq!(ordered_hits(&galley_texts(&bar), MENU_BAR), MENU_BAR);

        let file = panel(&ctx, &mut app, |app, ui| app.file_menu(ui));
        assert_eq!(
            ordered_hits(&galley_texts(&file), &menu_entries(FILE_MENU)),
            menu_entries(FILE_MENU)
        );
        assert!(
            galley_texts(&file).iter().any(|text| text == "Open Recent"),
            "Open Recent stays visible when the list is empty"
        );
        let edit = panel(&ctx, &mut app, |app, ui| app.edit_menu(ui));
        assert_eq!(
            ordered_hits(&galley_texts(&edit), &menu_entries(EDIT_MENU)),
            menu_entries(EDIT_MENU)
        );
        let insert = panel(&ctx, &mut app, |app, ui| app.insert_menu(ui));
        let insert_labels = sorted_insert_kinds().map(FieldKind::label);
        assert_eq!(
            ordered_hits(&galley_texts(&insert), &insert_labels),
            insert_labels.to_vec()
        );
        let help = panel(&ctx, &mut app, |app, ui| app.help_menu(ui));
        assert_eq!(
            ordered_hits(&galley_texts(&help), HELP_MENU),
            HELP_MENU.to_vec()
        );

        app.show_preferences = true;
        app.show_about = true;
        app.show_help = true;
        let dialogs = show_settled(&ctx, &mut app, |app, ctx| {
            egui::CentralPanel::default().show(ctx, |_| {});
            app.preferences_dialog(ctx);
            app.about_dialog(ctx);
            app.help_window(ctx);
        });
        let texts = galley_texts(&dialogs);
        assert!(texts.iter().any(|text| text == "Preferences"));
        assert!(texts.iter().any(|text| text == "Dark"));
        assert!(texts.iter().any(|text| text == "Light"));
        assert!(texts.iter().any(|text| text == "Radio labels"));
        assert!(texts.iter().any(|text| text == "Open by default"));
        assert!(texts.iter().any(|text| text == "Pinned by default"));
        assert!(texts.iter().any(|text| text == "PageForm"));
        assert!(texts.iter().any(|text| text == ABOUT_TAGLINE));
        assert!(texts
            .iter()
            .any(|text| text == &format!("Version {}", app_version())));
        assert!(texts.iter().any(|text| text == "OK"));
        assert!(texts.iter().any(|text| text == "E"));
        assert!(texts.iter().any(|text| text == "PageForm Help"));
        assert!(texts.iter().any(|text| text == "What is PageForm?"));
        assert!(app.show_help, "headless help uses the embedded window");
    }

    #[test]
    fn clicking_a_stub_and_preferences_updates_state() {
        let ctx = egui::Context::default();
        let mut app = PageFormApp::new(false, None);

        click_menu(&ctx, &mut app, "Select All", |app, ui| app.edit_menu(ui));
        assert_eq!(app.status, "Selected 0 items");
        click_menu(&ctx, &mut app, "Paste", |app, ui| app.edit_menu(ui));
        assert_eq!(app.status, "Selected 0 items");

        click_menu(&ctx, &mut app, "Preferences", |app, ui| app.edit_menu(ui));
        assert!(app.show_preferences);
        click_menu(&ctx, &mut app, "Help", |app, ui| app.help_menu(ui));
        assert!(app.show_help);
        click_menu(&ctx, &mut app, "About", |app, ui| app.help_menu(ui));
        assert!(app.show_about);

        let prefs = show_settled(&ctx, &mut app, |app, ctx| {
            egui::CentralPanel::default().show(ctx, |_| {});
            app.preferences_dialog(ctx);
        });
        let light_at = text_center(&prefs, "Light").expect("Light");
        click_at(&ctx, &mut app, light_at, |app, ctx| {
            egui::CentralPanel::default().show(ctx, |_| {});
            app.preferences_dialog(ctx);
        });
        assert!(!app.dark);
        assert!(app.persist_theme);
    }

    fn screen_input() -> egui::RawInput {
        use std::cell::Cell;
        thread_local! {
            static TIME: Cell<f64> = const { Cell::new(0.0) };
        }
        let time = TIME.with(|time| {
            let next = time.get() + 1.0;
            time.set(next);
            next
        });
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0))),
            time: Some(time),
            ..Default::default()
        }
    }

    fn run_ui(
        ctx: &egui::Context,
        app: &mut PageFormApp,
        input: egui::RawInput,
        mut show: impl FnMut(&mut PageFormApp, &egui::Context),
    ) -> egui::FullOutput {
        ctx.run(input, |ctx| show(app, ctx))
    }

    /// Areas and windows measure themselves on the first frame and paint on the next.
    fn show_settled(
        ctx: &egui::Context,
        app: &mut PageFormApp,
        mut show: impl FnMut(&mut PageFormApp, &egui::Context),
    ) -> egui::FullOutput {
        let _ = run_ui(ctx, app, screen_input(), &mut show);
        run_ui(ctx, app, screen_input(), show)
    }

    fn panel(
        ctx: &egui::Context,
        app: &mut PageFormApp,
        mut show: impl FnMut(&mut PageFormApp, &mut egui::Ui),
    ) -> egui::FullOutput {
        run_ui(ctx, app, screen_input(), |app, ctx| {
            egui::CentralPanel::default().show(ctx, |ui| show(app, ui));
        })
    }

    fn click_menu(
        ctx: &egui::Context,
        app: &mut PageFormApp,
        label: &str,
        mut show: impl FnMut(&mut PageFormApp, &mut egui::Ui) + Clone,
    ) {
        let output = panel(ctx, app, show.clone());
        let at = text_center(&output, label).unwrap_or_else(|| panic!("missing {label}"));
        click_at(ctx, app, at, |app, ctx| {
            egui::CentralPanel::default().show(ctx, |ui| show(app, ui));
        });
    }

    fn click_at(
        ctx: &egui::Context,
        app: &mut PageFormApp,
        pos: Pos2,
        mut show: impl FnMut(&mut PageFormApp, &egui::Context),
    ) {
        let mut raw = screen_input();
        raw.events = vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            },
        ];
        let _ = run_ui(ctx, app, raw, &mut show);
    }

    fn ordered_hits(texts: &[String], wanted: &[&str]) -> Vec<String> {
        let wanted: std::collections::HashSet<&str> = wanted.iter().copied().collect();
        texts
            .iter()
            .filter(|text| wanted.contains(text.as_str()))
            .cloned()
            .collect()
    }

    fn galley_texts(output: &egui::FullOutput) -> Vec<String> {
        let mut texts = Vec::new();
        for clipped in &output.shapes {
            collect_text(&clipped.shape, &mut texts);
        }
        texts
    }

    fn collect_text(shape: &egui::epaint::Shape, texts: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(text) => {
                let value = text.galley.text().trim();
                if !value.is_empty() {
                    texts.push(value.to_string());
                }
            }
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_text(shape, texts);
                }
            }
            _ => {}
        }
    }

    fn text_center(output: &egui::FullOutput, label: &str) -> Option<Pos2> {
        fn walk(shape: &egui::epaint::Shape, label: &str) -> Option<Pos2> {
            match shape {
                egui::epaint::Shape::Text(text) if text.galley.text().trim() == label => {
                    Some(text.pos + Vec2::new(4.0, text.galley.size().y * 0.5))
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
