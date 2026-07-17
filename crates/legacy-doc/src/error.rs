//! Parser errors with stable, non-panicking failure modes.

use thiserror::Error;

/// Result returned by the Word Binary parser.
pub type Result<T> = std::result::Result<T, DocError>;

/// Errors produced while opening or parsing a legacy DOC file.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DocError {
    /// Input exceeds the configured byte budget.
    #[error("DOC input is {actual} bytes; limit is {limit} bytes")]
    InputTooLarge {
        /// Actual input size.
        actual: usize,
        /// Configured maximum.
        limit: usize,
    },
    /// A CFB stream exceeds the configured byte budget.
    #[error("DOC stream {stream} is {actual} bytes; limit is {limit} bytes")]
    StreamTooLarge {
        /// Stream name.
        stream: &'static str,
        /// Actual stream size.
        actual: usize,
        /// Configured maximum.
        limit: usize,
    },
    /// A required CFB stream is absent.
    #[error("required DOC stream is missing: {0}")]
    MissingStream(&'static str),
    /// The compound-file reader rejected the container.
    #[error("invalid DOC compound file: {0}")]
    CompoundFile(String),
    /// A fixed or variable FIB field is invalid.
    #[error("invalid DOC FIB: {0}")]
    InvalidFib(String),
    /// The file is an older or unknown Word Binary revision.
    #[error("unsupported Word Binary revision: 0x{0:04X}")]
    UnsupportedVersion(u16),
    /// Encrypted and obfuscated documents are not parsed without a password API.
    #[error("password-protected DOC documents are not supported")]
    PasswordProtected,
    /// CLX/Pcdt/PlcPcd data is malformed.
    #[error("invalid DOC piece table: {0}")]
    InvalidPieceTable(String),
    /// BTE PLCF, PAPX FKP, or CHPX FKP data is malformed.
    #[error("invalid DOC formatting table: {0}")]
    InvalidFormatting(String),
    /// STSH, STD, UPX, or style inheritance data is malformed.
    #[error("invalid DOC stylesheet: {0}")]
    InvalidStyle(String),
    /// `SttbfFfn` or an FFN font record is malformed.
    #[error("invalid DOC font table: {0}")]
    InvalidFont(String),
    /// Table markers, row properties, or cell definitions are inconsistent.
    #[error("invalid DOC table structure: {0}")]
    InvalidTable(String),
    /// `PlcfHdd` boundaries or section header/footer linkage is malformed.
    #[error("invalid DOC header/footer table: {0}")]
    InvalidHeaderFooter(String),
    /// PICF, `OfficeArt`, or source picture-anchor data is malformed.
    #[error("invalid DOC media data: {0}")]
    InvalidMedia(String),
    /// Footnote/endnote reference or text-boundary PLC data is malformed.
    #[error("invalid DOC note table: {0}")]
    InvalidNote(String),
    /// A story field PLC or its nested begin/separator/end sequence is malformed.
    #[error("invalid DOC field table: {0}")]
    InvalidField(String),
    /// Comment reference/body PLC or metadata is malformed.
    #[error("invalid DOC comment table: {0}")]
    InvalidComment(String),
    /// `PlfLst`, `PlfLfo`, or a referenced list level is malformed.
    #[error("invalid DOC list table: {0}")]
    InvalidList(String),
    /// PLCFSED, SED, SEPX, or a section property is malformed.
    #[error("invalid DOC section table: {0}")]
    InvalidSection(String),
    /// A declared structure points outside its containing stream.
    #[error("{structure} range [{offset}, {end}) exceeds containing stream length {available}")]
    OutOfBounds {
        /// Structure being read.
        structure: &'static str,
        /// Requested byte offset.
        offset: usize,
        /// Requested exclusive byte end.
        end: usize,
        /// Available bytes.
        available: usize,
    },
    /// A configured count or character budget was exceeded.
    #[error("DOC {resource} count {actual} exceeds limit {limit}")]
    ResourceLimit {
        /// Resource name.
        resource: &'static str,
        /// Actual count.
        actual: u64,
        /// Configured maximum.
        limit: u64,
    },
}
