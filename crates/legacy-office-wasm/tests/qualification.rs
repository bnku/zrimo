use std::{fs, io::Cursor, path::PathBuf};

use legacy_office_wasm::convert_legacy_bytes;
use office_oxide::{Document, DocumentFormat};

#[test]
#[ignore = "requires npm run corpus:fetch"]
fn converts_doc_xls_and_ppt_to_valid_ooxml_bytes() {
    let corpus = PathBuf::from(std::env::var("CORPUS_DIR").expect("CORPUS_DIR is required"));
    let cases = [
        ("word6.doc", "doc", DocumentFormat::Docx),
        ("simple.xls", "xls", DocumentFormat::Xlsx),
        ("basic.ppt", "ppt", DocumentFormat::Pptx),
    ];

    for (file_name, source_format, target_format) in cases {
        let input = fs::read(corpus.join(file_name)).expect("fixture must be readable");
        let output = convert_legacy_bytes(&input, source_format).expect("conversion must succeed");
        assert!(
            output.starts_with(b"PK\x03\x04"),
            "{file_name} output is not an OOXML ZIP"
        );
        assert!(
            output.len() > 500,
            "{file_name} output is unexpectedly small"
        );
        Document::from_reader(Cursor::new(output), target_format)
            .expect("output must parse as OOXML");
    }
}
