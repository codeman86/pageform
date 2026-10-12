//! Fillable AcroForm export.
//!
//! Text fields get a real `/DA` and a normal appearance stream. Checkboxes get
//! explicit `/AP /N` entries named `/On` and `/Off`. Radio buttons in one
//! group share a parent field (`/FT /Btn`, radio flag) and each widget has
//! its own on-state name. Drop-downs are combo boxes (`/Ff` combo), not list
//! boxes. Dates are text fields with `/MaxLen` and an alternate name.
//! Signature fields are unsigned `/FT /Sig` widgets. Properties such as
//! `/TU`, `/Ff`, `/Q`, `/MaxLen`, `/V`, `/DV`, `/Opt`, `/MK`, and `/BS`
//! follow the field style. Static text, lines, and shapes are page content.
//! `NeedAppearances` is never set.
//!
//! Header text, footer text, margins, the grid, and guide lines stay on the
//! canvas and are not written into the file.

use pdf_writer::types::{AnnotationFlags, BorderType, FieldFlags, FieldType, Quadding};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

use crate::document::{Document, Field, FieldKind, TABLE_COLUMNS, TABLE_ROWS};
use crate::style::{BorderStyleKind, FieldStyle, FontFamily, Rgb, TextAlign};

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
    selected: String,
    required: bool,
    read_only: bool,
}

struct FontMap {
    ids: Vec<(FontFamily, Ref)>,
}

impl FontMap {
    fn id(&self, family: FontFamily) -> Ref {
        self.ids
            .iter()
            .find(|(stored, _)| *stored == family)
            .map(|(_, id)| *id)
            .unwrap_or_else(|| self.ids[0].1)
    }
}

