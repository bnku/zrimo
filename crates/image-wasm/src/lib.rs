//! TIFF-only fallback decoder for browser image formats.

use std::io::Cursor;

use image::{DynamicImage, ImageBuffer, ImageFormat, Luma, LumaA, Rgb, Rgba};
use tiff::{ColorType, decoder::DecodingResult};
use wasm_bindgen::prelude::*;

struct TiffPage {
    width: u32,
    height: u32,
    png: Vec<u8>,
}

/// Parsed, bounded multi-page TIFF document.
#[wasm_bindgen]
pub struct TiffViewerDocument {
    pages: Vec<TiffPage>,
}

#[wasm_bindgen]
impl TiffViewerDocument {
    /// Decode all TIFF IFDs and retain compressed PNG pages.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed/unsupported TIFF data or a pixel-limit breach.
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8], max_decoded_pixels: u32) -> Result<Self, JsValue> {
        decode_tiff(data, u64::from(max_decoded_pixels))
            .map(|pages| Self { pages })
            .map_err(|message| JsValue::from_str(&message))
    }

    /// Number of image directories/pages.
    #[wasm_bindgen(js_name = pageCount)]
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Width of one page in pixels.
    #[wasm_bindgen(js_name = pageWidth)]
    #[must_use]
    pub fn page_width(&self, page_index: usize) -> u32 {
        self.pages.get(page_index).map_or(0, |page| page.width)
    }

    /// Height of one page in pixels.
    #[wasm_bindgen(js_name = pageHeight)]
    #[must_use]
    pub fn page_height(&self, page_index: usize) -> u32 {
        self.pages.get(page_index).map_or(0, |page| page.height)
    }

    /// Return a decoded page as PNG bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when `page_index` is outside the document.
    #[wasm_bindgen(js_name = renderPagePng)]
    pub fn render_page_png(&self, page_index: usize) -> Result<Vec<u8>, JsValue> {
        self.pages
            .get(page_index)
            .map(|page| page.png.clone())
            .ok_or_else(|| JsValue::from_str("TIFF page index out of range"))
    }
}

fn decode_tiff(data: &[u8], max_pixels: u64) -> Result<Vec<TiffPage>, String> {
    let mut decoder = tiff::decoder::Decoder::new(Cursor::new(data)).map_err(error_message)?;
    let mut pages = Vec::new();
    let mut total_pixels = 0_u64;
    loop {
        let (width, height) = decoder.dimensions().map_err(error_message)?;
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| "TIFF dimensions overflow".to_owned())?;
        total_pixels = total_pixels
            .checked_add(pixels)
            .ok_or_else(|| "TIFF page pixels overflow".to_owned())?;
        if total_pixels > max_pixels {
            return Err(format!(
                "TIFF decoded pixels {total_pixels} exceed limit {max_pixels}"
            ));
        }
        let color = decoder.colortype().map_err(error_message)?;
        let samples = decoder.read_image().map_err(error_message)?;
        let image = dynamic_image(width, height, color, samples)?;
        let mut png = Cursor::new(Vec::new());
        image
            .write_to(&mut png, ImageFormat::Png)
            .map_err(error_message)?;
        pages.push(TiffPage {
            width,
            height,
            png: png.into_inner(),
        });
        if !decoder.more_images() {
            break;
        }
        decoder.next_image().map_err(error_message)?;
    }
    if pages.is_empty() {
        return Err("TIFF contains no images".to_owned());
    }
    Ok(pages)
}

/// Exercise the bounded native TIFF parser without constructing JavaScript values.
///
/// This entry point exists for native fuzzing and security regression tests.
///
/// # Errors
///
/// Returns the same bounded parse/decode error as the browser constructor.
pub fn fuzz_decode_tiff(data: &[u8], max_pixels: u64) -> Result<usize, String> {
    decode_tiff(data, max_pixels).map(|pages| pages.len())
}

