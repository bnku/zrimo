#![no_main]

use libfuzzer_sys::fuzz_target;
use pdf_oxide::PdfDocument;

fuzz_target!(|data: &[u8]| {
    if data.len() > 2 * 1024 * 1024 {
        return;
    }
    if let Ok(document) = PdfDocument::from_bytes(data.to_vec()) {
        let _ = document.page_count();
    }
});
