//! Structural checks for the sample AcroForm. `lopdf` is a test-only reader.

use lopdf::{Document, Object};
use pageform_core::{
    export_pdf, BorderStyleKind, CheckboxStyle, Document as Form, FieldKind, FontFamily,
    Orientation, PageSize, RectPt, Rgb, TextAlign, SAMPLE_CHECK_NAME, SAMPLE_CHECK_RECT,
    SAMPLE_TEXT_NAME, SAMPLE_TEXT_RECT,
};

#[test]
fn sample_pdf_has_two_acroform_fields_at_canvas_rects() {
    let bytes = export_pdf(&Form::sample()).expect("export");
    assert!(
        !bytes
            .windows(b"NeedAppearances".len())
            .any(|window| window == b"NeedAppearances"),
        "export must not rely on NeedAppearances"
    );

    let doc = Document::load_mem(&bytes).expect("parse pdf");
    let catalog = catalog(&doc);
    let page = page(&doc, catalog);
    let media = numbers(deref(&doc, page.get(b"MediaBox").expect("MediaBox")));
    assert_close(media[0], 0.0);
    assert_close(media[1], 0.0);
    assert_close(media[2], 612.0);
    assert_close(media[3], 792.0);

    let kids = deref(&doc, pages(&doc, catalog).get(b"Kids").expect("Kids"));
    assert_eq!(
        kids.as_array().expect("kids").len(),
        1,
        "M0 is a single page"
    );
    let annots = deref(&doc, page.get(b"Annots").expect("Annots"));
    assert_eq!(annots.as_array().expect("annots").len(), 2);
    let contents = stream_plain(&doc, deref(&doc, page.get(b"Contents").expect("Contents")));
    let operators: String = contents
        .iter()
        .filter(|byte| !byte.is_ascii_whitespace())
        .map(|byte| *byte as char)
        .collect();
    assert_eq!(
        operators, "qQ",
        "page content must stay empty so dots, guide lines, and the print-safe margin are not exported"
    );

    let acro = deref(&doc, catalog.get(b"AcroForm").expect("AcroForm"));
    let acro = acro.as_dict().expect("AcroForm dict");
    assert!(acro.get(b"NeedAppearances").is_err());
    let dr = deref(&doc, acro.get(b"DR").expect("DR"));
    let fonts = deref(&doc, dr.as_dict().expect("DR").get(b"Font").expect("Font"));
    let helv = deref(
        &doc,
        fonts.as_dict().expect("Font").get(b"Helv").expect("Helv"),
    );
    let helv = helv.as_dict().expect("Helv font");
    assert_name(
        deref(&doc, helv.get(b"BaseFont").expect("BaseFont")),
        b"Helvetica",
    );
    assert_name(
        deref(&doc, helv.get(b"Encoding").expect("Encoding")),
        b"WinAnsiEncoding",
    );

    let da = string_bytes(deref(&doc, acro.get(b"DA").expect("AcroForm DA")));
    assert!(
        contains(da, b"/Helv") && contains(da, b"Tf"),
        "AcroForm /DA must select /Helv with Tf, got {}",
        String::from_utf8_lossy(da)
    );

    let fields = deref(&doc, acro.get(b"Fields").expect("Fields"));
    let fields = fields.as_array().expect("Fields array");
    assert_eq!(fields.len(), 2);

    let text = find_field(&doc, fields, SAMPLE_TEXT_NAME);
    let check = find_field(&doc, fields, SAMPLE_CHECK_NAME);
    assert_name(deref(&doc, text.get(b"FT").unwrap()), b"Tx");
    assert_name(deref(&doc, check.get(b"FT").unwrap()), b"Btn");
    assert_rect(deref(&doc, text.get(b"Rect").unwrap()), SAMPLE_TEXT_RECT);
    assert_rect(deref(&doc, check.get(b"Rect").unwrap()), SAMPLE_CHECK_RECT);

    let text_da = string_bytes(deref(&doc, text.get(b"DA").expect("text /DA")));
    assert!(contains(text_da, b"/Helv"));
    assert!(contains(text_da, b"Tf"));

    let text_n = appearance_n(&doc, text);
    assert!(
        text_n.as_stream().is_ok(),
        "text /AP /N must be a single appearance stream"
    );

    let check_n = appearance_n(&doc, check);
    let states = check_n.as_dict().expect("checkbox /AP /N state dict");
    let on = deref(&doc, states.get(b"On").expect("checkbox /On appearance"));
    let off = deref(&doc, states.get(b"Off").expect("checkbox /Off appearance"));
    assert!(on
        .as_stream()
        .expect("/On stream")
        .dict
        .get(b"BBox")
        .is_ok());
    assert!(off
        .as_stream()
        .expect("/Off stream")
        .dict
        .get(b"BBox")
        .is_ok());
    assert!(stream_len(&doc, on) > 8);
    assert!(stream_len(&doc, off) > 8);
    assert_name(deref(&doc, check.get(b"V").expect("/V")), b"Off");
    assert_name(deref(&doc, check.get(b"AS").expect("/AS")), b"Off");

    if let Ok(flags) = check.get(b"Ff") {
        let bits = match deref(&doc, flags) {
            Object::Integer(value) => *value,
            other => panic!("unexpected /Ff {other:?}"),
        };
        assert_eq!(bits & (1 << 15), 0, "checkbox must not be a radio");
        assert_eq!(bits & (1 << 16), 0, "checkbox must not be a pushbutton");
    }
}

