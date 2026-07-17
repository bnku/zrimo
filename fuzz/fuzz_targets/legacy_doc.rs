#![no_main]

use legacy_doc::{DocLimits, WordBinaryDocument, decode_grpprl};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > 2 * 1024 * 1024 {
        return;
    }

    let _ = decode_grpprl(data);
    if let Ok(document) = WordBinaryDocument::from_bytes(data) {
        let limits = DocLimits::default();
        let _ = document.formatting_index(limits);
        let _ = document.logical_formatting(limits);
        let _ = document.semantic_formatting(limits);
        let _ = document.styles(limits);
        let _ = document.styled_formatting(limits);
        let _ = document.fonts(limits);
        let _ = document.sections(limits);
        let _ = document.tables(limits);
        let _ = document.header_footers(limits);
        let _ = document.media(limits);
        let _ = document.notes(limits);
        let _ = document.comments(limits);
        let _ = document.fields(limits);
        let _ = document.to_ooxml_ir(limits);
    }

    if data.len() < 3 {
        return;
    }
    let split_seed = usize::from(u16::from_le_bytes([data[0], data[1]]));
    let payload = &data[2..];
    let split = split_seed % (payload.len() + 1);
    let table_stream_name = if data[0] & 1 == 0 {
        "0Table"
    } else {
        "1Table"
    };
    let _ = WordBinaryDocument::from_streams(
        payload[..split].to_vec(),
        payload[split..].to_vec(),
        None,
        table_stream_name,
        DocLimits::default(),
    );
});
