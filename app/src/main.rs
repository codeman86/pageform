mod prefs;
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

    #[cfg(target_os = "linux")]
    ensure_keyboard_libraries()?;

    ui::run_gui(demo).map_err(|err| err.to_string())
}

/// winit loads these with dlopen, and a missing one panics inside the window
/// startup. The AppImage bundles both. A raw binary should say which library
/// is missing instead of aborting.
#[cfg(target_os = "linux")]
fn ensure_keyboard_libraries() -> Result<(), String> {
    let needed = [
        ("libxkbcommon-x11.so.0", "libxkbcommon-x11-0"),
        ("libxcb-xkb.so.1", "libxcb-xkb1"),
    ];
    let missing: Vec<(&str, &str)> = needed
        .into_iter()
        .filter(|(soname, _)| !shared_library_loads(soname))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let libraries = missing
        .iter()
        .map(|(soname, _)| *soname)
        .collect::<Vec<_>>()
        .join(" and ");
    let packages = missing
        .iter()
        .map(|(_, package)| *package)
        .collect::<Vec<_>>()
        .join(" and ");
    Err(format!(
        "missing {libraries} for keyboard input. The PageForm AppImage includes them. Install {packages}, or run the AppImage."
    ))
}

#[cfg(target_os = "linux")]
fn shared_library_loads(name: &str) -> bool {
    let Ok(name) = std::ffi::CString::new(name) else {
        return false;
    };
    // RTLD_NOW so a missing dependency (libxcb-xkb under libxkbcommon-x11) fails here.
    const RTLD_NOW: i32 = 2;
    unsafe {
        let handle = dlopen(name.as_ptr(), RTLD_NOW);
        if handle.is_null() {
            false
        } else {
            dlclose(handle);
            true
        }
    }
}

#[cfg(target_os = "linux")]
extern "C" {
    fn dlopen(filename: *const std::ffi::c_char, flags: i32) -> *mut std::ffi::c_void;
    fn dlclose(handle: *mut std::ffi::c_void) -> i32;
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
The window opens in the dark theme. Edit → Preferences, View → Dark / Light,
or the toolbar moon/sun icon switches it, and the choice is restored next launch.
The properties panel starts hidden. View → Properties, the toolbar icon,
or right-click Properties opens it. Double-click edits visible text on the
page. Enter finishes, Esc cancels, and Shift+Enter adds a line in a text box
or multiline field. Pin keeps the panel open.
Unpinned, it closes when you click empty canvas with Select, or when the
selection is deleted. Preferences stores that open and pinned choice.
Nothing selected edits the page. One item edits that item. Drag on empty
canvas to draw a selection box. Shift-click or Ctrl-click selects several
items. Edit > Align, the properties panel, and the right-click menu align,
distribute, and match size.
Calculations are a placeholder. Margins, header, and footer stay on the canvas.
File: New Blank Document starts a blank page and asks before discarding
unsaved work. New from Template opens a .pgt template as a new untitled
document and does not change the template. Open reads .pgd documents, .pgt
templates, and older .pageform files. Ctrl or Cmd+S saves the current .pgd,
or asks for a .pgd name the first time and after a template. Ctrl or
Cmd+Shift+S is Save As, which can store a Document (.pgd) or a Template
(.pgt). Saving a template does not make that file the open document. Ctrl or
Cmd+P prints. The window title shows the file name and a * when there are
unsaved changes. Exit asks before closing a changed file.
Export PDF asks where to save the PDF. If there is no system file dialog,
PageForm asks for the folder and file name.
Edit: Select, Select All, Copy, Cut, Paste, and Delete work. Ctrl or Cmd
with A, C, X, V, and the Delete key do the same. Preferences is a dialog
for the theme and radio labels.
Insert drops a default item in the view, listed alphabetically. A toolbar
insert tool places that default size on a click and sizes the item when you
drag. The items are: Checkbox,
Date, Drop-down, Image, Line, Radio, Signature, Square/Rectangle, Table,
Text (static label), Text Box, and Text Field. Toolbar icons use that same
order, with Square and Rectangle as separate tools under one Shapes icon. Insert Page is not
implemented yet. Header and footer are in the View menu only.
A selected radio makes the next radio join that group. Preferences can hide
the label beside each radio. There is no list box.
View toggles grid dots, guide lines, the print-safe margin, header, footer,
page numbers, and snap, and chooses the grid size and Dark or Light.
Header, footer, and page numbers are canvas only.
Help opens a separate FAQ window. About opens a centered dialog.
Grid sizes: Small (1/16 in), Medium (1/8 in, default), Large (1/4 in).
Snap follows the active size so columns and rows stay even."
    );
}
