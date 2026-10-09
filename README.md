# PageForm

Free, open-source desktop app for designing fillable PDF forms.

Milestone 0 is a single US Letter page: place a text field and a checkbox, snap them to a grid, and export a fillable AcroForm PDF. Windows and Linux are supported. macOS is not part of this milestone.

## Workspace

| Crate | Path | Role |
| --- | --- | --- |
| `pageform-core` | `core/` | Document model, geometry, snapping, PDF export. No UI. |
| `pageform` | `app/` | egui / eframe window and page canvas. |

## Run

```bash
cargo run -p pageform
```

That opens a blank 612 × 792 pt page. Scroll to zoom. Middle-drag, or hold Space and drag, to pan.

```bash
cargo run -p pageform -- --demo
cargo run -p pageform -- --export sample-form.pdf --preview sample-canvas.png
```

`--export` writes the built-in sample (one text field, one checkbox) without opening a window. The preview PNG is a headless drawing of that page, for environments with no display.

## Canvas

Coordinates are PDF points. The origin is the bottom-left of the page. One inch is 72 pt.

Tools:

- **Select** — click a field, drag it to move, drag a handle to resize. Edit the name in the side panel. Delete removes the selection.
- **Text field** / **Checkbox** — drag a rectangle, or click to drop a default-size field. Names start as `Text1`, `Check1`, and so on.

A click places a compact field. The size follows the active grid so more fields fit on the page than a typical word-processor form, and a row of clicks stays aligned. Checkboxes are the smaller item.

| Preset | Text field | Checkbox |
| --- | --- | --- |
| Small | 108 × 13.5 pt | 9 × 9 pt |
| Medium | 144 × 18 pt | 18 × 18 pt |
| Large | 216 × 36 pt | 36 × 36 pt |

## Grid, guides, and margins

Three densities. Medium is the default. Snap always uses the **minor** spacing of the active size, including when the dots or guide lines are hidden, so columns and rows land on the same lines.

| Preset | Minor (snap) | Major |
| --- | --- | --- |
| Small | 0.0625 in (4.5 pt), denser | 0.25 in (18 pt) |
| Medium | 0.125 in (9 pt) | 0.5 in (36 pt) |
| Large | 0.25 in (18 pt), wider | 1.0 in (72 pt) |

Canvas overlays, each with its own toggle:

- **Dots** cover the full page at the minor spacing. Major intersections are larger.
- **Guides** are a light line grid. They are not part of the PDF.
- **Margins** draw the print-safe outline, 0.5 in (36 pt) inside the page edge. That outline is not part of the PDF. A later print option could draw it; the default would be off, and Milestone 0 does not write it at all.

## Export

Export writes the current page as a fillable PDF:

- Text fields include a `/DA` of `/Helv 12 Tf 0 g` and a normal appearance stream. Helvetica is a standard font named `/Helv` in the form resources.
- Checkboxes include explicit `/AP /N` appearance streams named `/On` and `/Off`. The exported value is `/Off`.
- The file does not set `NeedAppearances`.

The sample used by tests and `--export` places:

| Name | Kind | PDF `/Rect` (bottom-left origin) |
| --- | --- | --- |
| `full_name` | text | `[72 702 216 720]` |
| `agree` | checkbox | `[72 666 90 684]` |

## Test

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The AcroForm test reads the sample PDF with `lopdf` (tests only) and checks the form, both fields, their names, and their rectangles.

## Not in this milestone

Multiple pages (next milestone), form templates, radio buttons, labels, alignment, undo, saving a design file, installers, PDF import, and macOS builds. The print-safe outline is a canvas guide only.

## License

MIT. See [LICENSE](LICENSE).
