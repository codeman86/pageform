# PageForm

Free, open-source desktop app for designing fillable PDF forms. This milestone is one US Letter page (612 by 792 pt) with pan and zoom, a toggleable dot grid, snap, and a text field plus a checkbox. Medium, the default grid, is 0.125 in minor and 0.5 in major. Small is 0.0625 in minor and 0.25 in major. Large is 0.25 in minor and 1 in major. Snap follows the active minor spacing, including when the dots are hidden, so columns and rows stay even. Guide lines and a 0.5 in print-safe outline can be shown on the canvas and are not written into the PDF. Export writes a fillable AcroForm file. Windows and Linux only.

The window opens in the dark theme. Dark / Light in the side panel switches the panels and the canvas grid, and that choice is restored the next time the app opens.

![PageForm letter page with a text field and a checkbox](docs/screenshot.png)

## Build

Install stable Rust, then:

```bash
cargo run -p pageform
cargo test --workspace
cargo run -p pageform -- --export sample-form.pdf
```

Scroll to zoom. Middle-drag, or hold Space and drag, to pan. `cargo run -p pageform -- --demo` opens the sample fields used by the export test.

## License

MIT. See [LICENSE](LICENSE).
