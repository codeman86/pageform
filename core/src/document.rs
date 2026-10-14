//! Single-page form document.
//!
//! Milestone 0 stores one US Letter page. A page list, page navigation, and
//! per-page fields belong to Milestone 1 and are not modeled here.
//!
//! Static text and the table placeholder are canvas objects. They are not
//! AcroForm fields. Radio buttons that share a name are one radio group.

use crate::geom::{
    align_rects, clamp_rect, distribute_rects, inset_rect, match_size, AlignEdge, Page, PdfPoint,
    RectPt,
};
use crate::grid::GridSize;
use crate::style::{clean_text, FieldStyle, Margins, Orientation, PageSize, Rgb};

pub const SAMPLE_TEXT_NAME: &str = "full_name";
pub const SAMPLE_CHECK_NAME: &str = "agree";

/// Shown on an empty date field. The exported value stays blank.
pub const DATE_HINT: &str = "YYYY-MM-DD";

/// Placeholder grid. Cells are not separate fields.
pub const TABLE_COLUMNS: u32 = 3;
pub const TABLE_ROWS: u32 = 2;

/// Medium text default, 1 inch from the top-left.
/// Bottom-left PDF rect → `/Rect [72 702 216 720]`.
pub const SAMPLE_TEXT_RECT: RectPt = RectPt {
    x: 72.0,
    y: 702.0,
    w: 144.0,
    h: 18.0,
};

/// Medium checkbox default, one row below the sample text field.
/// Bottom-left PDF rect → `/Rect [72 666 90 684]`.
pub const SAMPLE_CHECK_RECT: RectPt = RectPt {
    x: 72.0,
    y: 666.0,
    w: 18.0,
    h: 18.0,
};

