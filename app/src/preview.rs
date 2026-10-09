//! Headless canvas image for CI. This is not a captured egui frame.

use font8x8::{UnicodeFonts, BASIC_FONTS};
use pageform_core::{print_safe_rect, Document, FieldKind, GridSize, RectPt};
use png::{BitDepth, ColorType, Encoder};
use std::io::Cursor;

const SCALE: f32 = 2.0;
const MARGIN: f32 = 32.0;

/// Which canvas overlays to draw. None of these are written into the PDF.
pub struct PreviewLayers {
    pub grid: GridSize,
    pub dots: bool,
    pub guides: bool,
    pub margins: bool,
}

pub fn render_preview(
    doc: &Document,
    layers: &PreviewLayers,
) -> Result<Vec<u8>, png::EncodingError> {
    let page = doc.page();
    let width = (page.width * SCALE + MARGIN * 2.0).round() as u32;
    let height = (page.height * SCALE + MARGIN * 2.0).round() as u32;
    let mut image = Image::new(width, height, [232, 234, 238]);

    let page_x = MARGIN;
    let page_y = MARGIN;
    let page_w = page.width * SCALE;
    let page_h = page.height * SCALE;
    image.fill_rect(page_x, page_y, page_w, page_h, [255, 255, 255]);

    if layers.guides {
        paint_guides(&mut image, page.width, page.height, layers.grid);
    }
    if layers.dots {
        paint_dots(&mut image, page.width, page.height, layers.grid);
    }

    image.stroke_rect(page_x, page_y, page_w, page_h, 2.0, [70, 74, 82]);

    for field in doc.fields() {
        let rect = field.rect();
        let (x, y, w, h) = pdf_rect_px(rect, page.height);
        let border = match field.kind() {
            FieldKind::Text => [36, 64, 112],
            FieldKind::Checkbox => [32, 32, 36],
        };
        image.fill_rect(x, y, w, h, [255, 255, 255]);
        image.stroke_rect(x, y, w, h, 2.0, border);
        match field.kind() {
            FieldKind::Text => {
                image.text(x + 8.0, y + 8.0, field.name(), [40, 48, 64], 2);
            }
            FieldKind::Checkbox => {
                image.text(
                    x + w + 8.0,
                    y + (h - 16.0).max(0.0) * 0.5,
                    field.name(),
                    [40, 48, 64],
                    2,
                );
            }
        }
    }

    if layers.margins {
        let safe = print_safe_rect(&page);
        let (x, y, w, h) = pdf_rect_px(safe, page.height);
        image.stroke_rect(x, y, w, h, 2.0, [186, 112, 112]);
    }

    let mut cursor = Cursor::new(Vec::new());
    let mut encoder = Encoder::new(&mut cursor, width, height);
    encoder.set_color(ColorType::Rgb);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image.data)?;
    writer.finish()?;
    Ok(cursor.into_inner())
}

