//! Page geometry in PDF points. The origin is the bottom-left of the page.

use crate::grid::PT_PER_INCH;

pub const LETTER_WIDTH: f32 = 8.5 * PT_PER_INCH;
pub const LETTER_HEIGHT: f32 = 11.0 * PT_PER_INCH;

/// Inset from each edge of the page that many printers cannot mark.
/// Drawn on the canvas only. PDF export does not include this outline.
pub const PRINT_SAFE_MARGIN_PT: f32 = 0.5 * PT_PER_INCH;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Page {
    pub width: f32,
    pub height: f32,
}

impl Page {
    pub fn letter() -> Self {
        Self {
            width: LETTER_WIDTH,
            height: LETTER_HEIGHT,
        }
    }
}

/// Rectangle inside the print-safe margin. Canvas guide only.
pub fn print_safe_rect(page: &Page) -> RectPt {
    let inset = PRINT_SAFE_MARGIN_PT;
    RectPt {
        x: inset,
        y: inset,
        w: (page.width - inset * 2.0).max(1.0),
        h: (page.height - inset * 2.0).max(1.0),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PdfPoint {
    pub x: f32,
    pub y: f32,
}

/// Axis-aligned rectangle. `x` and `y` are the bottom-left corner in PDF points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectPt {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl RectPt {
    pub fn right(self) -> f32 {
        self.x + self.w
    }

    pub fn top(self) -> f32 {
        self.y + self.h
    }

    pub fn contains(self, p: PdfPoint) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.top()
    }

    /// `[llx, lly, urx, ury]` for a PDF `/Rect`.
    pub fn pdf_rect(self) -> [f32; 4] {
        [self.x, self.y, self.right(), self.top()]
    }
}

/// Screen rectangle in top-left coordinates (egui points).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Maps PDF page space to a pan/zoom view. `origin_*` is the screen position
/// of the page's top-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    pub origin_x: f32,
    pub origin_y: f32,
    pub zoom: f32,
    pub page_height: f32,
}

impl ViewTransform {
    pub fn new(page_height: f32) -> Self {
        Self {
            origin_x: 0.0,
            origin_y: 0.0,
            zoom: 1.0,
            page_height,
        }
    }

    pub fn screen_to_pdf(&self, screen_x: f32, screen_y: f32) -> PdfPoint {
        PdfPoint {
            x: (screen_x - self.origin_x) / self.zoom,
            y: self.page_height - (screen_y - self.origin_y) / self.zoom,
        }
    }

    pub fn pdf_to_screen(&self, p: PdfPoint) -> (f32, f32) {
        (
            self.origin_x + p.x * self.zoom,
            self.origin_y + (self.page_height - p.y) * self.zoom,
        )
    }

    pub fn pdf_rect_to_screen(&self, rect: RectPt) -> ScreenRect {
        let (x, y) = self.pdf_to_screen(PdfPoint {
            x: rect.x,
            y: rect.top(),
        });
        ScreenRect {
            x,
            y,
            w: rect.w * self.zoom,
            h: rect.h * self.zoom,
        }
    }

    /// Zoom while keeping the PDF point under `(screen_x, screen_y)` fixed.
    pub fn zoom_at(&mut self, screen_x: f32, screen_y: f32, new_zoom: f32) {
        let pdf = self.screen_to_pdf(screen_x, screen_y);
        self.zoom = new_zoom;
        self.origin_x = screen_x - pdf.x * self.zoom;
        self.origin_y = screen_y - (self.page_height - pdf.y) * self.zoom;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Nw,
    N,
    Ne,
    E,
    Se,
    S,
    Sw,
    W,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Handle::Nw,
        Handle::N,
        Handle::Ne,
        Handle::E,
        Handle::Se,
        Handle::S,
        Handle::Sw,
        Handle::W,
    ];

    pub fn point(self, rect: RectPt) -> PdfPoint {
        let cx = rect.x + rect.w * 0.5;
        let cy = rect.y + rect.h * 0.5;
        match self {
            Handle::Nw => PdfPoint {
                x: rect.x,
                y: rect.top(),
            },
            Handle::N => PdfPoint {
                x: cx,
                y: rect.top(),
            },
            Handle::Ne => PdfPoint {
                x: rect.right(),
                y: rect.top(),
            },
            Handle::E => PdfPoint {
                x: rect.right(),
                y: cy,
            },
            Handle::Se => PdfPoint {
                x: rect.right(),
                y: rect.y,
            },
            Handle::S => PdfPoint { x: cx, y: rect.y },
            Handle::Sw => PdfPoint {
                x: rect.x,
                y: rect.y,
            },
            Handle::W => PdfPoint { x: rect.x, y: cy },
        }
    }
}

