//! Fillable AcroForm export.
//!
//! Text fields get a real `/DA` (`/Helv 12 Tf 0 g`) and a normal appearance
//! stream. Checkboxes get explicit `/AP /N` entries named `/On` and `/Off`.
//! Radio buttons in one group share a parent field (`/FT /Btn`, radio flag)
//! and each widget has its own on-state name. Drop-downs are combo boxes
//! (`/Ff` combo), not list boxes. Dates are single-line text fields with
//! `/MaxLen 10` and an alternate name `YYYY-MM-DD`. Signature fields are
//! unsigned `/FT /Sig` widgets. Static text and the table placeholder are
//! page content, never AcroForm fields.
//! `NeedAppearances` is never set.

use pdf_writer::types::{AnnotationFlags, BorderType, FieldFlags, FieldType, Quadding};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

use crate::document::{Document, Field, FieldKind, DATE_HINT, TABLE_COLUMNS, TABLE_ROWS};

const FONT_KEY: &[u8] = b"Helv";
const DA: &[u8] = b"/Helv 12 Tf 0 g";

#[derive(Debug)]
pub enum ExportError {
    EmptyName,
    DuplicateName(String),
    InvalidRect { name: String },
    InvalidRadioState { name: String },
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::EmptyName => write!(f, "A field is missing a name"),
            ExportError::DuplicateName(name) => write!(f, "Duplicate field name \"{name}\""),
            ExportError::InvalidRect { name } => {
                write!(f, "Field \"{name}\" has a zero or invalid rectangle")
            }
            ExportError::InvalidRadioState { name } => {
                write!(f, "Radio group \"{name}\" has a blank or repeated option")
            }
        }
    }
}

impl std::error::Error for ExportError {}

struct Ids {
    next: i32,
}

impl Ids {
    fn next(&mut self) -> Ref {
        let id = Ref::new(self.next);
        self.next += 1;
        id
    }
}

struct RadioGroup {
    name: String,
    parent: Ref,
    kids: Vec<Ref>,
}

