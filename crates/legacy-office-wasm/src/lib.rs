//! In-memory legacy Office to OOXML bridge.

use std::io::Cursor;

use office_oxide::{Document, DocumentFormat, create::create_from_ir_to_writer};
use wasm_bindgen::prelude::*;

fn formats(format: &str) -> Result<(DocumentFormat, DocumentFormat), String> {
    match format.to_ascii_lowercase().as_str() {
        "doc" => Ok((DocumentFormat::Doc, DocumentFormat::Docx)),
        "xls" => Ok((DocumentFormat::Xls, DocumentFormat::Xlsx)),
        "ppt" => Ok((DocumentFormat::Ppt, DocumentFormat::Pptx)),
        _ => Err(format!("unsupported legacy Office format: {format}")),
    }
}

/// Convert a legacy Office buffer to OOXML without touching the file system.
///
/// # Errors
///
/// Returns an error when the format is unsupported, parsing fails, or serialization fails.
pub fn convert_legacy_bytes(data: &[u8], format: &str) -> Result<Vec<u8>, String> {
    let (source_format, target_format) = formats(format)?;
    let document = Document::from_reader(Cursor::new(data.to_vec()), source_format)
        .map_err(|error| error.to_string())?;
    let ir = document.to_ir();
    let mut output = Cursor::new(Vec::new());
    create_from_ir_to_writer(&ir, target_format, &mut output).map_err(|error| error.to_string())?;
    Ok(output.into_inner())
}

/// Convert DOC/XLS/PPT bytes to the corresponding OOXML package in memory.
///
/// # Errors
///
/// Returns a JavaScript error when the format is unsupported, parsing fails, or OOXML serialization fails.
#[wasm_bindgen(js_name = convertLegacyToOoxml)]
pub fn convert_legacy_to_ooxml(data: &[u8], format: &str) -> Result<Vec<u8>, JsValue> {
    convert_legacy_bytes(data, format).map_err(|message| JsValue::from_str(&message))
}

/// Extract plain text from a legacy Office document for diagnostics and search fallback.
///
/// # Errors
///
/// Returns a JavaScript error when the format is unsupported or the binary document cannot be parsed.
#[wasm_bindgen(js_name = extractLegacyPlainText)]
pub fn extract_legacy_plain_text(data: &[u8], format: &str) -> Result<String, JsValue> {
    let (source_format, _) = formats(format).map_err(|message| JsValue::from_str(&message))?;
    let document = Document::from_reader(Cursor::new(data.to_vec()), source_format)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(document.plain_text())
}

#[cfg(test)]
mod tests {
    use super::formats;

    #[test]
    fn maps_all_legacy_formats() {
        assert!(formats("DOC").is_ok());
        assert!(formats("xls").is_ok());
        assert!(formats("ppt").is_ok());
        assert!(formats("docx").is_err());
    }
}
