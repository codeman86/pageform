//! Fillable AcroForm export.
//!
//! Text fields get a real `/DA` (`/Helv 12 Tf 0 g`) and a normal appearance
//! stream. Checkboxes get explicit `/AP /N` entries named `/On` and `/Off`.
//! `NeedAppearances` is never set.

use pdf_writer::types::{AnnotationFlags, BorderType, FieldFlags, FieldType, Quadding};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

use crate::document::{Document, Field, FieldKind};

const FONT_KEY: &[u8] = b"Helv";
const DA: &[u8] = b"/Helv 12 Tf 0 g";

#[derive(Debug)]
pub enum ExportError {
    EmptyName,
    DuplicateName(String),
    InvalidRect { name: String },
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::EmptyName => write!(f, "A field is missing a name"),
            ExportError::DuplicateName(name) => write!(f, "Duplicate field name \"{name}\""),
            ExportError::InvalidRect { name } => {
                write!(f, "Field \"{name}\" has a zero or invalid rectangle")
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

    let mut widgets = Vec::with_capacity(doc.fields().len());
    for field in doc.fields() {
        let widget_id = ids.next();
        match field.kind() {
            FieldKind::Text => {
                let appearance_id = ids.next();
                write_text_field(&mut pdf, field, widget_id, appearance_id, page_id, font_id);
            }
            FieldKind::Checkbox => {
                let on_id = ids.next();
                let off_id = ids.next();
                write_checkbox(&mut pdf, field, widget_id, on_id, off_id, page_id);
            }
        }
        widgets.push(widget_id);
    }

    let page = doc.page();
    pdf.stream(contents_id, b"q\nQ\n");

    let mut page_writer = pdf.page(page_id);
    page_writer
        .media_box(Rect::new(0.0, 0.0, page.width, page.height))
        .parent(pages_id)
        .contents(contents_id);
    if !widgets.is_empty() {
        page_writer.annotations(widgets.iter().copied());
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
        form.fields(widgets.iter().copied())
            .default_appearance(Str(DA));
        form.default_resources()
            .fonts()
            .pair(Name(FONT_KEY), font_id);
    }
    catalog.finish();

    Ok(pdf.finish())
}

fn validate(doc: &Document) -> Result<(), ExportError> {
    let mut seen = Vec::new();
    for field in doc.fields() {
        let name = field.name();
        if name.trim().is_empty() {
            return Err(ExportError::EmptyName);
        }
        if seen.iter().any(|existing: &String| existing == name) {
            return Err(ExportError::DuplicateName(name.to_string()));
        }
        let rect = field.rect();
        if !rect.w.is_finite() || !rect.h.is_finite() || rect.w < 1.0 || rect.h < 1.0 {
            return Err(ExportError::InvalidRect {
                name: name.to_string(),
            });
        }
        seen.push(name.to_string());
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
) {
    let rect = field.rect();
    let bytes = text_appearance(rect.w, rect.h);
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

fn text_appearance(w: f32, h: f32) -> Vec<u8> {
    let mut content = Content::new();
    content.set_fill_gray(1.0);
    content.rect(0.0, 0.0, w, h);
    content.fill_nonzero();
    content.set_stroke_gray(0.0);
    content.set_line_width(1.0);
    content.rect(0.5, 0.5, (w - 1.0).max(0.5), (h - 1.0).max(0.5));
    content.stroke();
    content.begin_text();
    content.set_fill_gray(0.0);
    content.set_font(Name(FONT_KEY), 12.0);
    content.next_line(4.0, ((h - 12.0) * 0.5).max(2.0));
    content.end_text();
    content.finish().to_vec()
}