/// Serialize `doc` to a single-page fillable PDF.
pub fn export_pdf(doc: &Document) -> Result<Vec<u8>, ExportError> {
    validate(doc)?;

    let mut pdf = Pdf::new();
    let mut ids = Ids { next: 1 };
    let font_id = ids.next();
    let page_id = ids.next();
    let contents_id = ids.next();
    let pages_id = ids.next();
    let catalog_id = ids.next();

    let mut annots = Vec::new();
    let mut roots = Vec::new();
    let mut groups: Vec<RadioGroup> = Vec::new();

    for field in doc.fields() {
        if !field.is_form_field() {
            continue;
        }
        match field.kind() {
            FieldKind::Text => {
                let widget_id = ids.next();
                let appearance_id = ids.next();
                write_text_field(
                    &mut pdf,
                    field,
                    widget_id,
                    appearance_id,
                    page_id,
                    font_id,
                    TextExtras::none(),
                );
                annots.push(widget_id);
                roots.push(widget_id);
            }
            FieldKind::Date => {
                let widget_id = ids.next();
                let appearance_id = ids.next();
                write_text_field(
                    &mut pdf,
                    field,
                    widget_id,
                    appearance_id,
                    page_id,
                    font_id,
                    TextExtras {
                        max_len: Some(10),
                        alternate: Some(DATE_HINT),
                        hint: Some(DATE_HINT),
                    },
                );
                annots.push(widget_id);
                roots.push(widget_id);
            }
            FieldKind::Checkbox => {
                let widget_id = ids.next();
                let on_id = ids.next();
                let off_id = ids.next();
                write_checkbox(&mut pdf, field, widget_id, on_id, off_id, page_id);
                annots.push(widget_id);
                roots.push(widget_id);
            }
            FieldKind::Radio => {
                let widget_id = ids.next();
                let on_id = ids.next();
                let off_id = ids.next();
                let parent = if let Some(group) =
                    groups.iter_mut().find(|group| group.name == field.name())
                {
                    group.kids.push(widget_id);
                    group.parent
                } else {
                    let parent = ids.next();
                    groups.push(RadioGroup {
                        name: field.name().to_string(),
                        parent,
                        kids: vec![widget_id],
                    });
                    roots.push(parent);
                    parent
                };
                write_radio_widget(&mut pdf, field, widget_id, on_id, off_id, parent, page_id);
                annots.push(widget_id);
            }
            FieldKind::Signature => {
                let widget_id = ids.next();
                let appearance_id = ids.next();
                write_signature(&mut pdf, field, widget_id, appearance_id, page_id, font_id);
                annots.push(widget_id);
                roots.push(widget_id);
            }
            FieldKind::Dropdown => {
                let widget_id = ids.next();
                let appearance_id = ids.next();
                write_dropdown(&mut pdf, field, widget_id, appearance_id, page_id, font_id);
                annots.push(widget_id);
                roots.push(widget_id);
            }
            FieldKind::StaticText
            | FieldKind::Table
            | FieldKind::Line
            | FieldKind::Rectangle
            | FieldKind::TextBox
            | FieldKind::Image => {}
        }
    }

    for group in &groups {
        let mut field = pdf.form_field(group.parent);
        field
            .field_type(FieldType::Button)
            .field_flags(FieldFlags::RADIO)
            .partial_name(TextStr(&group.name))
            .children(group.kids.iter().copied())
            .radio_value(Name(b"Off"))
            .radio_default_value(Name(b"Off"));
        field.finish();
    }

    let (content, needs_font) = page_content(doc);
    pdf.stream(contents_id, &content);

    let page = doc.page();
    let mut page_writer = pdf.page(page_id);
    page_writer
        .media_box(Rect::new(0.0, 0.0, page.width, page.height))
        .parent(pages_id)
        .contents(contents_id);
    if needs_font {
        page_writer
            .resources()
            .fonts()
            .pair(Name(FONT_KEY), font_id);
    }
    if !annots.is_empty() {
        page_writer.annotations(annots.iter().copied());
    }
    page_writer.finish();

    pdf.pages(pages_id).count(1).kids([page_id]);

    pdf.type1_font(font_id)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));

    let mut catalog = pdf.catalog(catalog_id);
    catalog.pages(pages_id);
    {
        let mut form = catalog.form();
        form.fields(roots.iter().copied())
            .default_appearance(Str(DA));
        form.default_resources()
            .fonts()
            .pair(Name(FONT_KEY), font_id);
    }
    catalog.finish();

    Ok(pdf.finish())
}

struct TextExtras {
    max_len: Option<i32>,
    alternate: Option<&'static str>,
    hint: Option<&'static str>,
}

impl TextExtras {
    fn none() -> Self {
        Self {
            max_len: None,
            alternate: None,
            hint: None,
        }
    }
}

fn validate(doc: &Document) -> Result<(), ExportError> {
    let mut seen: Vec<(String, FieldKind)> = Vec::new();
    for field in doc.fields().iter().filter(|field| field.is_form_field()) {
        let name = field.name();
        if name.trim().is_empty() {
            return Err(ExportError::EmptyName);
        }
        if let Some((_, existing)) = seen.iter().find(|(existing, _)| existing == name) {
            if *existing == FieldKind::Radio && field.kind() == FieldKind::Radio {
                continue;
            }
            return Err(ExportError::DuplicateName(name.to_string()));
        }
        let rect = field.rect();
        if !rect.w.is_finite() || !rect.h.is_finite() || rect.w < 1.0 || rect.h < 1.0 {
            return Err(ExportError::InvalidRect {
                name: name.to_string(),
            });
        }
        seen.push((name.to_string(), field.kind()));
    }

    let mut groups: Vec<&str> = Vec::new();
    for field in doc
        .fields()
        .iter()
        .filter(|field| field.kind() == FieldKind::Radio)
    {
        if groups.contains(&field.name()) {
            continue;
        }
        groups.push(field.name());
        let mut states: Vec<&str> = Vec::new();
        for member in doc
            .fields()
            .iter()
            .filter(|member| member.kind() == FieldKind::Radio && member.name() == field.name())
        {
            let state = member.on_state();
            if state.is_empty() || state.eq_ignore_ascii_case("Off") || states.contains(&state) {
                return Err(ExportError::InvalidRadioState {
                    name: field.name().to_string(),
                });
            }
            states.push(state);
        }
    }
    Ok(())
}

