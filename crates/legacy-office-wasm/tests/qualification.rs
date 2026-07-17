use std::{
    fs,
    io::{Cursor, Read},
    path::PathBuf,
};

use legacy_office_wasm::convert_legacy_bytes;
use office_oxide::{Document, DocumentFormat};
use zip::ZipArchive;

#[test]
#[ignore = "requires npm run corpus:fetch"]
fn converts_word97_xls_and_ppt_to_valid_ooxml_bytes() {
    let corpus = PathBuf::from(std::env::var("CORPUS_DIR").expect("CORPUS_DIR is required"));

    let cases = [
        ("word97-simple.doc", "doc", DocumentFormat::Docx),
        ("word97-simple-table.doc", "doc", DocumentFormat::Docx),
        (
            "word97-header-footer-unicode.doc",
            "doc",
            DocumentFormat::Docx,
        ),
        ("word97-footnote.doc", "doc", DocumentFormat::Docx),
        ("word97-comments.doc", "doc", DocumentFormat::Docx),
        ("word97-ranged-comment.doc", "doc", DocumentFormat::Docx),
        ("word2000-bug46817.doc", "doc", DocumentFormat::Docx),
        ("word2003-test2.doc", "doc", DocumentFormat::Docx),
        ("simple.xls", "xls", DocumentFormat::Xlsx),
        ("basic.ppt", "ppt", DocumentFormat::Pptx),
    ];

    for (file_name, source_format, target_format) in cases {
        let input = fs::read(corpus.join(file_name)).expect("fixture must be readable");
        let output = convert_legacy_bytes(&input, source_format)
            .unwrap_or_else(|error| panic!("conversion of {file_name} must succeed: {error}"));
        assert!(
            output.starts_with(b"PK\x03\x04"),
            "{file_name} output is not an OOXML ZIP"
        );
        assert!(
            output.len() > 500,
            "{file_name} output is unexpectedly small"
        );
        if file_name == "word97-footnote.doc" {
            assert_docx_comments(&output, 1, "TestComment");
        } else if file_name == "word97-simple-table.doc" {
            assert_docx_fixed_tables(&output, 1);
        } else if file_name == "word97-comments.doc" {
            assert_docx_comments(&output, 3, "Who are the Project Managers?");
            assert_docx_numbering(&output, 82, 10);
        } else if file_name == "word97-ranged-comment.doc" {
            assert_docx_ranged_comment(&output, "This is a comment.");
        }
        Document::from_reader(Cursor::new(output), target_format)
            .expect("output must parse as OOXML");
    }
}

fn assert_docx_fixed_tables(docx: &[u8], expected: usize) {
    let mut archive = ZipArchive::new(Cursor::new(docx)).expect("DOCX ZIP must open");
    let document = read_zip_text(&mut archive, "word/document.xml");
    assert_eq!(
        document.matches("<w:tblLayout w:type=\"fixed\"/>").count(),
        expected
    );
}

fn assert_docx_ranged_comment(docx: &[u8], expected_text: &str) {
    let mut archive = ZipArchive::new(Cursor::new(docx)).expect("DOCX ZIP must open");
    let document = read_zip_text(&mut archive, "word/document.xml");
    let comments = read_zip_text(&mut archive, "word/comments.xml");

    assert_eq!(document.matches("<w:commentRangeStart ").count(), 1);
    assert_eq!(document.matches("<w:commentRangeEnd ").count(), 1);
    assert_eq!(document.matches("<w:commentReference ").count(), 1);
    assert!(!document.contains("ZRIMO_COMMENT"));
    assert_eq!(comments.matches("<w:comment ").count(), 1);
    assert!(comments.contains(expected_text));
}

fn assert_docx_numbering(docx: &[u8], expected_paragraphs: usize, expected_instances: usize) {
    let mut archive = ZipArchive::new(Cursor::new(docx)).expect("DOCX ZIP must open");
    let document = read_zip_text(&mut archive, "word/document.xml");
    let numbering = read_zip_text(&mut archive, "word/numbering.xml");
    let relationships = read_zip_text(&mut archive, "word/_rels/document.xml.rels");
    let content_types = read_zip_text(&mut archive, "[Content_Types].xml");

    assert_eq!(document.matches("<w:numPr>").count(), expected_paragraphs);
    assert!(!document.contains("ZRIMO_LIST"));
    assert_eq!(
        numbering.matches("<w:abstractNum ").count(),
        expected_instances
    );
    assert_eq!(
        numbering.matches("<w:num w:numId=").count(),
        expected_instances
    );
    assert!(numbering.contains("<w:numFmt w:val=\"decimal\"/>"));
    assert!(numbering.contains("<w:numFmt w:val=\"bullet\"/>"));
    assert!(relationships.contains("relationships/numbering"));
    assert!(content_types.contains("wordprocessingml.numbering+xml"));
}

fn assert_docx_comments(docx: &[u8], expected: usize, expected_text: &str) {
    let mut archive = ZipArchive::new(Cursor::new(docx)).expect("DOCX ZIP must open");
    let document = read_zip_text(&mut archive, "word/document.xml");
    let comments = read_zip_text(&mut archive, "word/comments.xml");
    let relationships = read_zip_text(&mut archive, "word/_rels/document.xml.rels");
    let content_types = read_zip_text(&mut archive, "[Content_Types].xml");
    assert_eq!(document.matches("<w:commentReference ").count(), expected);
    assert!(!document.contains("ZRIMO_COMMENT"));
    assert_eq!(comments.matches("<w:comment ").count(), expected);
    assert!(comments.contains(expected_text));
    assert!(relationships.contains("relationships/comments"));
    assert!(content_types.contains("wordprocessingml.comments+xml"));
}

fn read_zip_text<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>, name: &str) -> String {
    let mut text = String::new();
    archive
        .by_name(name)
        .unwrap_or_else(|_| panic!("missing DOCX part {name}"))
        .read_to_string(&mut text)
        .unwrap_or_else(|_| panic!("DOCX part {name} must be UTF-8"));
    text
}