fn paint_guides(image: &mut Image, page_w: f32, page_h: f32, grid: GridSize) {
    for x in grid_lines(page_w, grid.minor_pt()) {
        let major = on_step(x, grid.major_pt());
        let color = if major {
            [168, 196, 216]
        } else {
            [214, 228, 236]
        };
        let (px, _, _, _) = pdf_rect_px(
            RectPt {
                x,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            page_h,
        );
        let (_, top, _, _) = pdf_rect_px(
            RectPt {
                x: 0.0,
                y: page_h,
                w: 0.0,
                h: 0.0,
            },
            page_h,
        );
        image.vline(
            px,
            top,
            page_h * SCALE,
            if major { 2.0 } else { 1.0 },
            color,
        );
    }
    for y in grid_lines(page_h, grid.minor_pt()) {
        let major = on_step(y, grid.major_pt());
        let color = if major {
            [168, 196, 216]
        } else {
            [214, 228, 236]
        };
        let (_, py, _, _) = pdf_rect_px(
            RectPt {
                x: 0.0,
                y,
                w: 0.0,
                h: 0.0,
            },
            page_h,
        );
        image.hline(
            MARGIN,
            py,
            page_w * SCALE,
            if major { 2.0 } else { 1.0 },
            color,
        );
    }
}

fn paint_dots(image: &mut Image, page_w: f32, page_h: f32, grid: GridSize) {
    for x in grid_lines(page_w, grid.minor_pt()) {
        for y in grid_lines(page_h, grid.minor_pt()) {
            let major = on_step(x, grid.major_pt()) && on_step(y, grid.major_pt());
            let (px, py, _, _) = pdf_rect_px(
                RectPt {
                    x,
                    y,
                    w: 0.0,
                    h: 0.0,
                },
                page_h,
            );
            let size = if major { 5.0 } else { 3.0 };
            let color = if major {
                [142, 150, 164]
            } else {
                [176, 182, 192]
            };
            image.fill_rect(px - size * 0.5, py - size * 0.5, size, size, color);
        }
    }
}

fn pdf_rect_px(rect: RectPt, page_h: f32) -> (f32, f32, f32, f32) {
    (
        MARGIN + rect.x * SCALE,
        MARGIN + (page_h - rect.top()) * SCALE,
        rect.w * SCALE,
        rect.h * SCALE,
    )
}

fn grid_lines(span: f32, step: f32) -> impl Iterator<Item = f32> {
    let count = (span / step).floor() as i32;
    (0..=count)
        .map(move |index| index as f32 * step)
        .filter(move |value| *value <= span + 0.01)
}

fn on_step(value: f32, step: f32) -> bool {
    let nearest = (value / step).round() * step;
    (nearest - value).abs() < 0.05
}

struct Image {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Image {
    fn new(width: u32, height: u32, color: [u8; 3]) -> Self {
        let data = vec![0; (width * height * 3) as usize];
        let mut image = Self {
            width,
            height,
            data,
        };
        image.fill_rect(0.0, 0.0, width as f32, height as f32, color);
        image
    }

    fn set(&mut self, x: i32, y: i32, color: [u8; 3]) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let index = ((y as u32 * self.width + x as u32) * 3) as usize;
        self.data[index..index + 3].copy_from_slice(&color);
    }

    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [u8; 3]) {
        let x0 = x.round() as i32;
        let y0 = y.round() as i32;
        let x1 = (x + w).round() as i32;
        let y1 = (y + h).round() as i32;
        for py in y0..y1 {
            for px in x0..x1 {
                self.set(px, py, color);
            }
        }
    }

    fn hline(&mut self, x: f32, y: f32, w: f32, thickness: f32, color: [u8; 3]) {
        self.fill_rect(x, y - thickness * 0.5, w, thickness, color);
    }

    fn vline(&mut self, x: f32, y: f32, h: f32, thickness: f32, color: [u8; 3]) {
        self.fill_rect(x - thickness * 0.5, y, thickness, h, color);
    }

    fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, thickness: f32, color: [u8; 3]) {
        self.hline(x, y, w, thickness, color);
        self.hline(x, y + h, w, thickness, color);
        self.vline(x, y, h, thickness, color);
        self.vline(x + w, y, h, thickness, color);
    }

    fn text(&mut self, x: f32, y: f32, text: &str, color: [u8; 3], scale: i32) {
        let mut cursor = x.round() as i32;
        let top = y.round() as i32;
        for ch in text.chars() {
            let Some(glyph) = BASIC_FONTS.get(ch).or_else(|| BASIC_FONTS.get('?')) else {
                cursor += 8 * scale;
                continue;
            };
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..8 {
                    if bits & (1 << col) != 0 {
                        for sy in 0..scale {
                            for sx in 0..scale {
                                self.set(
                                    cursor + col * scale + sx,
                                    top + row as i32 * scale + sy,
                                    color,
                                );
                            }
                        }
                    }
                }
            }
            cursor += 8 * scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_png_covers_the_letter_page() {
        let bytes = render_preview(
            &Document::sample(),
            &PreviewLayers {
                grid: GridSize::Medium,
                dots: true,
                guides: true,
                margins: true,
            },
        )
        .unwrap();
        assert_eq!(&bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
        let decoder = png::Decoder::new(Cursor::new(bytes));
        let reader = decoder.read_info().unwrap();
        assert!(reader.info().width > 600);
        assert!(reader.info().height > 700);
    }
}
