//! Structural checks for the sample AcroForm. `lopdf` is a test-only reader.

use lopdf::{Document, Object};
use pageform_core::{
    export_pdf, Document as Form, FieldKind, RectPt, SAMPLE_CHECK_NAME, SAMPLE_CHECK_RECT,
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

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