#[test]
fn small_grid_rect_exports_half_points() {
    let mut doc = Form::letter();
    let rect = RectPt {
        x: 4.5,
        y: 9.0,
        w: 13.5,
        h: 18.0,
    };
    doc.add_field(FieldKind::Checkbox, rect);
    let bytes = export_pdf(&doc).unwrap();
    let pdf = Document::load_mem(&bytes).unwrap();
    let catalog = catalog(&pdf);
    let acro = deref(&pdf, catalog.get(b"AcroForm").unwrap());
    let fields = deref(&pdf, acro.as_dict().unwrap().get(b"Fields").unwrap());
    let field = deref(&pdf, &fields.as_array().unwrap()[0]);
    assert_rect(
        deref(&pdf, field.as_dict().unwrap().get(b"Rect").unwrap()),
        rect,
    );
}

fn catalog(doc: &Document) -> &lopdf::Dictionary {
    let root = deref(doc, doc.trailer.get(b"Root").expect("Root"));
    root.as_dict().expect("catalog")
}

fn pages<'a>(doc: &'a Document, catalog: &'a lopdf::Dictionary) -> &'a lopdf::Dictionary {
    deref(doc, catalog.get(b"Pages").expect("Pages"))
        .as_dict()
        .expect("pages")
}

fn page<'a>(doc: &'a Document, catalog: &'a lopdf::Dictionary) -> &'a lopdf::Dictionary {
    let kids = deref(doc, pages(doc, catalog).get(b"Kids").unwrap());
    deref(doc, &kids.as_array().unwrap()[0])
        .as_dict()
        .expect("page")
}

fn find_field<'a>(doc: &'a Document, fields: &'a [Object], name: &str) -> &'a lopdf::Dictionary {
    for field in fields {
        let dict = deref(doc, field).as_dict().expect("field");
        let bytes = string_bytes(deref(doc, dict.get(b"T").expect("/T")));
        if bytes == name.as_bytes() {
            return dict;
        }
    }
    panic!("missing field {name}");
}

fn appearance_n<'a>(doc: &'a Document, field: &'a lopdf::Dictionary) -> &'a Object {
    let ap = deref(doc, field.get(b"AP").expect("/AP"));
    deref(doc, ap.as_dict().expect("/AP").get(b"N").expect("/N"))
}

fn stream_len(doc: &Document, obj: &Object) -> usize {
    stream_plain(doc, obj).len()
}

fn stream_plain(doc: &Document, obj: &Object) -> Vec<u8> {
    let stream = deref(doc, obj).as_stream().expect("stream");
    stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone())
}

fn assert_name(obj: &Object, expected: &[u8]) {
    match obj {
        Object::Name(name) if name.as_slice() == expected => {}
        other => panic!(
            "expected /{}, got {other:?}",
            String::from_utf8_lossy(expected)
        ),
    }
}