pub fn snap_value(value: f32, grid: f32) -> f32 {
    if grid <= f32::EPSILON {
        return value;
    }
    (value / grid).round() * grid
}

pub fn snap_point(point: PdfPoint, grid: Option<f32>) -> PdfPoint {
    match grid {
        Some(grid) => PdfPoint {
            x: snap_value(point.x, grid),
            y: snap_value(point.y, grid),
        },
        None => point,
    }
}

pub fn clamp_rect(rect: RectPt, page: &Page) -> RectPt {
    let w = rect.w.clamp(1.0, page.width);
    let h = rect.h.clamp(1.0, page.height);
    RectPt {
        x: rect.x.clamp(0.0, page.width - w),
        y: rect.y.clamp(0.0, page.height - h),
        w,
        h,
    }
}

pub fn move_rect(
    origin: RectPt,
    pointer: PdfPoint,
    grab_x: f32,
    grab_y: f32,
    grid: Option<f32>,
    page: &Page,
) -> RectPt {
    let mut x = pointer.x - grab_x;
    let mut y = pointer.y - grab_y;
    if let Some(grid) = grid {
        x = snap_value(x, grid);
        y = snap_value(y, grid);
    }
    clamp_rect(RectPt { x, y, ..origin }, page)
}

pub fn resize_rect(
    origin: RectPt,
    handle: Handle,
    pointer: PdfPoint,
    grid: Option<f32>,
    min: f32,
    page: &Page,
) -> RectPt {
    let pointer = snap_point(pointer, grid);
    let min = min.max(1.0);
    let mut left = origin.x;
    let mut bottom = origin.y;
    let mut right = origin.right();
    let mut top = origin.top();

    if matches!(handle, Handle::W | Handle::Nw | Handle::Sw) {
        left = pointer.x.min(right - min);
    }
    if matches!(handle, Handle::E | Handle::Ne | Handle::Se) {
        right = pointer.x.max(left + min);
    }
    if matches!(handle, Handle::S | Handle::Se | Handle::Sw) {
        bottom = pointer.y.min(top - min);
    }
    if matches!(handle, Handle::N | Handle::Ne | Handle::Nw) {
        top = pointer.y.max(bottom + min);
    }

    clamp_rect(
        RectPt {
            x: left,
            y: bottom,
            w: right - left,
            h: top - bottom,
        },
        page,
    )
}

pub fn rect_from_drag(
    start: PdfPoint,
    end: PdfPoint,
    grid: Option<f32>,
    min: f32,
    page: &Page,
) -> RectPt {
    let start = snap_point(start, grid);
    let end = snap_point(end, grid);
    let min = min.max(1.0);
    let left = start.x.min(end.x);
    let mut right = start.x.max(end.x);
    let bottom = start.y.min(end.y);
    let mut top = start.y.max(end.y);
    if right - left < min {
        right = left + min;
    }
    if top - bottom < min {
        top = bottom + min;
    }
    clamp_rect(
        RectPt {
            x: left,
            y: bottom,
            w: right - left,
            h: top - bottom,
        },
        page,
    )
}

/// `anchor` is the visual top-left of the new field (PDF x, PDF top edge).
pub fn place_default(anchor: PdfPoint, w: f32, h: f32, grid: Option<f32>, page: &Page) -> RectPt {
    let anchor = snap_point(anchor, grid);
    clamp_rect(
        RectPt {
            x: anchor.x,
            y: anchor.y - h,
            w,
            h,
        },
        page,
    )
}