fn write_text_field(
    pdf: &mut Pdf,
    field: &Field,
    widget_id: Ref,
    appearance_id: Ref,
    page_id: Ref,
    font_id: Ref,
    extras: TextExtras,
) {
    let rect = field.rect();
    let bytes = text_appearance(rect.w, rect.h, extras.hint);
    let mut xobject = pdf.form_xobject(appearance_id, &bytes);
    xobject.bbox(Rect::new(0.0, 0.0, rect.w, rect.h));
    xobject.resources().fonts().pair(Name(FONT_KEY), font_id);
    xobject.finish();

    let mut widget = pdf.form_field(widget_id);
    widget
        .field_type(FieldType::Text)
        .partial_name(TextStr(field.name()))
        .text_value(TextStr(""))
        .vartext_default_appearance(Str(DA))
        .vartext_quadding(Quadding::Left);
    if let Some(max_len) = extras.max_len {
        widget.text_max_len(max_len);
    }
    if let Some(alternate) = extras.alternate {
        widget.alternate_name(TextStr(alternate));
    }
    {
        let mut resources = widget.insert(Name(b"DR")).dict();
        resources
            .insert(Name(b"Font"))
            .dict()
            .pair(Name(FONT_KEY), font_id);
    }
    let mut annot = widget.into_annotation();
    annot
        .rect(Rect::new(rect.x, rect.y, rect.right(), rect.top()))
        .flags(AnnotationFlags::PRINT)
        .page(page_id);
    annot.border_style().width(1.0).style(BorderType::Solid);
    annot
        .appearance_characteristics()
        .border_color_rgb(0.0, 0.0, 0.0)
        .background_color_rgb(1.0, 1.0, 1.0);
    annot.appearance().normal().stream(appearance_id);
    annot.finish();
}

fn write_checkbox(
    pdf: &mut Pdf,
    field: &Field,
    widget_id: Ref,
    on_id: Ref,
    off_id: Ref,
    page_id: Ref,
) {
    let rect = field.rect();
    write_box_appearance(pdf, on_id, rect.w, rect.h, true);
    write_box_appearance(pdf, off_id, rect.w, rect.h, false);

    let mut widget = pdf.form_field(widget_id);
    widget
        .field_type(FieldType::Button)
        .field_flags(FieldFlags::empty())
        .partial_name(TextStr(field.name()))
        .pair(Name(b"V"), Name(b"Off"))
        .pair(Name(b"DV"), Name(b"Off"));
    let mut annot = widget.into_annotation();
    annot
        .rect(Rect::new(rect.x, rect.y, rect.right(), rect.top()))
        .flags(AnnotationFlags::PRINT)
        .appearance_state(Name(b"Off"))
        .page(page_id);
    annot.border_style().width(1.0).style(BorderType::Solid);
    annot
        .appearance_characteristics()
        .border_color_rgb(0.0, 0.0, 0.0)
        .background_color_rgb(1.0, 1.0, 1.0);
    annot
        .appearance()
        .normal()
        .streams()
        .pairs([(Name(b"On"), on_id), (Name(b"Off"), off_id)]);
    annot.finish();
}

fn write_radio_widget(
    pdf: &mut Pdf,
    field: &Field,
    widget_id: Ref,
    on_id: Ref,
    off_id: Ref,
    parent: Ref,
    page_id: Ref,
) {
    let rect = field.rect();
    write_radio_appearance(pdf, on_id, rect.w, rect.h, true);
    write_radio_appearance(pdf, off_id, rect.w, rect.h, false);
    let on_name = field.on_state().as_bytes();

    let mut widget = pdf.form_field(widget_id);
    widget.parent(parent);
    let mut annot = widget.into_annotation();
    annot
        .rect(Rect::new(rect.x, rect.y, rect.right(), rect.top()))
        .flags(AnnotationFlags::PRINT)
        .appearance_state(Name(b"Off"))
        .page(page_id);
    annot
        .appearance_characteristics()
        .background_color_rgb(1.0, 1.0, 1.0);
    annot
        .appearance()
        .normal()
        .streams()
        .pairs([(Name(on_name), on_id), (Name(b"Off"), off_id)]);
    annot.finish();
}

