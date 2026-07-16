//! Small viewer-oriented binding over `pdf_oxide`.

use pdf_oxide::{PdfDocument, rendering};
use wasm_bindgen::prelude::*;

/// Parsed PDF handle used by the TypeScript adapter.
#[wasm_bindgen]
pub struct PdfViewerDocument {
    document: PdfDocument,
}

#[wasm_bindgen]
impl PdfViewerDocument {
    /// Parse an unencrypted PDF from memory.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the bytes are not a valid supported PDF.
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8]) -> Result<Self, JsValue> {
        let document = PdfDocument::from_bytes(data.to_vec())
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(Self { document })
    }

    /// Number of pages in the document.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the page tree is invalid.
    #[wasm_bindgen(js_name = pageCount)]
    pub fn page_count(&self) -> Result<usize, JsValue> {
        self.document
            .page_count()
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Render one page as PNG bytes.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the page does not exist or cannot be rendered.
    #[wasm_bindgen(js_name = renderPagePng)]
    pub fn render_page_png(&self, page_index: usize, dpi: Option<u32>) -> Result<Vec<u8>, JsValue> {
        let options = rendering::RenderOptions::with_dpi(dpi.unwrap_or(144));
        let image = rendering::render_page(&self.document, page_index, &options)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(image.data)
    }

    /// Return positioned page text as JSON for the selectable text layer.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when text extraction or serialization fails.
    #[wasm_bindgen(js_name = pageTextJson)]
    pub fn page_text_json(&self, page_index: usize) -> Result<String, JsValue> {
        let page = self
            .document
            .extract_page_text(page_index)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        serde_json::to_string(&page).map_err(|error| JsValue::from_str(&error.to_string()))
    }
}
