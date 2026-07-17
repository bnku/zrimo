//! In-memory legacy Office to OOXML bridge.

mod doc_comments;
mod doc_numbering;
mod doc_tables;

use std::io::Cursor;

use legacy_doc::{DocLimits, WordBinaryDocument};
use office_oxide::{Document, DocumentFormat, create::create_from_ir_to_writer};
use wasm_bindgen::prelude::*;

use crate::doc_comments::add_comments_to_docx;
use crate::doc_numbering::add_numbering_to_docx;
use crate::doc_tables::add_table_layout_to_docx;

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
    if source_format == DocumentFormat::Doc {
        let limits = DocLimits::default();
        let document = WordBinaryDocument::from_bytes_with_limits(data, limits)
            .map_err(|error| format!("DOC container parse failed: {error}"))?;
        let comments = document
            .comments(limits)
            .map_err(|error| format!("DOC comment parse failed: {error}"))?;
        let lists = document
            .lists(limits)
            .map_err(|error| format!("DOC list parse failed: {error}"))?;
        let fonts = document
            .fonts(limits)
            .map_err(|error| format!("DOC font parse failed: {error}"))?;
        let tables = document
            .tables(limits)
            .map_err(|error| format!("DOC table parse failed: {error}"))?;
        let ir = document
            .to_ooxml_ir(limits)
            .map_err(|error| format!("DOC semantic projection failed: {error}"))?;
        let mut output = Cursor::new(Vec::new());
        create_from_ir_to_writer(&ir, DocumentFormat::Docx, &mut output)
            .map_err(|error| error.to_string())?;
        let output = add_comments_to_docx(output.into_inner(), &document, &comments)?;
        let output = add_numbering_to_docx(output, &lists, &fonts)?;
        return add_table_layout_to_docx(output, &document, &tables);
    }
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