fn write_signature(
    pdf: &mut Pdf,
    field: &Field,
    widget_id: Ref,
    appearance_id: Ref,
    page_id: Ref,
    font_id: Ref,
) {
    let rect = field.rect();
    let bytes = labeled_box_appearance(rect.w, rect.h, field.caption());
    let mut xobject = pdf.form_xobject(appearance_id, &bytes);
    xobject.bbox(Rect::new(0.0, 0.0, rect.w, rect.h));
    xobject.resources().fonts().pair(Name(FONT_KEY), font_id);
    xobject.finish();

    let mut widget = pdf.form_field(widget_id);
    widget
        .field_type(FieldType::Signature)
        .partial_name(TextStr(field.name()));
    let mut annot = widget.into_annotation();
    annot
        .rect(Rect::new(rect.x, rect.y, rect.right(), rect.top()))
        .flags(AnnotationFlags::PRINT)
        .page(page_id);
    annot.border_style().width(1.0).style(BorderType::Solid);
    annot
        .appearance_characteristics()
        .border_color_rgb(0.0, 0.0, 0.0)
        .background_color_rgb(1.0, 1.0, 1.0);
    annot.appearance().normal().stream(appearance_id);
    annot.finish();
}

fn write_dropdown(
    pdf: &mut Pdf,
    field: &Field,
    widget_id: Ref,
    appearance_id: Ref,
    page_id: Ref,
    font_id: Ref,
) {
    let rect = field.rect();
    let bytes = dropdown_appearance(rect.w, rect.h);
    let mut xobject = pdf.form_xobject(appearance_id, &bytes);
    xobject.bbox(Rect::new(0.0, 0.0, rect.w, rect.h));
    xobject.resources().fonts().pair(Name(FONT_KEY), font_id);
    xobject.finish();

    let mut widget = pdf.form_field(widget_id);
    widget
        .field_type(FieldType::Choice)
        .field_flags(FieldFlags::COMBO)
        .partial_name(TextStr(field.name()))
        .vartext_default_appearance(Str(DA))
        .vartext_quadding(Quadding::Left)
        .choice_value(None);
    widget.choice_options().options(
        field
            .options()
            .iter()
            .map(|option| TextStr(option.as_str())),
    );
    {
        let mut resources = widget.insert(Name(b"DR")).dict();
        resources
            .insert(Name(b"Font"))
            .dict()
            .pair(Name(FONT_KEY), font_id);
    }
    let mut annot = widget.into_annotation();
    annot
        .rect(Rect::new(rect.x, rect.y, rect.right(), rect.top()))
        .flags(AnnotationFlags::PRINT)
        .page(page_id);
    annot.border_style().width(1.0).style(BorderType::Solid);
    annot
        .appearance_characteristics()
        .border_color_rgb(0.0, 0.0, 0.0)
        .background_color_rgb(1.0, 1.0, 1.0);
    annot.appearance().normal().stream(appearance_id);
    annot.finish();
}

fn write_box_appearance(pdf: &mut Pdf, id: Ref, w: f32, h: f32, checked: bool) {
    let mut content = Content::new();
    content.set_fill_gray(1.0);
    content.rect(0.0, 0.0, w, h);
    content.fill_nonzero();
    content.set_stroke_gray(0.0);
    content.set_line_width(1.0);
    let inset = 0.75;
    content.rect(
        inset,
        inset,
        (w - inset * 2.0).max(0.5),
        (h - inset * 2.0).max(0.5),
    );
    content.stroke();
    if checked {
        content.set_line_width((w.min(h) * 0.12).max(1.0));
        content.move_to(w * 0.22, h * 0.52);
        content.line_to(w * 0.42, h * 0.28);
        content.line_to(w * 0.78, h * 0.74);
        content.stroke();
    }
    let bytes = content.finish();
    let mut xobject = pdf.form_xobject(id, &bytes);
    xobject.bbox(Rect::new(0.0, 0.0, w, h));
    xobject.finish();
}

