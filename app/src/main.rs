mod preview;
mod theme;
mod ui;

use pageform_core::{export_pdf, Document, GridSize};
use preview::{render_preview, PreviewLayers};
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    match dispatch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("pageform: {err}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut demo = false;
    let mut export_path: Option<String> = None;
    let mut preview_path: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--demo" => demo = true,
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--export" => {
                index += 1;
                export_path = Some(required_value(&args, index, "--export")?);
            }
            "--preview" => {
                index += 1;
                preview_path = Some(required_value(&args, index, "--preview")?);
            }
            other => {
                return Err(format!(
                    "unknown argument \"{other}\". Run pageform --help."
                ));
            }
        }
        index += 1;
    }

    if export_path.is_some() || preview_path.is_some() {
        let doc = Document::sample();
        if let Some(path) = export_path {
            let bytes = export_pdf(&doc).map_err(|err| err.to_string())?;
            std::fs::write(&path, bytes).map_err(|err| format!("writing {path}: {err}"))?;
            println!("wrote {path}");
        }
        if let Some(path) = preview_path {
            let bytes = render_preview(
                &doc,
                &PreviewLayers {
                    grid: GridSize::Medium,
                    dots: true,
                    guides: true,
                    margins: true,
                },
            )
            .map_err(|err| format!("preview: {err}"))?;
            std::fs::write(&path, bytes).map_err(|err| format!("writing {path}: {err}"))?;
            println!("wrote {path}");
        }
        return Ok(());
    }

    ui::run_gui(demo).map_err(|err| err.to_string())
}

fn required_value(args: &[String], index: usize, flag: &str) -> Result<String, String> {
    args.get(index)
        .cloned()
        .ok_or_else(|| format!("{flag} needs a file path"))
}

fn print_help() {
    println!(
        "\
PageForm — design a fillable PDF form

Usage:
  pageform
      Open a blank US Letter page.
  pageform --demo
      Open the sample text field and checkbox.
  pageform --export sample.pdf [--preview sample.png]
      Write the sample form without opening a window.

Scroll to zoom. Middle-drag or Space-drag to pan.
The window opens in the dark theme. View → Dark / Light, or the toolbar
button, switches it and the choice is restored next launch.
File exports the toolbar path. Edit selects or deletes. Insert arms the
text field or checkbox tool.
Grid sizes: Small (1/16 in), Medium (1/8 in, default), Large (1/4 in).
Dots cover the page. Guide lines and the print-safe margin are canvas only.
Snap follows the active size so columns and rows stay even."
    );
}