/// Compact default size for a newly placed field.
///
/// Sizes are multiples of that preset's minor step, so a row or column of
/// clicked fields stays even when snap is on. Smaller presets place smaller
/// fields. A checkbox is narrower than the text field on the same preset.
pub fn default_size(kind: FieldKind, grid: GridSize) -> (f32, f32) {
    match (grid, kind) {
        (GridSize::Small, FieldKind::Text | FieldKind::StaticText | FieldKind::Dropdown) => {
            (108.0, 13.5)
        }
        (GridSize::Medium, FieldKind::Text | FieldKind::StaticText | FieldKind::Dropdown) => {
            (144.0, 18.0)
        }
        (GridSize::Large, FieldKind::Text | FieldKind::StaticText | FieldKind::Dropdown) => {
            (216.0, 36.0)
        }
        (GridSize::Small, FieldKind::Checkbox | FieldKind::Radio) => (9.0, 9.0),
        (GridSize::Medium, FieldKind::Checkbox | FieldKind::Radio) => (18.0, 18.0),
        (GridSize::Large, FieldKind::Checkbox | FieldKind::Radio) => (36.0, 36.0),
        (GridSize::Small, FieldKind::Date) => (90.0, 13.5),
        (GridSize::Medium, FieldKind::Date) => (108.0, 18.0),
        (GridSize::Large, FieldKind::Date) => (144.0, 36.0),
        (GridSize::Small, FieldKind::Signature) => (108.0, 27.0),
        (GridSize::Medium, FieldKind::Signature) => (144.0, 36.0),
        (GridSize::Large, FieldKind::Signature) => (216.0, 72.0),
        (GridSize::Small, FieldKind::Table) => (162.0, 81.0),
        (GridSize::Medium, FieldKind::Table) => (216.0, 108.0),
        (GridSize::Large, FieldKind::Table) => (288.0, 144.0),
        (GridSize::Small, FieldKind::Line) => (108.0, 4.5),
        (GridSize::Medium, FieldKind::Line) => (144.0, 9.0),
        (GridSize::Large, FieldKind::Line) => (216.0, 18.0),
        (GridSize::Small, FieldKind::Rectangle | FieldKind::Image) => (108.0, 81.0),
        (GridSize::Medium, FieldKind::Rectangle | FieldKind::Image) => (144.0, 108.0),
        (GridSize::Large, FieldKind::Rectangle | FieldKind::Image) => (216.0, 162.0),
        (GridSize::Small, FieldKind::TextBox) => (162.0, 54.0),
        (GridSize::Medium, FieldKind::TextBox) => (216.0, 72.0),
        (GridSize::Large, FieldKind::TextBox) => (288.0, 144.0),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FieldId(pub u64);

/// Page objects. Designer objects are not form fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// One-line fillable AcroForm text field.
    Text,
    /// Static one-line label. Never an AcroForm field.
    StaticText,
    Checkbox,
    /// One button in an AcroForm radio group. Buttons that share a name are
    /// the same group.
    Radio,
    /// Visual grid. Not an AcroForm field, and the cells are not editable.
    Table,
    /// Unsigned AcroForm signature widget. The app does not capture ink.
    Signature,
    /// Fillable text field with a date length and a `YYYY-MM-DD` hint.
    Date,
    /// AcroForm combo box. Not a list box.
    Dropdown,
    /// Straight stroke between two points. Not an AcroForm field.
    Line,
    /// Stroked rectangle. Not an AcroForm field. Dragging can make it a square.
    Rectangle,
    /// Multi-line static text. Not an AcroForm field.
    TextBox,
    /// Image placeholder box. Not an AcroForm field. No file is embedded.
    Image,
}

impl FieldKind {
    /// Known page objects. The Insert menu and the toolbar sort their own labels.
    pub const INSERT: [FieldKind; 12] = [
        FieldKind::Text,
        FieldKind::StaticText,
        FieldKind::Checkbox,
        FieldKind::Radio,
        FieldKind::Table,
        FieldKind::Signature,
        FieldKind::Date,
        FieldKind::Dropdown,
        FieldKind::Line,
        FieldKind::Rectangle,
        FieldKind::TextBox,
        FieldKind::Image,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FieldKind::Text => "Text Field",
            FieldKind::StaticText => "Text",
            FieldKind::Checkbox => "Checkbox",
            FieldKind::Radio => "Radio",
            FieldKind::Table => "Table",
            FieldKind::Signature => "Signature",
            FieldKind::Date => "Date",
            FieldKind::Dropdown => "Drop-down",
            FieldKind::Line => "Line",
            FieldKind::Rectangle => "Square/Rectangle",
            FieldKind::TextBox => "Text Box",
            FieldKind::Image => "Image",
        }
    }

    /// Real AcroForm widget. Designer objects are drawn on the page instead.
    pub fn is_form_field(self) -> bool {
        matches!(
            self,
            FieldKind::Text
                | FieldKind::Checkbox
                | FieldKind::Radio
                | FieldKind::Signature
                | FieldKind::Date
                | FieldKind::Dropdown
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    id: FieldId,
    name: String,
    kind: FieldKind,
    rect: RectPt,
    /// Static label, radio caption, signature word, or date hint.
    caption: String,
    /// Radio on-state PDF name. Empty for every other kind.
    on_state: String,
    options: Vec<String>,
    /// Endpoints for a line. `None` for every other kind.
    line: Option<(PdfPoint, PdfPoint)>,
    style: FieldStyle,
}

impl Field {
    pub fn id(&self) -> FieldId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> FieldKind {
        self.kind
    }

    pub fn rect(&self) -> RectPt {
        self.rect
    }

    pub fn caption(&self) -> &str {
        &self.caption
    }

    /// PDF name of this radio's on state, such as `Yes`. Empty otherwise.
    pub fn on_state(&self) -> &str {
        &self.on_state
    }

    pub fn options(&self) -> &[String] {
        &self.options
    }

    pub fn is_form_field(&self) -> bool {
        self.kind.is_form_field()
    }

    pub fn line_ends(&self) -> Option<(PdfPoint, PdfPoint)> {
        self.line
    }

    pub fn style(&self) -> &FieldStyle {
        &self.style
    }

    /// Lines use distance to the segment. Everything else uses the rectangle.
    pub fn contains_point(&self, point: PdfPoint, radius: f32) -> bool {
        if let Some((start, end)) = self.line {
            return segment_distance(point, start, end) <= radius.max(3.0);
        }
        self.rect.contains(point)
    }

    /// `Some(0)` is the start point and `Some(1)` is the end point.
    pub fn hit_line_end(&self, point: PdfPoint, radius: f32) -> Option<usize> {
        let (start, end) = self.line?;
        let radius = radius.max(3.0);
        if point_distance(point, start) <= radius {
            Some(0)
        } else if point_distance(point, end) <= radius {
            Some(1)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    Period,
    Control,
    Duplicate,
    Missing,
    EmptyOptions,
}

impl std::fmt::Display for NameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NameError::Empty => write!(f, "Field name cannot be empty"),
            NameError::Period => write!(f, "Field name cannot contain a period"),
            NameError::Control => write!(f, "Field name cannot contain control characters"),
            NameError::Duplicate => write!(f, "Another field already uses that name"),
            NameError::Missing => write!(f, "Field is no longer on the page"),
            NameError::EmptyOptions => write!(f, "Enter at least one drop-down option"),
        }
    }
}

impl std::error::Error for NameError {}

#[derive(Clone, Debug)]
pub struct Document {
    page: Page,
    page_size: PageSize,
    orientation: Orientation,
    margins: Margins,
    header_text: String,
    footer_text: String,
    fields: Vec<Field>,
    next_id: u64,
    /// Adjacent radio captions. Off hides them on the canvas and in the export.
    radio_labels: bool,
}

impl Document {
    pub fn letter() -> Self {
        Self {
            page: Page::letter(),
            page_size: PageSize::Letter,
            orientation: Orientation::Portrait,
            margins: Margins::default(),
            header_text: "Header".to_string(),
            footer_text: "Footer".to_string(),
            fields: Vec::new(),
            next_id: 1,
            radio_labels: true,
        }
    }

    /// Canonical two-field document used by the export test and `--export`.
    pub fn sample() -> Self {
        let mut doc = Self::letter();
        doc.insert(
            FieldKind::Text,
            SAMPLE_TEXT_NAME,
            SAMPLE_TEXT_RECT,
            String::new(),
            String::new(),
            Vec::new(),
        );
        doc.insert(
            FieldKind::Checkbox,
            SAMPLE_CHECK_NAME,
            SAMPLE_CHECK_RECT,
            String::new(),
            String::new(),
            Vec::new(),
        );
        doc
    }

    pub fn page(&self) -> Page {
        self.page
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    pub fn field(&self, id: FieldId) -> Option<&Field> {
        self.fields.iter().find(|field| field.id == id)
    }

    pub fn radio_labels(&self) -> bool {
        self.radio_labels
    }

    pub fn set_radio_labels(&mut self, show: bool) {
        self.radio_labels = show;
    }

    pub fn page_size(&self) -> PageSize {
        self.page_size
    }

    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    pub fn margins(&self) -> Margins {
        self.margins
    }

    /// Print-safe rectangle from the current margins. Canvas only.
    pub fn margin_rect(&self) -> RectPt {
        let margins = self.margins;
        inset_rect(
            &self.page,
            margins.top,
            margins.right,
            margins.bottom,
            margins.left,
        )
    }

    pub fn header_text(&self) -> &str {
        &self.header_text
    }

    pub fn footer_text(&self) -> &str {
        &self.footer_text
    }

    pub fn set_page_size(&mut self, size: PageSize) {
        self.page_size = size;
        self.rebuild_page();
    }

    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
        self.rebuild_page();
    }

    pub fn set_margins(&mut self, margins: Margins) {
        let cap = self.page.width.min(self.page.height) * 0.45;
        self.margins = Margins {
            top: margins.top.clamp(0.0, cap),
            right: margins.right.clamp(0.0, cap),
            bottom: margins.bottom.clamp(0.0, cap),
            left: margins.left.clamp(0.0, cap),
        };
    }

    pub fn set_header_text(&mut self, text: &str) {
        self.header_text = clean_text(text, false);
    }

    pub fn set_footer_text(&mut self, text: &str) {
        self.footer_text = clean_text(text, false);
    }

    pub fn add_field(&mut self, kind: FieldKind, rect: RectPt) -> FieldId {
        if kind == FieldKind::Radio {
            return self.add_radio(rect, None);
        }
        let name = self.unique_name(kind);
        let (caption, on_state, options) = defaults_for(kind);
        self.insert(kind, &name, rect, caption, on_state, options)
    }

    /// Place a radio button. `group` joins that group when it already exists.
    /// Otherwise this starts a new group named `RadioN`, caption `Yes`.
    pub fn add_radio(&mut self, rect: RectPt, group: Option<&str>) -> FieldId {
        if let Some(group) = group {
            if self
                .fields
                .iter()
                .any(|field| field.kind == FieldKind::Radio && field.name == group)
            {
                let (caption, on_state) = self.next_radio_option(group);
                return self.insert(FieldKind::Radio, group, rect, caption, on_state, Vec::new());
            }
        }
        let name = self.unique_name(FieldKind::Radio);
        self.insert(
            FieldKind::Radio,
            &name,
            rect,
            "Yes".to_string(),
            "Yes".to_string(),
            Vec::new(),
        )
    }

    pub fn update_rect(&mut self, id: FieldId, rect: RectPt) {
        self.set_bounds(id, rect);
    }

    /// Set an exact rectangle. Lines scale their endpoints into the new bounds.
    pub fn set_bounds(&mut self, id: FieldId, rect: RectPt) {
        let page = self.page;
        let Some(index) = self.fields.iter().position(|field| field.id == id) else {
            return;
        };
        let rect = clamp_rect(rect, &page);
        if self.fields[index].kind == FieldKind::Line {
            if let Some((start, end)) = self.fields[index].line {
                let old = self.fields[index].rect;
                let (start, end) = map_line(start, end, old, rect);
                let start = clamp_point(start, &page);
                let end = clamp_point(end, &page);
                self.set_line_ends(id, start, end);
                return;
            }
        }
        self.fields[index].rect = rect;
    }

    /// Place a line from `start` to `end`. A click with no drag uses `add_field`.
    pub fn add_line(&mut self, start: PdfPoint, end: PdfPoint) -> FieldId {
        let page = self.page;
        let start = clamp_point(start, &page);
        let end = clamp_point(end, &page);
        let rect = line_bounds(start, end, &page);
        let name = self.unique_name(FieldKind::Line);
        let id = self.insert(
            FieldKind::Line,
            &name,
            rect,
            String::new(),
            String::new(),
            Vec::new(),
        );
        self.set_line_ends(id, start, end);
        id
    }

    pub fn set_line_end(&mut self, id: FieldId, index: usize, point: PdfPoint) {
        let page = self.page;
        let Some(field) = self.fields.iter().find(|field| field.id == id) else {
            return;
        };
        let Some((start, end)) = field.line else {
            return;
        };
        let point = clamp_point(point, &page);
        let (start, end) = if index == 0 {
            (point, end)
        } else {
            (start, point)
        };
        self.set_line_ends(id, start, end);
    }

    /// Place a copy of `source`, shifted by `(dx, dy)`. The copy gets a new name.
    pub fn paste_copy(&mut self, source: &Field, dx: f32, dy: f32) -> FieldId {
        let page = self.page;
        let rect = clamp_rect(
            RectPt {
                x: source.rect.x + dx,
                y: source.rect.y + dy,
                w: source.rect.w,
                h: source.rect.h,
            },
            &page,
        );
        let name = self.unique_name(source.kind);
        let id = self.insert(
            source.kind,
            &name,
            rect,
            source.caption.clone(),
            source.on_state.clone(),
            source.options.clone(),
        );
        if let Some(field) = self.fields.iter_mut().find(|field| field.id == id) {
            field.style = source.style.clone();
        }
        if let Some((start, end)) = source.line {
            self.set_line_ends(
                id,
                clamp_point(
                    PdfPoint {
                        x: start.x + dx,
                        y: start.y + dy,
                    },
                    &page,
                ),
                clamp_point(
                    PdfPoint {
                        x: end.x + dx,
                        y: end.y + dy,
                    },
                    &page,
                ),
            );
        }
        id
    }

    pub fn remove(&mut self, id: FieldId) -> bool {
        let before = self.fields.len();
        self.fields.retain(|field| field.id != id);
        self.fields.len() != before
    }

    pub fn set_name(&mut self, id: FieldId, name: &str) -> Result<(), NameError> {
        validate_field_name(name)?;
        let Some(index) = self.fields.iter().position(|field| field.id == id) else {
            return Err(NameError::Missing);
        };
        if self.name_conflicts(id, name) {
            return Err(NameError::Duplicate);
        }
        let kind = self.fields[index].kind;
        let old_name = self.fields[index].name.clone();
        if kind == FieldKind::Radio {
            for field in &mut self.fields {
                if field.kind == FieldKind::Radio && field.name == old_name {
                    field.name = name.to_string();
                }
            }
        } else {
            self.fields[index].name = name.to_string();
        }
        Ok(())
    }

    /// Caption for static text, a text box, or the label beside a radio or checkbox.
    /// A newline is kept so a text box can break lines. Other control characters are rejected.
    pub fn set_caption(&mut self, id: FieldId, caption: &str) -> Result<(), NameError> {
        if caption.chars().any(|ch| ch.is_control() && ch != '\n') {
            return Err(NameError::Control);
        }
        let Some(field) = self.fields.iter().find(|field| field.id == id) else {
            return Err(NameError::Missing);
        };
        let kind = field.kind;
        let group = field.name.clone();
        let on_state = if kind == FieldKind::Radio && !caption.is_empty() {
            Some(self.unique_on_state(id, &group, caption))
        } else {
            None
        };
        let Some(field) = self.fields.iter_mut().find(|field| field.id == id) else {
            return Err(NameError::Missing);
        };
        field.caption = caption.to_string();
        if let Some(on_state) = on_state {
            field.on_state = on_state;
        }
        Ok(())
    }

    /// Replace the combo options. Blank entries are dropped. Not a list box.
    pub fn set_options(&mut self, id: FieldId, options: Vec<String>) -> Result<(), NameError> {
        let cleaned: Vec<String> = options
            .into_iter()
            .map(|option| option.trim().to_string())
            .filter(|option| !option.is_empty())
            .collect();
        if cleaned.is_empty() {
            return Err(NameError::EmptyOptions);
        }
        if cleaned
            .iter()
            .any(|option| option.chars().any(|ch| ch.is_control()))
        {
            return Err(NameError::Control);
        }
        let Some(field) = self.fields.iter_mut().find(|field| field.id == id) else {
            return Err(NameError::Missing);
        };
        if field.kind != FieldKind::Dropdown {
            return Err(NameError::Missing);
        }
        field.options = cleaned;
        Ok(())
    }

    pub fn update_style(&mut self, id: FieldId, edit: impl FnOnce(&mut FieldStyle)) {
        let Some(field) = self.fields.iter_mut().find(|field| field.id == id) else {
            return;
        };
        edit(&mut field.style);
        field.style.font_size = field.style.font_size.clamp(4.0, 96.0);
        field.style.border_width = field.style.border_width.clamp(0.0, 24.0);
        field.style.stroke_width = field.style.stroke_width.clamp(0.0, 24.0);
        field.style.text_color = Rgb::new(
            field.style.text_color.r,
            field.style.text_color.g,
            field.style.text_color.b,
        );
        field.style.border_color = Rgb::new(
            field.style.border_color.r,
            field.style.border_color.g,
            field.style.border_color.b,
        );
        field.style.stroke_color = Rgb::new(
            field.style.stroke_color.r,
            field.style.stroke_color.g,
            field.style.stroke_color.b,
        );
        if let Some(fill) = field.style.fill {
            field.style.fill = Some(Rgb::new(fill.r, fill.g, fill.b));
        }
        if let Some(max) = field.style.max_chars {
            field.style.max_chars = Some(max.clamp(1, 5000));
        }
    }

    pub fn set_tooltip(&mut self, id: FieldId, tooltip: &str) {
        let tooltip = clean_text(tooltip, false);
        self.update_style(id, |style| style.tooltip = tooltip);
    }

    pub fn set_default_value(&mut self, id: FieldId, value: &str) {
        let value = clean_text(value, true);
        self.update_style(id, |style| style.default_value = value);
    }

    /// Required is stored on every button in a radio group.
    pub fn set_required(&mut self, id: FieldId, required: bool) {
        self.set_group_flag(id, |style| style.required = required);
    }

    /// Read-only is stored on every button in a radio group.
    pub fn set_read_only(&mut self, id: FieldId, read_only: bool) {
        self.set_group_flag(id, |style| style.read_only = read_only);
    }

    /// Checkbox and radio defaults. A selected radio clears the others.
    pub fn set_checked(&mut self, id: FieldId, checked: bool) {
        let Some(field) = self.fields.iter().find(|field| field.id == id) else {
            return;
        };
        if field.kind == FieldKind::Radio {
            let group = field.name.clone();
            for member in &mut self.fields {
                if member.kind != FieldKind::Radio || member.name != group {
                    continue;
                }
                if checked {
                    member.style.default_value = if member.id == id {
                        "On".to_string()
                    } else {
                        String::new()
                    };
                } else if member.id == id {
                    member.style.default_value.clear();
                }
            }
            return;
        }
        self.update_style(id, |style| {
            style.default_value = if checked {
                "Yes".to_string()
            } else {
                String::new()
            };
        });
    }

    pub fn align_fields(&mut self, ids: &[FieldId], edge: AlignEdge) {
        self.map_selection(ids, 2, |rects| align_rects(rects, edge));
    }

    pub fn distribute_fields(&mut self, ids: &[FieldId], horizontal: bool) {
        self.map_selection(ids, 3, |rects| distribute_rects(rects, horizontal));
    }

    /// `reference` is an index into `ids`. That item keeps its size.
    pub fn match_field_size(
        &mut self,
        ids: &[FieldId],
        reference: usize,
        width: bool,
        height: bool,
    ) {
        self.map_selection(ids, 2, |rects| match_size(rects, reference, width, height));
    }

    fn set_group_flag(&mut self, id: FieldId, edit: impl Fn(&mut FieldStyle)) {
        let Some(field) = self.fields.iter().find(|field| field.id == id) else {
            return;
        };
        let group = (field.kind == FieldKind::Radio).then(|| field.name.clone());
        for member in &mut self.fields {
            let same = member.id == id
                || group
                    .as_ref()
                    .is_some_and(|group| member.kind == FieldKind::Radio && member.name == *group);
            if same {
                edit(&mut member.style);
            }
        }
    }

    fn map_selection(&mut self, ids: &[FieldId], min: usize, edit: impl FnOnce(&mut [RectPt])) {
        let items: Vec<(FieldId, RectPt)> = ids
            .iter()
            .filter_map(|id| self.field(*id).map(|field| (*id, field.rect())))
            .collect();
        if items.len() < min {
            return;
        }
        let mut rects: Vec<RectPt> = items.iter().map(|(_, rect)| *rect).collect();
        edit(&mut rects);
        for (index, (id, _)) in items.iter().enumerate() {
            self.set_bounds(*id, rects[index]);
        }
    }

    fn rebuild_page(&mut self) {
        let (mut width, mut height) = self.page_size.portrait_pt();
        if self.orientation == Orientation::Landscape {
            std::mem::swap(&mut width, &mut height);
        }
        self.page = Page { width, height };
        let ids: Vec<FieldId> = self.fields.iter().map(|field| field.id).collect();
        for id in ids {
            let rect = self.field(id).map(|field| field.rect()).unwrap_or(RectPt {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
            });
            self.set_bounds(id, rect);
        }
        self.set_margins(self.margins);
    }

    fn insert(
        &mut self,
        kind: FieldKind,
        name: &str,
        rect: RectPt,
        caption: String,
        on_state: String,
        options: Vec<String>,
    ) -> FieldId {
        let id = FieldId(self.next_id);
        self.next_id += 1;
        let rect = clamp_rect(rect, &self.page);
        let line = (kind == FieldKind::Line).then(|| horizontal_line(rect));
        self.fields.push(Field {
            id,
            name: name.to_string(),
            kind,
            rect,
            caption,
            on_state,
            options,
            line,
            style: FieldStyle::for_kind(kind),
        });
        id
    }

    fn set_line_ends(&mut self, id: FieldId, start: PdfPoint, end: PdfPoint) {
        let page = self.page;
        let Some(field) = self.fields.iter_mut().find(|field| field.id == id) else {
            return;
        };
        field.line = Some((start, end));
        field.rect = line_bounds(start, end, &page);
    }

    fn name_conflicts(&self, id: FieldId, new_name: &str) -> bool {
        let group = self
            .fields
            .iter()
            .find(|field| field.id == id)
            .and_then(|field| (field.kind == FieldKind::Radio).then(|| field.name.clone()));
        self.fields.iter().any(|field| {
            if field.name != new_name || field.id == id {
                return false;
            }
            if let Some(group) = &group {
                if field.kind == FieldKind::Radio && &field.name == group {
                    return false;
                }
            }
            true
        })
    }

    fn next_radio_option(&self, group: &str) -> (String, String) {
        let members: Vec<&Field> = self
            .fields
            .iter()
            .filter(|field| field.kind == FieldKind::Radio && field.name == group)
            .collect();
        let (caption, base) = if members.len() == 1 && members[0].caption == "Yes" {
            ("No".to_string(), "No".to_string())
        } else {
            let n = members.len() + 1;
            (format!("Option {n}"), format!("Opt{n}"))
        };
        let on_state = disambiguate_state(&members, &base);
        (caption, on_state)
    }

    fn unique_on_state(&self, id: FieldId, group: &str, caption: &str) -> String {
        let members: Vec<&Field> = self
            .fields
            .iter()
            .filter(|field| field.id != id && field.kind == FieldKind::Radio && field.name == group)
            .collect();
        disambiguate_state(&members, &sanitize_on_state(caption))
    }

    fn unique_name(&self, kind: FieldKind) -> String {
        let base = match kind {
            FieldKind::Text => "Text",
            FieldKind::StaticText => "Label",
            FieldKind::Checkbox => "Check",
            FieldKind::Radio => "Radio",
            FieldKind::Table => "Table",
            FieldKind::Signature => "Sign",
            FieldKind::Date => "Date",
            FieldKind::Dropdown => "Choice",
            FieldKind::Line => "Line",
            FieldKind::Rectangle => "Shape",
            FieldKind::TextBox => "TextBox",
            FieldKind::Image => "Image",
        };
        let mut n = 1u32;
        loop {
            let candidate = format!("{base}{n}");
            if !self.fields.iter().any(|field| field.name == candidate) {
                return candidate;
            }
            n += 1;
        }
    }
}

fn validate_field_name(name: &str) -> Result<(), NameError> {
    if name.trim().is_empty() {
        return Err(NameError::Empty);
    }
    if name.contains('.') {
        return Err(NameError::Period);
    }
    if name.chars().any(|ch| ch.is_control()) {
        return Err(NameError::Control);
    }
    Ok(())
}

fn defaults_for(kind: FieldKind) -> (String, String, Vec<String>) {
    match kind {
        FieldKind::StaticText => ("Label".to_string(), String::new(), Vec::new()),
        FieldKind::Table => ("placeholder".to_string(), String::new(), Vec::new()),
        FieldKind::Signature => ("Sign".to_string(), String::new(), Vec::new()),
        FieldKind::Date => (DATE_HINT.to_string(), String::new(), Vec::new()),
        FieldKind::Dropdown => (
            String::new(),
            String::new(),
            ["Yes", "No", "Other"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
        FieldKind::TextBox => ("Text".to_string(), String::new(), Vec::new()),
        FieldKind::Image => ("Image".to_string(), String::new(), Vec::new()),
        FieldKind::Text
        | FieldKind::Checkbox
        | FieldKind::Radio
        | FieldKind::Line
        | FieldKind::Rectangle => (String::new(), String::new(), Vec::new()),
    }
}

fn horizontal_line(rect: RectPt) -> (PdfPoint, PdfPoint) {
    let y = rect.y + rect.h * 0.5;
    (PdfPoint { x: rect.x, y }, PdfPoint { x: rect.right(), y })
}

fn line_bounds(start: PdfPoint, end: PdfPoint, page: &Page) -> RectPt {
    clamp_rect(
        RectPt {
            x: start.x.min(end.x),
            y: start.y.min(end.y),
            w: (start.x - end.x).abs().max(1.0),
            h: (start.y - end.y).abs().max(1.0),
        },
        page,
    )
}

fn map_line(start: PdfPoint, end: PdfPoint, old: RectPt, new: RectPt) -> (PdfPoint, PdfPoint) {
    let map = |value: f32, origin: f32, span: f32, new_origin: f32, new_span: f32| {
        let same_origin = (origin - new_origin).abs() < 0.001;
        let same_span = (span - new_span).abs() < 0.001;
        // An unchanged axis stays put. A flat line is stored in a 1 pt box, so
        // recentering that pad would walk the other coordinate by 0.5 pt.
        if same_origin && same_span {
            value
        } else if span <= 1.0 {
            value + (new_origin - origin) + (new_span - span) * 0.5
        } else {
            new_origin + (value - origin) / span * new_span
        }
    };
    (
        PdfPoint {
            x: map(start.x, old.x, old.w, new.x, new.w),
            y: map(start.y, old.y, old.h, new.y, new.h),
        },
        PdfPoint {
            x: map(end.x, old.x, old.w, new.x, new.w),
            y: map(end.y, old.y, old.h, new.y, new.h),
        },
    )
}

fn clamp_point(point: PdfPoint, page: &Page) -> PdfPoint {
    PdfPoint {
        x: point.x.clamp(0.0, page.width),
        y: point.y.clamp(0.0, page.height),
    }
}

fn point_distance(a: PdfPoint, b: PdfPoint) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

fn segment_distance(point: PdfPoint, start: PdfPoint, end: PdfPoint) -> f32 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.01 {
        return point_distance(point, start);
    }
    let t = ((point.x - start.x) * dx + (point.y - start.y) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    point_distance(
        point,
        PdfPoint {
            x: start.x + dx * t,
            y: start.y + dy * t,
        },
    )
}

fn sanitize_on_state(caption: &str) -> String {
    let out: String = caption
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect();
    if out.is_empty() || out.eq_ignore_ascii_case("Off") {
        "Opt".to_string()
    } else {
        out
    }
}

fn disambiguate_state(members: &[&Field], base: &str) -> String {
    let mut candidate = base.to_string();
    let mut n = 2u32;
    loop {
        let taken = members.iter().any(|field| field.on_state == candidate);
        if !taken {
            return candidate;
        }
        candidate = format!("{base}{n}");
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GridSize;

    #[test]
    fn sample_is_one_letter_page_with_two_named_fields() {
        let doc = Document::sample();
        assert_eq!(doc.page(), Page::letter());
        assert_eq!(doc.fields().len(), 2);
        assert_eq!(doc.fields()[0].name(), SAMPLE_TEXT_NAME);
        assert_eq!(doc.fields()[0].kind(), FieldKind::Text);
        assert_eq!(doc.fields()[1].name(), SAMPLE_CHECK_NAME);
        assert_eq!(doc.fields()[1].kind(), FieldKind::Checkbox);
        assert!(doc.radio_labels());
    }

    #[test]
    fn insert_kinds_keep_their_labels() {
        // The Insert menu and the toolbar sort these labels themselves.
        let labels: Vec<_> = FieldKind::INSERT.iter().map(|kind| kind.label()).collect();
        assert_eq!(
            labels,
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
        assert!(FieldKind::Text.is_form_field());
        assert!(!FieldKind::StaticText.is_form_field());
        assert!(!FieldKind::Table.is_form_field());
        assert!(FieldKind::Radio.is_form_field());
        assert!(FieldKind::Dropdown.is_form_field());
        assert!(!FieldKind::Line.is_form_field());
        assert!(!FieldKind::Rectangle.is_form_field());
        assert!(!FieldKind::TextBox.is_form_field());
        assert!(!FieldKind::Image.is_form_field());
    }

    #[test]
    fn sample_rects_sit_on_every_grid() {
        for rect in [SAMPLE_TEXT_RECT, SAMPLE_CHECK_RECT] {
            for size in GridSize::ALL {
                for value in [rect.x, rect.y, rect.w, rect.h, rect.right(), rect.top()] {
                    let snapped = (value / size.minor_pt()).round() * size.minor_pt();
                    assert!(
                        (snapped - value).abs() < 1e-3,
                        "{value} is off the {size:?} grid"
                    );
                }
            }
        }
    }

    #[test]
    fn default_sizes_shrink_with_the_grid_and_stay_on_it() {
        for grid in GridSize::ALL {
            let (text_w, text_h) = default_size(FieldKind::Text, grid);
            let (check_w, check_h) = default_size(FieldKind::Checkbox, grid);
            assert!(check_w < text_w && check_h <= text_h);
            assert!((check_w - check_h).abs() < 1e-3, "checkboxes are square");
            assert_eq!(
                default_size(FieldKind::Radio, grid),
                default_size(FieldKind::Checkbox, grid)
            );
            assert_eq!(
                default_size(FieldKind::StaticText, grid),
                default_size(FieldKind::Text, grid)
            );
            for kind in FieldKind::INSERT {
                let (w, h) = default_size(kind, grid);
                for value in [w, h] {
                    let snapped = (value / grid.minor_pt()).round() * grid.minor_pt();
                    assert!(
                        (snapped - value).abs() < 1e-3,
                        "{kind:?} {value} is off the {grid:?} grid"
                    );
                }
            }
        }
        let small = default_size(FieldKind::Text, GridSize::Small);
        let medium = default_size(FieldKind::Text, GridSize::Medium);
        let large = default_size(FieldKind::Text, GridSize::Large);
        assert!(small.0 < medium.0 && medium.0 < large.0);
        assert!(small.1 < medium.1 && medium.1 < large.1);
        let small_box = default_size(FieldKind::Checkbox, GridSize::Small).0;
        let medium_box = default_size(FieldKind::Checkbox, GridSize::Medium).0;
        let large_box = default_size(FieldKind::Checkbox, GridSize::Large).0;
        assert!(small_box < medium_box && medium_box < large_box);
    }

    #[test]
    fn names_reject_empty_period_and_duplicates() {
        let mut doc = Document::letter();
        let id = doc.add_field(FieldKind::Text, SAMPLE_TEXT_RECT);
        assert_eq!(doc.field(id).unwrap().name(), "Text1");
        assert_eq!(doc.set_name(id, "   "), Err(NameError::Empty));
        assert_eq!(doc.set_name(id, "a.b"), Err(NameError::Period));
        assert_eq!(doc.set_name(id, "full_name"), Ok(()));
        let other = doc.add_field(FieldKind::Checkbox, SAMPLE_CHECK_RECT);
        assert_eq!(doc.set_name(other, "full_name"), Err(NameError::Duplicate));
        assert_eq!(doc.set_name(id, "full_name"), Ok(()));
    }

    #[test]
    fn radio_group_shares_a_name_and_yes_no_labels() {
        let mut doc = Document::letter();
        let yes = doc.add_field(FieldKind::Radio, SAMPLE_CHECK_RECT);
        let group = doc.field(yes).unwrap().name().to_string();
        assert_eq!(group, "Radio1");
        assert_eq!(doc.field(yes).unwrap().caption(), "Yes");
        assert_eq!(doc.field(yes).unwrap().on_state(), "Yes");
        let no = doc.add_radio(SAMPLE_TEXT_RECT, Some(&group));
        assert_eq!(doc.field(no).unwrap().name(), group);
        assert_eq!(doc.field(no).unwrap().caption(), "No");
        assert_eq!(doc.field(no).unwrap().on_state(), "No");
        let third = doc.add_radio(
            RectPt {
                x: 72.0,
                y: 630.0,
                w: 18.0,
                h: 18.0,
            },
            Some(&group),
        );
        assert_eq!(doc.field(third).unwrap().caption(), "Option 3");
        assert_eq!(doc.field(third).unwrap().on_state(), "Opt3");
        assert_eq!(doc.set_name(yes, "Answer"), Ok(()));
        assert_eq!(doc.field(no).unwrap().name(), "Answer");
        assert_eq!(doc.field(third).unwrap().name(), "Answer");
        let text = doc.add_field(FieldKind::Text, SAMPLE_TEXT_RECT);
        let text_name = doc.field(text).unwrap().name().to_string();
        assert_eq!(doc.set_name(no, &text_name), Err(NameError::Duplicate));
        let other = doc.add_field(FieldKind::Radio, SAMPLE_CHECK_RECT);
        assert_ne!(doc.field(other).unwrap().name(), "Answer");
        assert_eq!(doc.field(other).unwrap().caption(), "Yes");
    }

    #[test]
    fn caption_keeps_a_newline_and_rejects_other_controls() {
        let mut doc = Document::letter();
        let id = doc.add_field(FieldKind::TextBox, SAMPLE_TEXT_RECT);
        assert_eq!(doc.set_caption(id, "Line one\nLine two"), Ok(()));
        assert_eq!(doc.field(id).unwrap().caption(), "Line one\nLine two");
        assert_eq!(doc.set_caption(id, "bad\u{0001}"), Err(NameError::Control));
        assert_eq!(doc.field(id).unwrap().caption(), "Line one\nLine two");
    }

    #[test]
    fn static_text_caption_is_not_a_field_name() {
        let mut doc = Document::letter();
        let id = doc.add_field(FieldKind::StaticText, SAMPLE_TEXT_RECT);
        assert!(!doc.field(id).unwrap().is_form_field());
        assert_eq!(doc.set_caption(id, "Full name"), Ok(()));
        assert_eq!(doc.field(id).unwrap().caption(), "Full name");
        assert_eq!(doc.set_caption(id, "a.b"), Ok(()));
        let table = doc.add_field(FieldKind::Table, SAMPLE_TEXT_RECT);
        assert!(!doc.field(table).unwrap().is_form_field());
        assert_eq!(doc.field(table).unwrap().caption(), "placeholder");
    }

    #[test]
    fn dropdown_starts_with_three_options_and_keeps_one() {
        let mut doc = Document::letter();
        let id = doc.add_field(FieldKind::Dropdown, SAMPLE_TEXT_RECT);
        assert_eq!(
            doc.field(id).unwrap().options(),
            ["Yes".to_string(), "No".to_string(), "Other".to_string()]
        );
        assert_eq!(
            doc.set_options(id, vec![" ".into(), "Red".into(), "Blue".into()]),
            Ok(())
        );
        assert_eq!(
            doc.field(id).unwrap().options(),
            ["Red".to_string(), "Blue".to_string()]
        );
        assert_eq!(
            doc.set_options(id, vec!["  ".into()]),
            Err(NameError::EmptyOptions)
        );
        assert_eq!(doc.field(id).unwrap().options().len(), 2);
    }

    #[test]
    fn menu_line_is_horizontal_and_drag_line_keeps_its_ends() {
        let mut doc = Document::letter();
        let id = doc.add_field(FieldKind::Line, SAMPLE_TEXT_RECT);
        let (start, end) = doc.field(id).unwrap().line_ends().unwrap();
        assert!((start.y - end.y).abs() < 0.1);
        assert!(end.x > start.x);
        assert!(!doc.field(id).unwrap().is_form_field());
        let drawn = doc.add_line(
            PdfPoint { x: 40.0, y: 80.0 },
            PdfPoint { x: 120.0, y: 200.0 },
        );
        let (start, end) = doc.field(drawn).unwrap().line_ends().unwrap();
        assert!((start.x - 40.0).abs() < 0.1);
        assert!((start.y - 80.0).abs() < 0.1);
        assert!((end.x - 120.0).abs() < 0.1);
        assert!((end.y - 200.0).abs() < 0.1);
        doc.set_line_end(drawn, 1, PdfPoint { x: 90.0, y: 90.0 });
        let (_, end) = doc.field(drawn).unwrap().line_ends().unwrap();
        assert!((end.x - 90.0).abs() < 0.1);
        assert!((end.y - 90.0).abs() < 0.1);
    }

    #[test]
    fn moving_a_line_on_one_axis_leaves_the_other_axis_alone() {
        let mut doc = Document::letter();
        let id = doc.add_line(
            PdfPoint { x: 40.0, y: 80.0 },
            PdfPoint { x: 140.0, y: 80.0 },
        );
        for _ in 0..4 {
            let rect = doc.field(id).unwrap().rect();
            let mut next = rect;
            next.x += 1.0;
            doc.set_bounds(id, next);
        }
        let (start, end) = doc.field(id).unwrap().line_ends().unwrap();
        assert!(
            (start.y - 80.0).abs() < 0.01,
            "x edit moved y to {}",
            start.y
        );
        assert!((end.y - 80.0).abs() < 0.01, "x edit moved y to {}", end.y);
        assert!((start.x - 44.0).abs() < 0.01);
        assert!((end.x - 144.0).abs() < 0.01);

        let rect = doc.field(id).unwrap().rect();
        let mut next = rect;
        next.y += 2.0;
        doc.set_bounds(id, next);
        let (start, end) = doc.field(id).unwrap().line_ends().unwrap();
        assert!(
            (start.x - 44.0).abs() < 0.01,
            "y edit moved x to {}",
            start.x
        );
        assert!((end.x - 144.0).abs() < 0.01, "y edit moved x to {}", end.x);
        assert!((start.y - 82.0).abs() < 0.01);
        assert!((end.y - 82.0).abs() < 0.01);
    }

    #[test]
    fn radio_labels_default_on_and_can_be_hidden() {
        let mut doc = Document::letter();
        assert!(doc.radio_labels());
        doc.set_radio_labels(false);
        assert!(!doc.radio_labels());
    }
}