fn assert_rect(obj: &Object, expected: RectPt) {
    let actual = numbers(obj);
    let want = expected.pdf_rect();
    for (index, (got, expected)) in actual.iter().zip(want).enumerate() {
        assert!(
            (got - expected).abs() < 0.02,
            "rect[{index}] {got} != {expected}"
        );
    }
}

fn numbers(obj: &Object) -> Vec<f32> {
    obj.as_array().expect("array").iter().map(number).collect()
}

fn number(obj: &Object) -> f32 {
    match obj {
        Object::Integer(value) => *value as f32,
        Object::Real(value) => *value,
        other => panic!("not a number: {other:?}"),
    }
}

fn string_bytes(obj: &Object) -> &[u8] {
    match obj {
        Object::String(bytes, _) => bytes,
        other => panic!("not a string: {other:?}"),
    }
}

fn deref<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
    let mut obj = obj;
    for _ in 0..8 {
        match obj {
            Object::Reference(id) => {
                obj = doc
                    .get_object(*id)
                    .unwrap_or_else(|err| panic!("missing {id:?}: {err}"));
            }
            _ => return obj,
        }
    }
    panic!("reference cycle");
}

fn assert_close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.02, "{actual} != {expected}");
}

#[test]
fn new_insert_items_export_as_fields_or_page_content() {
    let mut doc = Form::letter();
    let text = RectPt {
        x: 72.0,
        y: 700.0,
        w: 144.0,
        h: 18.0,
    };
    doc.add_field(FieldKind::Text, text);
    doc.add_field(
        FieldKind::StaticText,
        RectPt {
            x: 72.0,
            y: 670.0,
            w: 144.0,
            h: 18.0,
        },
    );
    doc.set_caption(doc.fields()[1].id(), "Full name").unwrap();
    doc.add_field(
        FieldKind::Checkbox,
        RectPt {
            x: 72.0,
            y: 640.0,
            w: 18.0,
            h: 18.0,
        },
    );
    let radio = doc.add_field(
        FieldKind::Radio,
        RectPt {
            x: 72.0,
            y: 610.0,
            w: 18.0,
            h: 18.0,
        },
    );
    let group = doc.field(radio).unwrap().name().to_string();
    doc.add_radio(
        RectPt {
            x: 72.0,
            y: 580.0,
            w: 18.0,
            h: 18.0,
        },
        Some(&group),
    );
    doc.add_field(
        FieldKind::Table,
        RectPt {
            x: 200.0,
            y: 500.0,
            w: 216.0,
            h: 108.0,
        },
    );
    doc.add_field(
        FieldKind::Signature,
        RectPt {
            x: 72.0,
            y: 450.0,
            w: 144.0,
            h: 36.0,
        },
    );
    doc.add_field(
        FieldKind::Date,
        RectPt {
            x: 72.0,
            y: 420.0,
            w: 108.0,
            h: 18.0,
        },
    );
    doc.add_field(
        FieldKind::Dropdown,
        RectPt {
            x: 72.0,
            y: 390.0,
            w: 144.0,
            h: 18.0,
        },
    );
    doc.add_field(
        FieldKind::Line,
        RectPt {
            x: 72.0,
            y: 360.0,
            w: 144.0,
            h: 9.0,
        },
    );
    doc.add_field(
        FieldKind::Rectangle,
        RectPt {
            x: 300.0,
            y: 300.0,
            w: 72.0,
            h: 54.0,
        },
    );
    doc.add_field(
        FieldKind::TextBox,
        RectPt {
            x: 72.0,
            y: 240.0,
            w: 180.0,
            h: 54.0,
        },
    );
    doc.add_field(
        FieldKind::Image,
        RectPt {
            x: 300.0,
            y: 200.0,
            w: 90.0,
            h: 72.0,
        },
    );

    let bytes = export_pdf(&doc).unwrap();
    assert!(!bytes
        .windows(b"NeedAppearances".len())
        .any(|window| window == b"NeedAppearances"));
    let pdf = Document::load_mem(&bytes).unwrap();
    let root = catalog(&pdf);
    let page_dict = page(&pdf, root);
    let acro = deref(&pdf, root.get(b"AcroForm").unwrap())
        .as_dict()
        .unwrap();
    let fields = deref(&pdf, acro.get(b"Fields").unwrap());
    let fields = fields.as_array().unwrap();
    // Text, checkbox, one radio parent, signature, date, drop-down.
    assert_eq!(fields.len(), 6);
    let annots = deref(&pdf, page_dict.get(b"Annots").unwrap());
    // The radio parent is not a widget. Two radio widgets plus five other fields.
    assert_eq!(annots.as_array().unwrap().len(), 7);

    let names = field_names(&pdf, fields);
    assert!(names.iter().any(|name| name == "Text1"));
    assert!(names.iter().any(|name| name == "Check1"));
    assert!(names.iter().any(|name| name == "Radio1"));
    assert!(names.iter().any(|name| name == "Sign1"));
    assert!(names.iter().any(|name| name == "Date1"));
    assert!(names.iter().any(|name| name == "Choice1"));
    assert!(!names.iter().any(|name| name.contains("Label")));
    assert!(!names.iter().any(|name| name.contains("Table")));
    assert!(!names.iter().any(|name| name.contains("Line")));
    assert!(!names.iter().any(|name| name.contains("Shape")));
    assert!(!names.iter().any(|name| name.contains("TextBox")));
    assert!(!names.iter().any(|name| name.contains("Image")));

    let radio_field = find_field(&pdf, fields, "Radio1");
    assert_name(deref(&pdf, radio_field.get(b"FT").unwrap()), b"Btn");
    let flags = flag_bits(&pdf, radio_field);
    assert_ne!(flags & (1 << 15), 0, "radio flag");
    assert_eq!(flags & (1 << 16), 0, "not a pushbutton");
    let kids = deref(&pdf, radio_field.get(b"Kids").unwrap());
    assert_eq!(kids.as_array().unwrap().len(), 2);

    let date = find_field(&pdf, fields, "Date1");
    assert_name(deref(&pdf, date.get(b"FT").unwrap()), b"Tx");
    match deref(&pdf, date.get(b"MaxLen").unwrap()) {
        Object::Integer(len) => assert_eq!(*len, 10),
        other => panic!("MaxLen {other:?}"),
    }
    let tu = string_bytes(deref(&pdf, date.get(b"TU").unwrap()));
    assert_eq!(tu, b"YYYY-MM-DD");

    let choice = find_field(&pdf, fields, "Choice1");
    assert_name(deref(&pdf, choice.get(b"FT").unwrap()), b"Ch");
    let choice_flags = flag_bits(&pdf, choice);
    assert_ne!(choice_flags & (1 << 17), 0, "combo, not a list box");
    assert_eq!(choice_flags & (1 << 18), 0, "not an editable combo");
    assert_eq!(choice_flags & (1 << 21), 0, "not multi-select");
    let options = deref(&pdf, choice.get(b"Opt").unwrap());
    assert_eq!(options.as_array().unwrap().len(), 3);

    let sign = find_field(&pdf, fields, "Sign1");
    assert_name(deref(&pdf, sign.get(b"FT").unwrap()), b"Sig");

    let contents = stream_plain(&pdf, deref(&pdf, page_dict.get(b"Contents").unwrap()));
    let text = String::from_utf8_lossy(&contents);
    assert!(text.contains("Full name"), "static text is page content");
    assert!(text.contains("Text"), "text box caption is page content");
    assert!(text.contains("Image"), "image placeholder is page content");
    assert!(
        !text.contains("Header") && !text.contains("Footer"),
        "view chrome is not in the PDF"
    );

    doc.set_radio_labels(false);
    let hidden = export_pdf(&doc).unwrap();
    let hidden_pdf = Document::load_mem(&hidden).unwrap();
    let hidden_catalog = catalog(&hidden_pdf);
    let hidden_page = page(&hidden_pdf, hidden_catalog);
    let hidden_contents = stream_plain(
        &hidden_pdf,
        deref(&hidden_pdf, hidden_page.get(b"Contents").unwrap()),
    );
    let hidden_text = String::from_utf8_lossy(&hidden_contents);
    assert!(!hidden_text.contains("(Yes)"));
    assert!(!hidden_text.contains("(No)"));
}

