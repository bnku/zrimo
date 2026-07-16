use std::{fs, path::PathBuf};

use pdf_oxide::{PdfDocument, rendering};

#[test]
#[ignore = "requires npm run corpus:fetch"]
fn renders_pdf_and_extracts_positioned_text() {
    let corpus = PathBuf::from(std::env::var("CORPUS_DIR").expect("CORPUS_DIR is required"));
    let input = fs::read(corpus.join("hello.pdf")).expect("fixture must be readable");
    let document = PdfDocument::from_bytes(input).expect("PDF must parse");

    assert_eq!(document.page_count().expect("page tree must parse"), 1);
    let page_text = document
        .extract_page_text(0)
        .expect("text extraction must succeed");
    assert!(
        !page_text.chars.is_empty(),
        "positioned text map must not be empty"
    );

    let image = rendering::render_page(&document, 0, &rendering::RenderOptions::with_dpi(72))
        .expect("bitmap render must succeed");
    assert!(image.width > 0 && image.height > 0);
    assert!(
        image.data.starts_with(b"\x89PNG\r\n\x1a\n"),
        "renderer must return PNG bytes"
    );
}