/// Serialize `doc` to a single-page fillable PDF.
pub fn export_pdf(doc: &Document) -> Result<Vec<u8>, ExportError> {
    validate(doc)?;

    let mut pdf = Pdf::new();
    let mut ids = Ids { next: 1 };
    let families = used_fonts(doc);
    let fonts = FontMap {
        ids: families
            .into_iter()
            .map(|family| (family, ids.next()))
            .collect(),
    };
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
            FieldKind::Text | FieldKind::Date => {
                let widget_id = ids.next();
                let appearance_id = ids.next();
                write_text_field(&mut pdf, field, widget_id, appearance_id, page_id, &fonts);
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
                    if field.style().checked() {
                        group.selected = field.on_state().to_string();
                    }
                    group.required |= field.style().required;
                    group.read_only |= field.style().read_only;
                    group.parent
                } else {
                    let parent = ids.next();
                    groups.push(RadioGroup {
                        name: field.name().to_string(),
                        parent,
                        kids: vec![widget_id],
                        selected: if field.style().checked() {
                            field.on_state().to_string()
                        } else {
                            "Off".to_string()
                        },
                        required: field.style().required,
                        read_only: field.style().read_only,
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
                write_signature(&mut pdf, field, widget_id, appearance_id, page_id, &fonts);
                annots.push(widget_id);
                roots.push(widget_id);
            }
            FieldKind::Dropdown => {
                let widget_id = ids.next();
                let appearance_id = ids.next();
                write_dropdown(&mut pdf, field, widget_id, appearance_id, page_id, &fonts);
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
        let mut flags = FieldFlags::RADIO;
        if group.required {
            flags |= FieldFlags::REQUIRED;
        }
        if group.read_only {
            flags |= FieldFlags::READ_ONLY;
        }
        let mut field = pdf.form_field(group.parent);
        field
            .field_type(FieldType::Button)
            .field_flags(flags)
            .partial_name(TextStr(&group.name))
            .children(group.kids.iter().copied())
            .radio_value(Name(group.selected.as_bytes()))
            .radio_default_value(Name(group.selected.as_bytes()));
        field.finish();
    }

    let (content, content_fonts) = page_content(doc);
    pdf.stream(contents_id, &content);

    let page = doc.page();
    let mut page_writer = pdf.page(page_id);
    page_writer
        .media_box(Rect::new(0.0, 0.0, page.width, page.height))
        .parent(pages_id)
        .contents(contents_id);
    if !content_fonts.is_empty() {
        let mut resources = page_writer.resources();
        let mut page_fonts = resources.fonts();
        for family in &content_fonts {
            page_fonts.pair(Name(family.pdf_key()), fonts.id(*family));
        }
    }
    if !annots.is_empty() {
        page_writer.annotations(annots.iter().copied());
    }
    page_writer.finish();

    pdf.pages(pages_id).count(1).kids([page_id]);

    for (family, id) in &fonts.ids {
        pdf.type1_font(*id)
            .base_font(Name(family.base_font()))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
    }

    let mut catalog = pdf.catalog(catalog_id);
    catalog.pages(pages_id);
    {
        let mut form = catalog.form();
        form.fields(roots.iter().copied())
            .default_appearance(Str(DA));
        let mut resources = form.default_resources();
        let mut form_fonts = resources.fonts();
        for (family, id) in &fonts.ids {
            form_fonts.pair(Name(family.pdf_key()), *id);
        }
    }
    catalog.finish();

    Ok(pdf.finish())
}

fn used_fonts(doc: &Document) -> Vec<FontFamily> {
    let mut fonts = vec![FontFamily::Helvetica];
    for field in doc.fields() {
        let font = field.style().font;
        if !fonts.contains(&font) {
            fonts.push(font);
        }
    }
    fonts
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
    fonts: &FontMap,
) {
    let style = field.style();
    let rect = field.rect();
    let shown = appearance_text(field);
    let bytes = text_appearance(rect.w, rect.h, style, shown.as_deref());
    let mut xobject = pdf.form_xobject(appearance_id, &bytes);
    xobject.bbox(Rect::new(0.0, 0.0, rect.w, rect.h));
    xobject
        .resources()
        .fonts()
        .pair(Name(style.font.pdf_key()), fonts.id(style.font));
    xobject.finish();

    let da = da_string(style);
    let mut widget = pdf.form_field(widget_id);
    widget
        .field_type(FieldType::Text)
        .partial_name(TextStr(field.name()))
        .text_value(TextStr(style.default_value.as_str()))
        .vartext_default_appearance(Str(da.as_bytes()))
        .vartext_quadding(quadding(style.align));
    if !style.default_value.is_empty() {
        widget.text_default_value(TextStr(style.default_value.as_str()));
    }
    let flags = text_flags(style);
    if !flags.is_empty() {
        widget.field_flags(flags);
    }
    if let Some(max_len) = style.max_chars {
        widget.text_max_len(max_len);
    }
    if let Some(alternate) = alternate_name(field) {
        widget.alternate_name(TextStr(alternate));
    }
    widget_font(&mut widget, style.font, fonts.id(style.font));
    let mut annot = widget.into_annotation();
    annot
        .rect(Rect::new(rect.x, rect.y, rect.right(), rect.top()))
        .flags(AnnotationFlags::PRINT)
        .page(page_id);
    apply_border(&mut annot, style);
    apply_colors(&mut annot, style, None);
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
    let style = field.style();
    let rect = field.rect();
    write_box_appearance(pdf, on_id, rect.w, rect.h, style, true);
    write_box_appearance(pdf, off_id, rect.w, rect.h, style, false);
    let state: &[u8] = if style.checked() { b"On" } else { b"Off" };

    let mut widget = pdf.form_field(widget_id);
    widget
        .field_type(FieldType::Button)
        .field_flags(identity_flags(style))
        .partial_name(TextStr(field.name()))
        .pair(Name(b"V"), Name(state))
        .pair(Name(b"DV"), Name(state));
    if let Some(alternate) = alternate_name(field) {
        widget.alternate_name(TextStr(alternate));
    }
    let mut annot = widget.into_annotation();
    annot
        .rect(Rect::new(rect.x, rect.y, rect.right(), rect.top()))
        .flags(AnnotationFlags::PRINT)
        .appearance_state(Name(state))
        .page(page_id);
    apply_border(&mut annot, style);
    apply_colors(&mut annot, style, Some(style.checkbox_style.zapf()));
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
    let style = field.style();
    let rect = field.rect();
    write_radio_appearance(pdf, on_id, rect.w, rect.h, style, true);
    write_radio_appearance(pdf, off_id, rect.w, rect.h, style, false);
    let on_name = field.on_state().as_bytes();
    let state: &[u8] = if style.checked() { on_name } else { b"Off" };

    let mut widget = pdf.form_field(widget_id);
    widget.parent(parent);
    let mut annot = widget.into_annotation();
    annot
        .rect(Rect::new(rect.x, rect.y, rect.right(), rect.top()))
        .flags(AnnotationFlags::PRINT)
        .appearance_state(Name(state))
        .page(page_id);
    apply_border(&mut annot, style);
    apply_colors(&mut annot, style, None);
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
    fonts: &FontMap,
) {
    let style = field.style();
    let rect = field.rect();
    let bytes = labeled_box_appearance(rect.w, rect.h, field.caption(), style);
    let mut xobject = pdf.form_xobject(appearance_id, &bytes);
    xobject.bbox(Rect::new(0.0, 0.0, rect.w, rect.h));
    xobject
        .resources()
        .fonts()
        .pair(Name(style.font.pdf_key()), fonts.id(style.font));
    xobject.finish();

    let mut widget = pdf.form_field(widget_id);
    widget
        .field_type(FieldType::Signature)
        .partial_name(TextStr(field.name()));
    let flags = identity_flags(style);
    if !flags.is_empty() {
        widget.field_flags(flags);
    }
    if let Some(alternate) = alternate_name(field) {
        widget.alternate_name(TextStr(alternate));
    }
    let mut annot = widget.into_annotation();
    annot
        .rect(Rect::new(rect.x, rect.y, rect.right(), rect.top()))
        .flags(AnnotationFlags::PRINT)
        .page(page_id);
    apply_border(&mut annot, style);
    apply_colors(&mut annot, style, None);
    annot.appearance().normal().stream(appearance_id);
    annot.finish();
}

fn write_dropdown(
    pdf: &mut Pdf,
    field: &Field,
    widget_id: Ref,
    appearance_id: Ref,
    page_id: Ref,
    fonts: &FontMap,
) {
    let style = field.style();
    let rect = field.rect();
    let chosen = field
        .options()
        .iter()
        .find(|option| *option == &style.default_value)
        .map(String::as_str);
    let bytes = dropdown_appearance(rect.w, rect.h, style, chosen);
    let mut xobject = pdf.form_xobject(appearance_id, &bytes);
    xobject.bbox(Rect::new(0.0, 0.0, rect.w, rect.h));
    xobject
        .resources()
        .fonts()
        .pair(Name(style.font.pdf_key()), fonts.id(style.font));
    xobject.finish();

    let da = da_string(style);
    let mut widget = pdf.form_field(widget_id);
    widget
        .field_type(FieldType::Choice)
        .field_flags(FieldFlags::COMBO | identity_flags(style))
        .partial_name(TextStr(field.name()))
        .vartext_default_appearance(Str(da.as_bytes()))
        .vartext_quadding(quadding(style.align));
    if let Some(value) = chosen {
        widget.choice_value(Some(TextStr(value)));
        widget.choice_default_value(Some(TextStr(value)));
    } else {
        widget.choice_value(None);
    }
    widget.choice_options().options(
        field
            .options()
            .iter()
            .map(|option| TextStr(option.as_str())),
    );
    if let Some(alternate) = alternate_name(field) {
        widget.alternate_name(TextStr(alternate));
    }
    widget_font(&mut widget, style.font, fonts.id(style.font));
    let mut annot = widget.into_annotation();
    annot
        .rect(Rect::new(rect.x, rect.y, rect.right(), rect.top()))
        .flags(AnnotationFlags::PRINT)
        .page(page_id);
    apply_border(&mut annot, style);
    apply_colors(&mut annot, style, None);
    annot.appearance().normal().stream(appearance_id);
    annot.finish();
}

fn widget_font(widget: &mut pdf_writer::writers::Field<'_>, font: FontFamily, id: Ref) {
    widget
        .insert(Name(b"DR"))
        .dict()
        .insert(Name(b"Font"))
        .dict()
        .pair(Name(font.pdf_key()), id);
}

fn apply_border(annot: &mut pdf_writer::writers::Annotation<'_>, style: &FieldStyle) {
    let mut border = annot.border_style();
    border
        .width(style.border_width)
        .style(border_type(style.border_style));
    if style.border_style == BorderStyleKind::Dashed {
        border.dashes([3.0, 2.0]);
    }
}

fn apply_colors(
    annot: &mut pdf_writer::writers::Annotation<'_>,
    style: &FieldStyle,
    caption: Option<&str>,
) {
    let mut mk = annot.appearance_characteristics();
    mk.border_color_rgb(
        style.border_color.r,
        style.border_color.g,
        style.border_color.b,
    );
    if let Some(fill) = style.fill {
        mk.background_color_rgb(fill.r, fill.g, fill.b);
    }
    if let Some(caption) = caption {
        mk.normal_caption(TextStr(caption));
    }
}

fn alternate_name(field: &Field) -> Option<&str> {
    if !field.style().tooltip.is_empty() {
        Some(field.style().tooltip.as_str())
    } else if field.kind() == FieldKind::Date && !field.style().date_format.is_empty() {
        Some(field.style().date_format.as_str())
    } else {
        None
    }
}

fn appearance_text(field: &Field) -> Option<String> {
    if !field.style().default_value.is_empty() {
        return Some(field.style().default_value.clone());
    }
    if field.kind() == FieldKind::Date {
        return Some(field.style().date_format.clone());
    }
    None
}

fn identity_flags(style: &FieldStyle) -> FieldFlags {
    let mut flags = FieldFlags::empty();
    if style.read_only {
        flags |= FieldFlags::READ_ONLY;
    }
    if style.required {
        flags |= FieldFlags::REQUIRED;
    }
    flags
}

fn text_flags(style: &FieldStyle) -> FieldFlags {
    let mut flags = identity_flags(style);
    if style.multiline {
        flags |= FieldFlags::MULTILINE;
    }
    flags
}

fn quadding(align: TextAlign) -> Quadding {
    match align {
        TextAlign::Left => Quadding::Left,
        TextAlign::Center => Quadding::Center,
        TextAlign::Right => Quadding::Right,
    }
}

fn border_type(style: BorderStyleKind) -> BorderType {
    match style {
        BorderStyleKind::Solid => BorderType::Solid,
        BorderStyleKind::Dashed => BorderType::Dashed,
        BorderStyleKind::Beveled => BorderType::Beveled,
        BorderStyleKind::Inset => BorderType::Inset,
        BorderStyleKind::Underline => BorderType::Underline,
    }
}

fn da_string(style: &FieldStyle) -> String {
    format!(
        "/{} {} Tf {}",
        style.font.pdf_key_str(),
        format_num(style.font_size),
        color_operand(style.text_color)
    )
}

fn color_operand(color: Rgb) -> String {
    if color.is_black() {
        "0 g".to_string()
    } else {
        format!(
            "{} {} {} rg",
            format_num(color.r),
            format_num(color.g),
            format_num(color.b)
        )
    }
}

fn format_num(value: f32) -> String {
    if (value - value.round()).abs() < 0.001 {
        format!("{}", value.round() as i32)
    } else {
        let text = format!("{value:.3}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn write_box_appearance(pdf: &mut Pdf, id: Ref, w: f32, h: f32, style: &FieldStyle, checked: bool) {
    let mut content = Content::new();
    paint_widget_box(&mut content, w, h, style);
    if checked {
        draw_checkbox_mark(&mut content, style, w, h);
    }
    let bytes = content.finish();
    let mut xobject = pdf.form_xobject(id, &bytes);
    xobject.bbox(Rect::new(0.0, 0.0, w, h));
    xobject.finish();
}

fn write_radio_appearance(pdf: &mut Pdf, id: Ref, w: f32, h: f32, style: &FieldStyle, on: bool) {
    let mut content = Content::new();
    if let Some(fill) = style.fill {
        set_fill(&mut content, fill);
        content.rect(0.0, 0.0, w, h);
        content.fill_nonzero();
    }
    set_stroke(&mut content, style.border_color);
    content.set_line_width(style.border_width.max(0.6));
    let radius = (w.min(h) * 0.5 - style.border_width - 0.4).max(1.0);
    stroke_circle(&mut content, w * 0.5, h * 0.5, radius);
    if on {
        set_fill(&mut content, style.text_color);
        fill_circle(&mut content, w * 0.5, h * 0.5, radius * 0.45);
    }
    let bytes = content.finish();
    let mut xobject = pdf.form_xobject(id, &bytes);
    xobject.bbox(Rect::new(0.0, 0.0, w, h));
    xobject.finish();
}

fn paint_widget_box(content: &mut Content, w: f32, h: f32, style: &FieldStyle) {
    if let Some(fill) = style.fill {
        set_fill(content, fill);
        content.rect(0.0, 0.0, w, h);
        content.fill_nonzero();
    }
    if style.border_width <= 0.05 {
        return;
    }
    set_stroke(content, style.border_color);
    content.set_line_width(style.border_width);
    if style.border_style == BorderStyleKind::Dashed {
        content.set_dash_pattern([3.0, 2.0], 0.0);
    }
    match style.border_style {
        BorderStyleKind::Underline => {
            let y = (style.border_width * 0.5).min(h * 0.5);
            content.move_to(0.0, y);
            content.line_to(w, y);
            content.stroke();
        }
        BorderStyleKind::Beveled | BorderStyleKind::Inset => {
            let inset = style.border_width * 0.5;
            content.rect(
                inset,
                inset,
                (w - style.border_width).max(0.4),
                (h - style.border_width).max(0.4),
            );
            content.stroke();
            let (light, dark) = if style.border_style == BorderStyleKind::Beveled {
                (0.9, 0.35)
            } else {
                (0.35, 0.9)
            };
            content.set_stroke_gray(light);
            content.move_to(inset, inset);
            content.line_to(inset, h - inset);
            content.line_to(w - inset, h - inset);
            content.stroke();
            content.set_stroke_gray(dark);
            content.move_to(inset, inset);
            content.line_to(w - inset, inset);
            content.line_to(w - inset, h - inset);
            content.stroke();
        }
        BorderStyleKind::Solid | BorderStyleKind::Dashed => {
            let inset = style.border_width * 0.5;
            content.rect(
                inset,
                inset,
                (w - style.border_width).max(0.4),
                (h - style.border_width).max(0.4),
            );
            content.stroke();
        }
    }
}

fn draw_checkbox_mark(content: &mut Content, style: &FieldStyle, w: f32, h: f32) {
    set_stroke(content, style.text_color);
    set_fill(content, style.text_color);
    let weight = (w.min(h) * 0.12).max(0.8);
    content.set_line_width(weight);
    let cx = w * 0.5;
    let cy = h * 0.5;
    let radius = w.min(h) * 0.28;
    match style.checkbox_style {
        crate::style::CheckboxStyle::Check => {
            content.move_to(w * 0.22, h * 0.52);
            content.line_to(w * 0.42, h * 0.28);
            content.line_to(w * 0.78, h * 0.74);
            content.stroke();
        }
        crate::style::CheckboxStyle::Cross => {
            content.move_to(w * 0.28, h * 0.28);
            content.line_to(w * 0.72, h * 0.72);
            content.stroke();
            content.move_to(w * 0.28, h * 0.72);
            content.line_to(w * 0.72, h * 0.28);
            content.stroke();
        }
        crate::style::CheckboxStyle::Circle => {
            fill_circle(content, cx, cy, radius * 0.75);
        }
        crate::style::CheckboxStyle::Diamond => {
            content.move_to(cx, cy + radius);
            content.line_to(cx + radius, cy);
            content.line_to(cx, cy - radius);
            content.line_to(cx - radius, cy);
            content.close_path();
            content.stroke();
        }
        crate::style::CheckboxStyle::Square => {
            let side = radius * 1.4;
            content.rect(cx - side * 0.5, cy - side * 0.5, side, side);
            content.stroke();
        }
        crate::style::CheckboxStyle::Star => {
            stroke_star(content, cx, cy, radius);
        }
    }
}

fn stroke_star(content: &mut Content, cx: f32, cy: f32, radius: f32) {
    let mut points = [(0.0, 0.0); 5];
    for (index, point) in points.iter_mut().enumerate() {
        let angle = -std::f32::consts::FRAC_PI_2 + index as f32 * std::f32::consts::TAU / 5.0;
        *point = (cx + radius * angle.cos(), cy + radius * angle.sin());
    }
    content.move_to(points[0].0, points[0].1);
    for index in 0..5 {
        let point = points[(index * 2) % 5];
        content.line_to(point.0, point.1);
    }
    content.close_path();
    content.stroke();
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

fn text_appearance(w: f32, h: f32, style: &FieldStyle, shown: Option<&str>) -> Vec<u8> {
    let mut content = Content::new();
    paint_widget_box(&mut content, w, h, style);
    if let Some(shown) = shown {
        let hint = style.default_value.is_empty();
        paint_lines(&mut content, w, h, shown, style, hint);
    }
    content.finish().to_vec()
}

fn labeled_box_appearance(w: f32, h: f32, label: &str, style: &FieldStyle) -> Vec<u8> {
    let mut content = Content::new();
    paint_widget_box(&mut content, w, h, style);
    content.set_stroke_gray(0.45);
    content.set_line_width(0.8);
    content.move_to(6.0, 8.0);
    content.line_to((w - 6.0).max(8.0), 8.0);
    content.stroke();
    if !label.is_empty() {
        paint_lines(&mut content, w, h, label, style, true);
    }
    content.finish().to_vec()
}

fn dropdown_appearance(w: f32, h: f32, style: &FieldStyle, value: Option<&str>) -> Vec<u8> {
    let mut content = Content::new();
    paint_widget_box(&mut content, w, h, style);
    set_stroke(&mut content, style.border_color);
    content.set_line_width(1.0);
    let mid = h * 0.5;
    let tip_x = (w - 8.0).max(4.0);
    content.move_to(tip_x - 4.0, mid + 2.0);
    content.line_to(tip_x, mid - 2.0);
    content.line_to(tip_x + 4.0, mid + 2.0);
    content.stroke();
    if let Some(value) = value {
        paint_lines(&mut content, w - 16.0, h, value, style, false);
    }
    content.finish().to_vec()
}

fn paint_lines(content: &mut Content, w: f32, h: f32, text: &str, style: &FieldStyle, hint: bool) {
    if text.is_empty() {
        return;
    }
    let size = style.font_size.clamp(4.0, 96.0);
    let lines = if style.multiline {
        wrap_text(text, w, style.font, size)
    } else {
        vec![text.replace('\n', " ")]
    };
    let leading = size * 1.2;
    let mut y = if lines.len() == 1 {
        ((h - size) * 0.5).max(1.5)
    } else {
        (h - size - 2.0).max(1.5)
    };
    for line in &lines {
        if y < 0.5 {
            break;
        }
        let width = approx_text_width(line, style.font, size);
        let x = match style.align {
            TextAlign::Left => 4.0,
            TextAlign::Center => ((w - width) * 0.5).max(2.0),
            TextAlign::Right => (w - 4.0 - width).max(2.0),
        };
        content.begin_text();
        if hint {
            content.set_fill_gray(0.45);
        } else {
            set_fill(content, style.text_color);
        }
        content.set_font(Name(style.font.pdf_key()), size);
        content.next_line(x, y);
        content.show(Str(&winansi(line)));
        content.end_text();
        y -= leading;
    }
}

fn set_fill(content: &mut Content, color: Rgb) {
    if color.is_black() {
        content.set_fill_gray(0.0);
    } else if color.r >= 0.999 && color.g >= 0.999 && color.b >= 0.999 {
        content.set_fill_gray(1.0);
    } else {
        content.set_fill_rgb(color.r, color.g, color.b);
    }
}

fn set_stroke(content: &mut Content, color: Rgb) {
    if color.is_black() {
        content.set_stroke_gray(0.0);
    } else {
        content.set_stroke_rgb(color.r, color.g, color.b);
    }
}

fn page_content(doc: &Document) -> (Vec<u8>, Vec<FontFamily>) {
    let mut content = Content::new();
    content.save_state();
    let mut fonts = Vec::new();
    for field in doc.fields() {
        let style = field.style();
        match field.kind() {
            FieldKind::StaticText if !field.caption().is_empty() => {
                paint_page_text(
                    &mut content,
                    field.rect(),
                    field.caption(),
                    style,
                    false,
                    &mut fonts,
                );
                if style.border_width > 0.05 {
                    stroke_page_box(&mut content, field.rect(), style, true);
                }
            }
            FieldKind::Table => stroke_table(&mut content, field.rect(), style),
            FieldKind::Line => {
                if let Some((start, end)) = field.line_ends() {
                    if style.stroke_width > 0.05 {
                        content.save_state();
                        set_stroke(&mut content, style.stroke_color);
                        content.set_line_width(style.stroke_width);
                        if style.border_style == BorderStyleKind::Dashed {
                            content.set_dash_pattern([3.0, 2.0], 0.0);
                        }
                        content.move_to(start.x, start.y);
                        content.line_to(end.x, end.y);
                        content.stroke();
                        content.restore_state();
                    }
                }
            }
            FieldKind::Rectangle | FieldKind::Image | FieldKind::TextBox => {
                let rect = field.rect();
                content.save_state();
                if let Some(fill) = style.fill {
                    set_fill(&mut content, fill);
                    content.rect(rect.x, rect.y, rect.w, rect.h);
                    content.fill_nonzero();
                }
                let width = if field.kind() == FieldKind::TextBox {
                    style.border_width
                } else {
                    style.stroke_width
                };
                let color = if field.kind() == FieldKind::TextBox {
                    style.border_color
                } else {
                    style.stroke_color
                };
                if width > 0.05 {
                    set_stroke(&mut content, color);
                    content.set_line_width(width);
                    if style.border_style == BorderStyleKind::Dashed {
                        content.set_dash_pattern([3.0, 2.0], 0.0);
                    }
                    if style.border_style == BorderStyleKind::Underline {
                        content.move_to(rect.x, rect.y);
                        content.line_to(rect.right(), rect.y);
                    } else {
                        content.rect(rect.x, rect.y, rect.w, rect.h);
                    }
                    content.stroke();
                }
                if field.kind() == FieldKind::Image {
                    set_stroke(&mut content, color);
                    content.set_line_width(width.max(0.6));
                    content.move_to(rect.x, rect.y);
                    content.line_to(rect.right(), rect.top());
                    content.stroke();
                    content.move_to(rect.x, rect.top());
                    content.line_to(rect.right(), rect.y);
                    content.stroke();
                }
                content.restore_state();
                let words = if field.kind() == FieldKind::TextBox {
                    wrap_text(field.caption(), rect.w, style.font, style.font_size)
                } else if field.kind() == FieldKind::Image && !field.caption().is_empty() {
                    vec![field.caption().to_string()]
                } else {
                    Vec::new()
                };
                if words.iter().any(|line| !line.is_empty()) {
                    remember_font(&mut fonts, style.font);
                    let leading = style.font_size.max(8.0) * 1.15;
                    let mut y = rect.top() - style.font_size.max(8.0);
                    for line in words {
                        if y < rect.y || line.is_empty() {
                            break;
                        }
                        let x = aligned_x(rect.x, rect.w, &line, style, 4.0);
                        show_text(
                            &mut content,
                            x,
                            y,
                            &line,
                            style.font,
                            style.font_size.max(8.0),
                            style.text_color,
                        );
                        y -= leading;
                    }
                }
            }
            FieldKind::Radio if doc.radio_labels() && !field.caption().is_empty() => {
                let rect = field.rect();
                remember_font(&mut fonts, style.font);
                let x = rect.right() + 4.0;
                let y = rect.y + ((rect.h - style.font_size) * 0.5).max(1.0);
                show_text(
                    &mut content,
                    x,
                    y,
                    field.caption(),
                    style.font,
                    style.font_size,
                    style.text_color,
                );
            }
            _ => {}
        }
    }
    content.restore_state();
    if content.len() <= 4 {
        return (b"q\nQ\n".to_vec(), Vec::new());
    }
    (content.finish().to_vec(), fonts)
}

fn stroke_page_box(
    content: &mut Content,
    rect: crate::geom::RectPt,
    style: &FieldStyle,
    border: bool,
) {
    let width = if border {
        style.border_width
    } else {
        style.stroke_width
    };
    if width <= 0.05 {
        return;
    }
    let color = if border {
        style.border_color
    } else {
        style.stroke_color
    };
    content.save_state();
    set_stroke(content, color);
    content.set_line_width(width);
    content.rect(rect.x, rect.y, rect.w, rect.h);
    content.stroke();
    content.restore_state();
}

fn paint_page_text(
    content: &mut Content,
    rect: crate::geom::RectPt,
    text: &str,
    style: &FieldStyle,
    multiline: bool,
    fonts: &mut Vec<FontFamily>,
) {
    remember_font(fonts, style.font);
    let lines = if multiline {
        wrap_text(text, rect.w, style.font, style.font_size)
    } else {
        vec![text.to_string()]
    };
    let y = rect.y + ((rect.h - style.font_size) * 0.5).max(1.0);
    for line in lines.iter().take(1) {
        let x = aligned_x(rect.x, rect.w, line, style, 2.0);
        show_text(
            content,
            x,
            y,
            line,
            style.font,
            style.font_size,
            style.text_color,
        );
    }
}

fn aligned_x(left: f32, width: f32, text: &str, style: &FieldStyle, pad: f32) -> f32 {
    let text_width = approx_text_width(text, style.font, style.font_size);
    match style.align {
        TextAlign::Left => left + pad,
        TextAlign::Center => left + ((width - text_width) * 0.5).max(0.0),
        TextAlign::Right => left + (width - pad - text_width).max(0.0),
    }
}

fn show_text(
    content: &mut Content,
    x: f32,
    y: f32,
    text: &str,
    font: FontFamily,
    size: f32,
    color: Rgb,
) {
    content.begin_text();
    set_fill(content, color);
    content.set_font(Name(font.pdf_key()), size);
    content.next_line(x, y);
    content.show(Str(&winansi(text)));
    content.end_text();
}

fn remember_font(fonts: &mut Vec<FontFamily>, font: FontFamily) {
    if !fonts.contains(&font) {
        fonts.push(font);
    }
}

fn approx_text_width(text: &str, font: FontFamily, size: f32) -> f32 {
    let factor = match font {
        FontFamily::Courier => 0.6,
        FontFamily::Times => 0.45,
        FontFamily::Helvetica => 0.5,
    };
    text.chars().count() as f32 * size * factor
}

fn stroke_table(content: &mut Content, rect: crate::geom::RectPt, style: &FieldStyle) {
    content.save_state();
    if let Some(fill) = style.fill {
        set_fill(content, fill);
        content.rect(rect.x, rect.y, rect.w, rect.h);
        content.fill_nonzero();
    }
    set_stroke(content, style.stroke_color);
    content.set_line_width(style.stroke_width.max(0.4));
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
    content.restore_state();
}

fn wrap_text(text: &str, width: f32, font: FontFamily, size: f32) -> Vec<String> {
    let char_width = match font {
        FontFamily::Courier => size * 0.6,
        FontFamily::Times => size * 0.45,
        FontFamily::Helvetica => size * 0.5,
    }
    .max(1.0);
    let max_chars = ((width - 8.0) / char_width).floor().max(1.0) as usize;
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
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn winansi(text: &str) -> Vec<u8> {
    text.chars()
        .map(|ch| {
            if ch.is_ascii() && !ch.is_control() {
                ch as u8
            } else if ch == '\n' {
                b' '
            } else {
                b'?'
            }
        })
        .collect()
}