pub fn hit_handle(rect: RectPt, point: PdfPoint, radius: f32) -> Option<Handle> {
    let radius_sq = radius * radius;
    Handle::ALL.into_iter().find(|handle| {
        let at = handle.point(rect);
        let dx = at.x - point.x;
        let dy = at.y - point.y;
        dx * dx + dy * dy <= radius_sq
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GridSize;

    fn close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1e-3, "{actual} != {expected}");
    }

    #[test]
    fn letter_is_us_letter_points() {
        let page = Page::letter();
        close(page.width, 612.0);
        close(page.height, 792.0);
    }

    #[test]
    fn screen_top_left_is_pdf_top_left() {
        let view = ViewTransform::new(792.0);
        let pdf = view.screen_to_pdf(0.0, 0.0);
        close(pdf.x, 0.0);
        close(pdf.y, 792.0);
        let pdf = view.screen_to_pdf(72.0, 72.0);
        close(pdf.x, 72.0);
        close(pdf.y, 720.0);
    }

    #[test]
    fn pdf_screen_roundtrip() {
        let view = ViewTransform {
            origin_x: 40.0,
            origin_y: 25.0,
            zoom: 1.5,
            page_height: 792.0,
        };
        let original = PdfPoint { x: 100.0, y: 640.0 };
        let (sx, sy) = view.pdf_to_screen(original);
        let back = view.screen_to_pdf(sx, sy);
        close(back.x, original.x);
        close(back.y, original.y);
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let mut view = ViewTransform {
            origin_x: 10.0,
            origin_y: 20.0,
            zoom: 1.0,
            page_height: 792.0,
        };
        let before = view.screen_to_pdf(200.0, 150.0);
        view.zoom_at(200.0, 150.0, 2.5);
        let after = view.screen_to_pdf(200.0, 150.0);
        close(after.x, before.x);
        close(after.y, before.y);
    }

    #[test]
    fn snap_follows_the_active_grid_size() {
        let page = Page::letter();
        let origin = RectPt {
            x: 72.0,
            y: 72.0,
            w: 36.0,
            h: 36.0,
        };
        let pointer = PdfPoint { x: 82.0, y: 80.0 };
        let moved = |grid: Option<f32>| move_rect(origin, pointer, 0.0, 0.0, grid, &page);

        let medium = moved(Some(GridSize::Medium.minor_pt()));
        close(medium.x, 81.0);
        close(medium.y, 81.0);
        close(medium.w, 36.0);

        let large = moved(Some(GridSize::Large.minor_pt()));
        close(large.x, 90.0);
        close(large.y, 72.0);

        let small = moved(Some(GridSize::Small.minor_pt()));
        close(small.x, 81.0);
        close(small.y, 81.0);

        let free = moved(None);
        close(free.x, 82.0);
        close(free.y, 80.0);
    }

    #[test]
    fn resize_snaps_the_moving_edge() {
        let page = Page::letter();
        let origin = RectPt {
            x: 72.0,
            y: 72.0,
            w: 36.0,
            h: 36.0,
        };
        let resized = resize_rect(
            origin,
            Handle::E,
            PdfPoint { x: 100.0, y: 80.0 },
            Some(GridSize::Medium.minor_pt()),
            9.0,
            &page,
        );
        close(resized.x, 72.0);
        close(resized.w, 27.0);
        close(resized.h, 36.0);
    }

    #[test]
    fn snap_keeps_a_row_on_one_baseline() {
        let page = Page::letter();
        let grid = Some(GridSize::Medium.minor_pt());
        let (w, h) = (144.0, 18.0);
        let left = place_default(PdfPoint { x: 10.0, y: 700.0 }, w, h, grid, &page);
        let right = place_default(PdfPoint { x: 160.0, y: 703.0 }, w, h, grid, &page);
        close(left.y, right.y);
        close(left.top(), right.top());
        close(left.h, h);
    }

    #[test]
    fn print_safe_rect_is_a_half_inch_inset() {
        let safe = print_safe_rect(&Page::letter());
        close(safe.x, 36.0);
        close(safe.y, 36.0);
        close(safe.w, 612.0 - 72.0);
        close(safe.h, 792.0 - 72.0);
    }

    #[test]
    fn place_default_snaps_the_top_left() {
        let page = Page::letter();
        let rect = place_default(
            PdfPoint { x: 10.0, y: 700.0 },
            216.0,
            36.0,
            Some(GridSize::Medium.minor_pt()),
            &page,
        );
        close(rect.x, 9.0);
        close(rect.top(), 702.0);
        close(rect.h, 36.0);
    }
}