#[allow(clippy::too_many_lines)]
fn dynamic_image(
    width: u32,
    height: u32,
    color: ColorType,
    decoded: DecodingResult,
) -> Result<DynamicImage, String> {
    macro_rules! image_from {
        ($data:expr, $pixel:ty, $variant:ident) => {{
            let buffer = ImageBuffer::<$pixel, _>::from_raw(width, height, $data)
                .ok_or_else(|| "TIFF sample count does not match dimensions".to_owned())?;
            Ok(DynamicImage::$variant(buffer))
        }};
    }
    match (color, decoded) {
        (ColorType::Gray(8), DecodingResult::U8(data)) => image_from!(data, Luma<u8>, ImageLuma8),
        (ColorType::GrayA(8), DecodingResult::U8(data)) => {
            image_from!(data, LumaA<u8>, ImageLumaA8)
        }
        (ColorType::RGB(8), DecodingResult::U8(data)) => image_from!(data, Rgb<u8>, ImageRgb8),
        (ColorType::RGBA(8), DecodingResult::U8(data)) => image_from!(data, Rgba<u8>, ImageRgba8),
        (ColorType::Gray(16), DecodingResult::U16(data)) => {
            image_from!(data, Luma<u16>, ImageLuma16)
        }
        (ColorType::GrayA(16), DecodingResult::U16(data)) => {
            image_from!(data, LumaA<u16>, ImageLumaA16)
        }
        (ColorType::RGB(16), DecodingResult::U16(data)) => image_from!(data, Rgb<u16>, ImageRgb16),
        (ColorType::RGBA(16), DecodingResult::U16(data)) => {
            image_from!(data, Rgba<u16>, ImageRgba16)
        }
        (ColorType::CMYK(8), DecodingResult::U8(data)) => {
            let rgba = cmyk_to_rgba(&data, false)?;
            image_from!(rgba, Rgba<u8>, ImageRgba8)
        }
        (ColorType::CMYKA(8), DecodingResult::U8(data)) => {
            let rgba = cmyk_to_rgba(&data, true)?;
            image_from!(rgba, Rgba<u8>, ImageRgba8)
        }
        (kind, _) => Err(format!("unsupported TIFF color type: {kind:?}")),
    }
}

fn cmyk_to_rgba(data: &[u8], has_alpha: bool) -> Result<Vec<u8>, String> {
    let stride = if has_alpha { 5 } else { 4 };
    if !data.len().is_multiple_of(stride) {
        return Err("invalid TIFF CMYK sample count".to_owned());
    }
    let mut output = Vec::with_capacity(data.len() / stride * 4);
    for pixel in data.chunks_exact(stride) {
        let c = u16::from(pixel[0]);
        let m = u16::from(pixel[1]);
        let y = u16::from(pixel[2]);
        let k = u16::from(pixel[3]);
        output.extend_from_slice(&[
            cmyk_component(c, k),
            cmyk_component(m, k),
            cmyk_component(y, k),
            if has_alpha { pixel[4] } else { 255 },
        ]);
    }
    Ok(output)
}

fn cmyk_component(color: u16, black: u16) -> u8 {
    u8::try_from(255_u16.saturating_sub((color + black).min(255))).unwrap_or(0)
}

fn error_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use tiff::encoder::{TiffEncoder, colortype::RGB8};

    use super::decode_tiff;

    #[test]
    fn decodes_multi_page_tiff_to_png_pages() {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes).expect("encoder");
            encoder
                .write_image::<RGB8>(1, 1, &[255, 0, 0])
                .expect("first page");
            encoder
                .write_image::<RGB8>(2, 1, &[0, 255, 0, 0, 0, 255])
                .expect("second page");
        }

        let pages = decode_tiff(&bytes.into_inner(), 10).expect("decode");
        assert_eq!(pages.len(), 2);
        assert_eq!((pages[1].width, pages[1].height), (2, 1));
        assert!(pages[0].png.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn enforces_aggregate_pixel_limit() {
        let mut bytes = Cursor::new(Vec::new());
        TiffEncoder::new(&mut bytes)
            .expect("encoder")
            .write_image::<RGB8>(2, 2, &[0; 12])
            .expect("page");
        assert!(decode_tiff(&bytes.into_inner(), 3).is_err());
    }
}
