//! Single-page form document.
//!
//! Milestone 0 stores one US Letter page. A page list, page navigation, and
//! per-page fields belong to Milestone 1 and are not modeled here.

use crate::geom::{clamp_rect, Page, RectPt};
use crate::grid::GridSize;

pub const SAMPLE_TEXT_NAME: &str = "full_name";
pub const SAMPLE_CHECK_NAME: &str = "agree";

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
        (GridSize::Small, FieldKind::Text) => (108.0, 13.5),
        (GridSize::Small, FieldKind::Checkbox) => (9.0, 9.0),
        (GridSize::Medium, FieldKind::Text) => (144.0, 18.0),
        (GridSize::Medium, FieldKind::Checkbox) => (18.0, 18.0),
        (GridSize::Large, FieldKind::Text) => (216.0, 36.0),
        (GridSize::Large, FieldKind::Checkbox) => (36.0, 36.0),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FieldId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Checkbox,
}

impl FieldKind {
    pub fn label(self) -> &'static str {
        match self {
            FieldKind::Text => "Text",
            FieldKind::Checkbox => "Checkbox",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    id: FieldId,
    name: String,
    kind: FieldKind,
    rect: RectPt,
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    Period,
    Control,
    Duplicate,
    Missing,
}

impl std::fmt::Display for NameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NameError::Empty => write!(f, "Field name cannot be empty"),
            NameError::Period => write!(f, "Field name cannot contain a period"),
            NameError::Control => write!(f, "Field name cannot contain control characters"),
            NameError::Duplicate => write!(f, "Another field already uses that name"),
            NameError::Missing => write!(f, "Field is no longer on the page"),
        }
    }
}

impl std::error::Error for NameError {}

#[derive(Clone, Debug)]
pub struct Document {
    page: Page,
    fields: Vec<Field>,
    next_id: u64,
}

impl Document {
    pub fn letter() -> Self {
        Self {
            page: Page::letter(),
            fields: Vec::new(),
            next_id: 1,
        }
    }

    /// Canonical two-field document used by the export test and `--export`.
    pub fn sample() -> Self {
        let mut doc = Self::letter();
        doc.insert(FieldKind::Text, SAMPLE_TEXT_NAME, SAMPLE_TEXT_RECT);
        doc.insert(FieldKind::Checkbox, SAMPLE_CHECK_NAME, SAMPLE_CHECK_RECT);
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

    pub fn add_field(&mut self, kind: FieldKind, rect: RectPt) -> FieldId {
        let name = self.unique_name(kind);
        self.insert(kind, &name, rect)
    }

    pub fn update_rect(&mut self, id: FieldId, rect: RectPt) {
        if let Some(field) = self.fields.iter_mut().find(|field| field.id == id) {
            field.rect = clamp_rect(rect, &self.page);
        }
    }

    pub fn remove(&mut self, id: FieldId) -> bool {
        let before = self.fields.len();
        self.fields.retain(|field| field.id != id);
        self.fields.len() != before
    }

    pub fn set_name(&mut self, id: FieldId, name: &str) -> Result<(), NameError> {
        if name.trim().is_empty() {
            return Err(NameError::Empty);
        }
        if name.contains('.') {
            return Err(NameError::Period);
        }
        if name.chars().any(|ch| ch.is_control()) {
            return Err(NameError::Control);
        }
        if self
            .fields
            .iter()
            .any(|field| field.id != id && field.name == name)
        {
            return Err(NameError::Duplicate);
        }
        let field = self
            .fields
            .iter_mut()
            .find(|field| field.id == id)
            .ok_or(NameError::Missing)?;
        field.name = name.to_string();
        Ok(())
    }

    fn insert(&mut self, kind: FieldKind, name: &str, rect: RectPt) -> FieldId {
        let id = FieldId(self.next_id);
        self.next_id += 1;
        self.fields.push(Field {
            id,
            name: name.to_string(),
            kind,
            rect: clamp_rect(rect, &self.page),
        });
        id
    }

    fn unique_name(&self, kind: FieldKind) -> String {
        let base = match kind {
            FieldKind::Text => "Text",
            FieldKind::Checkbox => "Check",
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
            for value in [text_w, text_h, check_w, check_h] {
                let snapped = (value / grid.minor_pt()).round() * grid.minor_pt();
                assert!((snapped - value).abs() < 1e-3);
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
}
