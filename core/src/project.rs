//! Version 1 `.pageform` project file.
//!
//! One file holds a list of pages. This app edits the first page. Every item
//! property the panel can change is stored.

use serde::{Deserialize, Serialize};

use super::{Document, FieldKind, PdfPoint, RectPt};
use crate::style::{FieldStyle, Margins, Orientation, PageSize};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    version: u32,
    radio_labels: bool,
    pages: Vec<ProjectPage>,
}

#[derive(Serialize, Deserialize)]
struct ProjectPage {
    size: PageSize,
    orientation: Orientation,
    margins: Margins,
    header_text: String,
    footer_text: String,
    items: Vec<ProjectItem>,
}

#[derive(Serialize, Deserialize)]
struct ProjectItem {
    name: String,
    kind: FieldKind,
    rect: RectPt,
    caption: String,
    on_state: String,
    options: Vec<String>,
    line: Option<(PdfPoint, PdfPoint)>,
    style: FieldStyle,
}

impl Document {
    pub fn to_project_ron(&self) -> Result<String, String> {
        let page = ProjectPage {
            size: self.page_size,
            orientation: self.orientation,
            margins: self.margins,
            header_text: self.header_text.clone(),
            footer_text: self.footer_text.clone(),
            items: self
                .fields
                .iter()
                .map(|field| ProjectItem {
                    name: field.name.clone(),
                    kind: field.kind,
                    rect: field.rect,
                    caption: field.caption.clone(),
                    on_state: field.on_state.clone(),
                    options: field.options.clone(),
                    line: field.line,
                    style: field.style.clone(),
                })
                .collect(),
        };
        let file = ProjectFile {
            version: FORMAT_VERSION,
            radio_labels: self.radio_labels,
            pages: vec![page],
        };
        ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::new())
            .map_err(|err| format!("could not write the project: {err}"))
    }

    pub fn from_project_ron(text: &str) -> Result<Self, String> {
        let file: ProjectFile =
            ron::from_str(text).map_err(|err| format!("could not read the project: {err}"))?;
        if file.version != FORMAT_VERSION {
            return Err(format!(
                "this file is PageForm version {}. This app reads version {FORMAT_VERSION}.",
                file.version
            ));
        }
        let Some(page) = file.pages.into_iter().next() else {
            return Err("the project has no pages".to_string());
        };
        let mut doc = Self::letter();
        doc.page_size = page.size;
        doc.orientation = page.orientation;
        doc.rebuild_page();
        doc.set_margins(page.margins);
        doc.header_text = page.header_text;
        doc.footer_text = page.footer_text;
        doc.radio_labels = file.radio_labels;
        for item in page.items {
            let id = doc.insert(
                item.kind,
                &item.name,
                item.rect,
                item.caption,
                item.on_state,
                item.options,
            );
            if let Some(field) = doc.fields.iter_mut().find(|field| field.id == id) {
                field.style = item.style;
            }
            if let Some((start, end)) = item.line {
                doc.set_line_ends(id, start, end);
            }
        }
        doc.mark_clean();
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FieldKind, SAMPLE_TEXT_RECT};

    #[test]
    fn project_round_trip_keeps_page_items_and_clears_the_dirty_flag() {
        let mut doc = Document::letter();
        assert!(!doc.is_dirty());
        let id = doc.add_field(FieldKind::Text, SAMPLE_TEXT_RECT);
        doc.set_name(id, "person").unwrap();
        doc.set_default_value(id, "Ada");
        doc.set_header_text("Hello");
        doc.set_page_size(PageSize::A4);
        doc.add_line(
            PdfPoint { x: 40.0, y: 80.0 },
            PdfPoint { x: 140.0, y: 120.0 },
        );
        assert!(doc.is_dirty());
        let text = doc.to_project_ron().unwrap();
        assert!(text.contains("version: 1"), "{text}");
        assert!(text.contains("person"), "{text}");
        let loaded = Document::from_project_ron(&text).unwrap();
        assert!(!loaded.is_dirty());
        assert_eq!(loaded.page_size(), PageSize::A4);
        assert_eq!(loaded.header_text(), "Hello");
        assert_eq!(loaded.fields().len(), 2);
        let person = loaded
            .fields()
            .iter()
            .find(|field| field.name() == "person")
            .unwrap();
        assert_eq!(person.style().default_value, "Ada");
        assert!((person.rect().x - SAMPLE_TEXT_RECT.x).abs() < 0.01);
        let line = loaded
            .fields()
            .iter()
            .find(|field| field.kind() == FieldKind::Line)
            .unwrap();
        let (start, end) = line.line_ends().unwrap();
        assert!((start.y - 80.0).abs() < 0.01);
        assert!((end.y - 120.0).abs() < 0.01);
    }

    #[test]
    fn a_newer_project_version_is_rejected() {
        let err =
            Document::from_project_ron("(version: 2, radio_labels: true, pages: [])").unwrap_err();
        assert!(err.contains("version 2"), "{err}");
    }
}