fn write_radio_appearance(pdf: &mut Pdf, id: Ref, w: f32, h: f32, on: bool) {
    let mut content = Content::new();
    content.set_fill_gray(1.0);
    content.rect(0.0, 0.0, w, h);
    content.fill_nonzero();
    content.set_stroke_gray(0.0);
    content.set_line_width(1.0);
    let radius = (w.min(h) * 0.5 - 1.25).max(1.0);
    stroke_circle(&mut content, w * 0.5, h * 0.5, radius);
    if on {
        content.set_fill_gray(0.0);
        fill_circle(&mut content, w * 0.5, h * 0.5, radius * 0.45);
    }
    let bytes = content.finish();
    let mut xobject = pdf.form_xobject(id, &bytes);
    xobject.bbox(Rect::new(0.0, 0.0, w, h));
    xobject.finish();
}

fn stroke_circle(content: &mut Content, cx: f32, cy: f32, radius: f32) {
    circle_path(content, cx, cy, radius);
    content.stroke();
}

fn fill_circle(content: &mut Content, cx: f32, cy: f32, radius: f32) {
    circle_path(content, cx, cy, radius);
    content.fill_nonzero();
}

fn circle_path(content: &mut Content, cx: f32, cy: f32, radius: f32) {
    let k = 0.552_284_8 * radius;
    content.move_to(cx + radius, cy);
    content.cubic_to(cx + radius, cy + k, cx + k, cy + radius, cx, cy + radius);
    content.cubic_to(cx - k, cy + radius, cx - radius, cy + k, cx - radius, cy);
    content.cubic_to(cx - radius, cy - k, cx - k, cy - radius, cx, cy - radius);
    content.cubic_to(cx + k, cy - radius, cx + radius, cy - k, cx + radius, cy);
    content.close_path();
}

fn text_appearance(w: f32, h: f32, hint: Option<&str>) -> Vec<u8> {
    let mut content = Content::new();
    content.set_fill_gray(1.0);
    content.rect(0.0, 0.0, w, h);
    content.fill_nonzero();
    content.set_stroke_gray(0.0);
    content.set_line_width(1.0);
    content.rect(0.5, 0.5, (w - 1.0).max(0.5), (h - 1.0).max(0.5));
    content.stroke();
    content.begin_text();
    content.set_font(Name(FONT_KEY), 12.0);
    content.next_line(4.0, ((h - 12.0) * 0.5).max(2.0));
    if let Some(hint) = hint {
        content.set_fill_gray(0.45);
        content.show(Str(&winansi(hint)));
    } else {
        content.set_fill_gray(0.0);
    }
    content.end_text();
    content.finish().to_vec()
}

fn labeled_box_appearance(w: f32, h: f32, label: &str) -> Vec<u8> {
    let mut content = Content::new();
    content.set_fill_gray(1.0);
    content.rect(0.0, 0.0, w, h);
    content.fill_nonzero();
    content.set_stroke_gray(0.0);
    content.set_line_width(1.0);
    content.rect(0.5, 0.5, (w - 1.0).max(0.5), (h - 1.0).max(0.5));
    content.stroke();
    content.move_to(6.0, 8.0);
    content.line_to((w - 6.0).max(8.0), 8.0);
    content.stroke();
    content.begin_text();
    content.set_fill_gray(0.35);
    content.set_font(Name(FONT_KEY), 10.0);
    content.next_line(6.0, (h * 0.55).max(12.0));
    content.show(Str(&winansi(label)));
    content.end_text();
    content.finish().to_vec()
}

fn dropdown_appearance(w: f32, h: f32) -> Vec<u8> {
    let mut content = Content::new();
    content.set_fill_gray(1.0);
    content.rect(0.0, 0.0, w, h);
    content.fill_nonzero();
    content.set_stroke_gray(0.0);
    content.set_line_width(1.0);
    content.rect(0.5, 0.5, (w - 1.0).max(0.5), (h - 1.0).max(0.5));
    content.stroke();
    let mid = h * 0.5;
    let tip_x = (w - 8.0).max(4.0);
    content.move_to(tip_x - 4.0, mid + 2.0);
    content.line_to(tip_x, mid - 2.0);
    content.line_to(tip_x + 4.0, mid + 2.0);
    content.stroke();
    content.finish().to_vec()
}

