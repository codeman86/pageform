//! PageForm core: one-page document model, grid snapping, and AcroForm export.
//!
//! This crate has no UI. Milestone 0 is a single US Letter page. Multi-page
//! documents are Milestone 1 and are intentionally not represented here.

mod document;
mod export;
mod geom;
mod grid;

pub use document::{
    default_size, Document, Field, FieldId, FieldKind, NameError, DATE_HINT, SAMPLE_CHECK_NAME,
    SAMPLE_CHECK_RECT, SAMPLE_TEXT_NAME, SAMPLE_TEXT_RECT, TABLE_COLUMNS, TABLE_ROWS,
};
pub use export::{export_pdf, ExportError};
pub use geom::{
    clamp_rect, hit_handle, move_rect, place_default, print_safe_rect, rect_from_drag, resize_rect,
    snap_point, snap_value, Handle, Page, PdfPoint, RectPt, ScreenRect, ViewTransform,
    LETTER_HEIGHT, LETTER_WIDTH, PRINT_SAFE_MARGIN_PT,
};
pub use grid::GridSize;
