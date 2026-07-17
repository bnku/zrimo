//! Format-neutral contracts shared by the Zrimo WASM adapters.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// File formats accepted by the public viewer API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentFormat {
    /// Word Open XML.
    Docx,
    /// Excel Open XML.
    Xlsx,
    /// `PowerPoint` Open XML.
    Pptx,
    /// Legacy Word binary.
    Doc,
    /// Legacy Excel binary.
    Xls,
    /// Legacy `PowerPoint` binary.
    Ppt,
    /// Portable Document Format.
    Pdf,
    /// PNG image.
    Png,
    /// JPEG image.
    Jpeg,
    /// GIF image.
    Gif,
    /// WebP image.
    Webp,
    /// SVG image.
    Svg,
    /// BMP image.
    Bmp,
    /// TIFF image.
    Tiff,
}

impl DocumentFormat {
    /// Parse a case-insensitive extension with or without a leading dot.
    #[must_use]
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "docx" => Some(Self::Docx),
            "xlsx" => Some(Self::Xlsx),
            "pptx" => Some(Self::Pptx),
            "doc" => Some(Self::Doc),
            "xls" => Some(Self::Xls),
            "ppt" => Some(Self::Ppt),
            "pdf" => Some(Self::Pdf),
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "gif" => Some(Self::Gif),
            "webp" => Some(Self::Webp),
            "svg" => Some(Self::Svg),
            "bmp" => Some(Self::Bmp),
            "tif" | "tiff" => Some(Self::Tiff),
            _ => None,
        }
    }

    /// Whether the format requires the legacy-to-OOXML conversion adapter.
    #[must_use]
    pub const fn is_legacy_office(self) -> bool {
        matches!(self, Self::Doc | Self::Xls | Self::Ppt)
    }
}

/// Stable error codes crossing the Rust/TypeScript boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewerErrorCode {
    /// File format cannot be detected or is not supported.
    UnsupportedFormat,
    /// Input is malformed or corrupt.
    InvalidDocument,
    /// Document is encrypted or password protected.
    PasswordProtected,
    /// Rendering failed after parsing succeeded.
    RenderFailed,
    /// Operation was cancelled by the host.
    Cancelled,
    /// Adapter failed for another reason.
    Internal,
}

/// Internal error carrying a stable code and safe message.
#[derive(Debug, Error)]
#[error("{code:?}: {message}")]
pub struct ViewerError {
    /// Machine-readable code.
    pub code: ViewerErrorCode,
    /// Human-readable diagnostic.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::DocumentFormat;

    #[test]
    fn parses_aliases_case_insensitively() {
        assert_eq!(
            DocumentFormat::from_extension(".JPG"),
            Some(DocumentFormat::Jpeg)
        );
        assert_eq!(
            DocumentFormat::from_extension("tif"),
            Some(DocumentFormat::Tiff)
        );
        assert_eq!(DocumentFormat::from_extension("unknown"), None);
    }
}