fn page_content(doc: &Document) -> (Vec<u8>, bool) {
    let mut content = Content::new();
    content.save_state();
    let mut drew_text = false;
    for field in doc.fields() {
        match field.kind() {
            FieldKind::StaticText if !field.caption().is_empty() => {
                show_line(
                    &mut content,
                    field.rect().x + 2.0,
                    text_baseline(field.rect().y, field.rect().h),
                    field.caption(),
                );
                drew_text = true;
            }
            FieldKind::Table => stroke_table(&mut content, field.rect()),
            FieldKind::Line => {
                if let Some((start, end)) = field.line_ends() {
                    content.set_stroke_gray(0.0);
                    content.set_line_width(1.0);
                    content.move_to(start.x, start.y);
                    content.line_to(end.x, end.y);
                    content.stroke();
                }
            }
            FieldKind::Rectangle | FieldKind::Image | FieldKind::TextBox => {
                let rect = field.rect();
                content.set_stroke_gray(0.0);
                content.set_line_width(1.0);
                content.rect(rect.x, rect.y, rect.w, rect.h);
                content.stroke();
                if field.kind() == FieldKind::Image {
                    content.move_to(rect.x, rect.y);
                    content.line_to(rect.right(), rect.top());
                    content.stroke();
                    content.move_to(rect.x, rect.top());
                    content.line_to(rect.right(), rect.y);
                    content.stroke();
                }
                let words = if field.kind() == FieldKind::TextBox {
                    wrap_caption(field.caption(), rect.w)
                } else if field.kind() == FieldKind::Image {
                    vec![field.caption().to_string()]
                } else {
                    Vec::new()
                };
                if words.iter().any(|line| !line.is_empty()) {
                    let mut y = rect.top() - 14.0;
                    for line in words {
                        if y < rect.y {
                            break;
                        }
                        show_line(&mut content, rect.x + 4.0, y, &line);
                        y -= 14.0;
                        drew_text = true;
                    }
                }
            }
            FieldKind::Radio if doc.radio_labels() && !field.caption().is_empty() => {
                let rect = field.rect();
                show_line(
                    &mut content,
                    rect.right() + 4.0,
                    text_baseline(rect.y, rect.h),
                    field.caption(),
                );
                drew_text = true;
            }
            _ => {}
        }
    }
    content.restore_state();
    if content.len() <= 4 {
        return (b"q\nQ\n".to_vec(), false);
    }
    (content.finish().to_vec(), drew_text)
}

fn show_line(content: &mut Content, x: f32, y: f32, text: &str) {
    content.begin_text();
    content.set_fill_gray(0.0);
    content.set_font(Name(FONT_KEY), 12.0);
    content.next_line(x, y);
    content.show(Str(&winansi(text)));
    content.end_text();
}

fn text_baseline(bottom: f32, height: f32) -> f32 {
    bottom + ((height - 12.0) * 0.5).max(1.0)
}

fn stroke_table(content: &mut Content, rect: crate::geom::RectPt) {
    content.set_stroke_gray(0.2);
    content.set_line_width(0.8);
    content.rect(rect.x, rect.y, rect.w, rect.h);
    content.stroke();
    for column in 1..TABLE_COLUMNS {
        let x = rect.x + rect.w * (column as f32) / (TABLE_COLUMNS as f32);
        content.move_to(x, rect.y);
        content.line_to(x, rect.top());
        content.stroke();
    }
    for row in 1..TABLE_ROWS {
        let y = rect.y + rect.h * (row as f32) / (TABLE_ROWS as f32);
        content.move_to(rect.x, y);
        content.line_to(rect.right(), y);
        content.stroke();
    }
}

fn wrap_caption(text: &str, width: f32) -> Vec<String> {
    let max_chars = ((width - 8.0) / 6.0).floor().max(1.0) as usize;
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let next_len = if current.is_empty() {
            word.len()
        } else {
            current.len() + 1 + word.len()
        };
        if next_len > max_chars && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
        if word.len() > max_chars {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            let mut rest = word;
            while rest.len() > max_chars {
                let (head, tail) = rest.split_at(max_chars);
                lines.push(head.to_string());
                rest = tail;
            }
            current = rest.to_string();
        } else if current.is_empty() {
            current = word.to_string();
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn winansi(text: &str) -> Vec<u8> {
    text.chars()
        .map(|ch| {
            if ch.is_ascii() && !ch.is_control() {
                ch as u8
            } else {
                b'?'
            }
        })
        .collect()
}