#[test]
fn styled_properties_round_trip_into_the_acroform() {
    let mut doc = Form::letter();
    let text = doc.add_field(
        FieldKind::Text,
        RectPt {
            x: 72.0,
            y: 700.0,
            w: 180.0,
            h: 36.0,
        },
    );
    doc.set_name(text, "person").unwrap();
    doc.set_tooltip(text, "Full name");
    doc.set_required(text, true);
    doc.set_read_only(text, true);
    doc.set_default_value(text, "Ada");
    doc.update_style(text, |style| {
        style.max_chars = Some(20);
        style.multiline = true;
        style.align = TextAlign::Center;
        style.font = FontFamily::Times;
        style.font_size = 14.0;
        style.text_color = Rgb::new(1.0, 0.0, 0.0);
        style.border_width = 2.0;
        style.border_color = Rgb::new(0.0, 0.0, 1.0);
        style.border_style = BorderStyleKind::Dashed;
        style.fill = Some(Rgb::new(1.0, 1.0, 0.0));
    });

    let check = doc.add_field(
        FieldKind::Checkbox,
        RectPt {
            x: 72.0,
            y: 650.0,
            w: 18.0,
            h: 18.0,
        },
    );
    doc.set_checked(check, true);
    doc.update_style(check, |style| {
        style.checkbox_style = CheckboxStyle::Cross;
    });

    let choice = doc.add_field(
        FieldKind::Dropdown,
        RectPt {
            x: 72.0,
            y: 610.0,
            w: 144.0,
            h: 18.0,
        },
    );
    doc.set_options(choice, vec!["Red".into(), "Blue".into()])
        .unwrap();
    doc.set_default_value(choice, "Blue");

    let radio = doc.add_field(
        FieldKind::Radio,
        RectPt {
            x: 72.0,
            y: 560.0,
            w: 18.0,
            h: 18.0,
        },
    );
    doc.set_name(radio, "Color").unwrap();
    let other = doc.add_radio(
        RectPt {
            x: 110.0,
            y: 560.0,
            w: 18.0,
            h: 18.0,
        },
        Some("Color"),
    );
    doc.set_checked(other, true);
    doc.set_required(radio, true);

    let date = doc.add_field(
        FieldKind::Date,
        RectPt {
            x: 200.0,
            y: 560.0,
            w: 120.0,
            h: 18.0,
        },
    );
    doc.update_style(date, |style| style.date_format = "MM/DD/YYYY".to_string());

    let line = doc.add_field(
        FieldKind::Line,
        RectPt {
            x: 72.0,
            y: 500.0,
            w: 144.0,
            h: 9.0,
        },
    );
    doc.update_style(line, |style| {
        style.stroke_width = 3.0;
        style.stroke_color = Rgb::new(1.0, 0.0, 0.0);
    });
    doc.set_header_text("CANVAS-HEADER");
    doc.set_footer_text("CANVAS-FOOTER");
    doc.set_page_size(PageSize::Letter);
    doc.set_orientation(Orientation::Landscape);

    let bytes = export_pdf(&doc).unwrap();
    assert!(!bytes
        .windows(b"NeedAppearances".len())
        .any(|window| window == b"NeedAppearances"));
    let pdf = Document::load_mem(&bytes).unwrap();
    let root = catalog(&pdf);
    let page_dict = page(&pdf, root);
    let media = numbers(deref(&pdf, page_dict.get(b"MediaBox").unwrap()));
    assert_close(media[2], 792.0);
    assert_close(media[3], 612.0);

    let acro = deref(&pdf, root.get(b"AcroForm").unwrap())
        .as_dict()
        .unwrap();
    assert!(acro.get(b"NeedAppearances").is_err());
    let fields = deref(&pdf, acro.get(b"Fields").unwrap());
    let fields = fields.as_array().unwrap();

    let person = find_field(&pdf, fields, "person");
    assert_eq!(
        string_bytes(deref(&pdf, person.get(b"TU").unwrap())),
        b"Full name"
    );
    let flags = flag_bits(&pdf, person);
    assert_ne!(flags & 1, 0, "read-only");
    assert_ne!(flags & 2, 0, "required");
    assert_ne!(flags & (1 << 12), 0, "multiline");
    match deref(&pdf, person.get(b"MaxLen").unwrap()) {
        Object::Integer(len) => assert_eq!(*len, 20),
        other => panic!("MaxLen {other:?}"),
    }
    match deref(&pdf, person.get(b"Q").unwrap()) {
        Object::Integer(value) => assert_eq!(*value, 1, "center"),
        other => panic!("Q {other:?}"),
    }
    let da = string_bytes(deref(&pdf, person.get(b"DA").unwrap()));
    let da_text = String::from_utf8_lossy(da);
    assert!(da_text.contains("/TiRo"), "{da_text}");
    assert!(da_text.contains("14"), "{da_text}");
    assert!(da_text.contains("rg"), "{da_text}");
    assert_eq!(string_bytes(deref(&pdf, person.get(b"V").unwrap())), b"Ada");
    assert_eq!(
        string_bytes(deref(&pdf, person.get(b"DV").unwrap())),
        b"Ada"
    );
    let bs = deref(&pdf, person.get(b"BS").unwrap()).as_dict().unwrap();
    match deref(&pdf, bs.get(b"W").unwrap()) {
        Object::Integer(width) => assert_eq!(*width, 2),
        Object::Real(width) => assert!((width - 2.0).abs() < 0.01),
        other => panic!("border width {other:?}"),
    }
    assert_name(deref(&pdf, bs.get(b"S").unwrap()), b"D");
    let mk = deref(&pdf, person.get(b"MK").unwrap()).as_dict().unwrap();
    let border = numbers(deref(&pdf, mk.get(b"BC").unwrap()));
    assert_close(border[2], 1.0);
    let fill = numbers(deref(&pdf, mk.get(b"BG").unwrap()));
    assert_close(fill[0], 1.0);
    assert_close(fill[1], 1.0);
    assert_close(fill[2], 0.0);
    let appearance = stream_plain(&pdf, appearance_n(&pdf, person));
    let appearance_text = String::from_utf8_lossy(&appearance);
    assert!(
        appearance_text.contains("Ada"),
        "explicit appearance shows the default value"
    );

    let check_field = find_field(&pdf, fields, "Check1");
    assert_name(deref(&pdf, check_field.get(b"V").unwrap()), b"On");
    assert_name(deref(&pdf, check_field.get(b"DV").unwrap()), b"On");
    assert_name(deref(&pdf, check_field.get(b"AS").unwrap()), b"On");
    let check_mk = deref(&pdf, check_field.get(b"MK").unwrap())
        .as_dict()
        .unwrap();
    assert_eq!(
        string_bytes(deref(&pdf, check_mk.get(b"CA").unwrap())),
        b"8"
    );
    let states = appearance_n(&pdf, check_field)
        .as_dict()
        .expect("checkbox appearances");
    assert!(stream_len(&pdf, deref(&pdf, states.get(b"On").unwrap())) > 8);
    assert!(stream_len(&pdf, deref(&pdf, states.get(b"Off").unwrap())) > 8);

    let choice_field = find_field(&pdf, fields, "Choice1");
    let options = deref(&pdf, choice_field.get(b"Opt").unwrap());
    assert_eq!(options.as_array().unwrap().len(), 2);
    assert_eq!(
        string_bytes(deref(&pdf, choice_field.get(b"V").unwrap())),
        b"Blue"
    );
    assert_eq!(
        string_bytes(deref(&pdf, choice_field.get(b"DV").unwrap())),
        b"Blue"
    );

    let group = find_field(&pdf, fields, "Color");
    assert_name(deref(&pdf, group.get(b"FT").unwrap()), b"Btn");
    let radio_flags = flag_bits(&pdf, group);
    assert_ne!(radio_flags & (1 << 15), 0, "radio");
    assert_ne!(radio_flags & 2, 0, "required on the group");
    assert_eq!(
        deref(&pdf, group.get(b"Kids").unwrap())
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_name(deref(&pdf, group.get(b"V").unwrap()), b"No");
    assert_name(deref(&pdf, group.get(b"DV").unwrap()), b"No");

    let date_field = find_field(&pdf, fields, "Date1");
    assert_eq!(
        string_bytes(deref(&pdf, date_field.get(b"TU").unwrap())),
        b"MM/DD/YYYY"
    );
    match deref(&pdf, date_field.get(b"MaxLen").unwrap()) {
        Object::Integer(len) => assert_eq!(*len, 10),
        other => panic!("date MaxLen {other:?}"),
    }

    let contents = stream_plain(&pdf, deref(&pdf, page_dict.get(b"Contents").unwrap()));
    let page_text = String::from_utf8_lossy(&contents);
    assert!(
        page_text.contains("1 0 0 RG"),
        "line stroke color is in the page content: {page_text}"
    );
    assert!(
        page_text.contains("3 w"),
        "line stroke width is in the page content: {page_text}"
    );
    assert!(!page_text.contains("CANVAS-HEADER"));
    assert!(!page_text.contains("CANVAS-FOOTER"));
}

#[test]
fn inline_text_edits_export_as_page_content_or_field_value() {
    let mut doc = Form::letter();
    let label = doc.add_field(
        FieldKind::StaticText,
        RectPt {
            x: 72.0,
            y: 700.0,
            w: 180.0,
            h: 18.0,
        },
    );
    doc.set_caption(label, "Full name").unwrap();
    let text_box = doc.add_field(
        FieldKind::TextBox,
        RectPt {
            x: 72.0,
            y: 640.0,
            w: 180.0,
            h: 48.0,
        },
    );
    doc.set_caption(text_box, "Line one\nLine two").unwrap();
    let check = doc.add_field(
        FieldKind::Checkbox,
        RectPt {
            x: 72.0,
            y: 600.0,
            w: 18.0,
            h: 18.0,
        },
    );
    doc.set_caption(check, "Agree").unwrap();
    let text = doc.add_field(
        FieldKind::Text,
        RectPt {
            x: 72.0,
            y: 560.0,
            w: 144.0,
            h: 18.0,
        },
    );
    doc.set_name(text, "person").unwrap();
    doc.set_default_value(text, "Ada");

    let bytes = export_pdf(&doc).unwrap();
    let pdf = Document::load_mem(&bytes).unwrap();
    let root = catalog(&pdf);
    let page_dict = page(&pdf, root);
    let contents = stream_plain(&pdf, deref(&pdf, page_dict.get(b"Contents").unwrap()));
    let page_text = String::from_utf8_lossy(&contents);
    assert!(page_text.contains("Full name"), "{page_text}");
    assert!(page_text.contains("Line one"), "{page_text}");
    assert!(page_text.contains("Line two"), "{page_text}");
    assert!(page_text.contains("Agree"), "{page_text}");

    let acro = deref(&pdf, root.get(b"AcroForm").unwrap())
        .as_dict()
        .unwrap();
    let fields = deref(&pdf, acro.get(b"Fields").unwrap());
    let person = find_field(&pdf, fields.as_array().unwrap(), "person");
    assert_eq!(string_bytes(deref(&pdf, person.get(b"V").unwrap())), b"Ada");
    let appearance = stream_plain(&pdf, appearance_n(&pdf, person));
    assert!(String::from_utf8_lossy(&appearance).contains("Ada"));
}

fn field_names(doc: &Document, fields: &[Object]) -> Vec<String> {
    fields
        .iter()
        .map(|field| {
            let dict = deref(doc, field).as_dict().unwrap();
            let bytes = string_bytes(deref(doc, dict.get(b"T").unwrap()));
            String::from_utf8_lossy(bytes).into_owned()
        })
        .collect()
}

fn flag_bits(doc: &Document, field: &lopdf::Dictionary) -> i64 {
    match deref(doc, field.get(b"Ff").unwrap()) {
        Object::Integer(value) => *value,
        other => panic!("unexpected /Ff {other:?}"),
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
