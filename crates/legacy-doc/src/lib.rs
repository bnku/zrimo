//! Bounded parser for the Microsoft Word Binary File Format (`.doc`).
//!
//! The crate intentionally owns Word-specific parsing instead of using
//! `office_oxide`'s lossy DOC-to-text projection. It reuses only the permissive
//! CFB container reader; parsed structures retain source character positions so
//! formatting, sections, and tables can be layered on without heuristics.

mod binary;
mod comments;
mod document;
mod error;
mod fib;
mod fields;
mod fonts;
mod formatting;
mod headers;
mod limits;
mod lists;
mod media;
mod notes;
mod ooxml_ir;
mod piece_table;
mod properties;
mod sections;
mod sprm;
mod styles;
mod tables;

pub use comments::{CommentCollection, SourceComment};
pub use document::{Story, StoryKind, WordBinaryDocument};
pub use error::{DocError, Result};
pub use fib::{FcLcb, Fib, FibBase, FibLocations, StoryLengths};
pub use fields::{Field, FieldCollection};
pub use fonts::{FontEntry, FontTable};
pub use formatting::{
    FormattingIndex, FormattingRun, LogicalFormattingIndex, LogicalFormattingRun,
    SemanticCharacterRun, SemanticFormattingIndex, SemanticParagraphRun, StyledCharacterRun,
    StyledFormattingIndex, StyledParagraphRun,
};
pub use headers::{
    HeaderFooterKind, HeaderFooterLink, HeaderFooterStory, HeaderFooterTable, SectionHeaderFooters,
};
pub use limits::DocLimits;
pub use lists::{
    ListCollection, ListDefinition, ListFollow, ListLevel, ListLevelOverride, ListOverride,
    ResolvedListParagraph,
};
pub use media::{
    BlipFormat, BlipImage, InlinePicture, MediaCollection, PictureGeometry, PictureStorage,
};
pub use notes::{NoteCollection, NoteKind, NoteReference, SourceNote};
pub use ooxml_ir::build_ooxml_ir;
pub use piece_table::{DecodedText, PieceEncoding, PiecePropertyModifier, PieceTable, TextPiece};
pub use properties::{
    CharacterPropertyDelta, LineSpacing, ParagraphPropertyDelta, TabChange, TabStop, ToggleValue,
    VerticalAlignment, apply_character_sprms, apply_paragraph_sprms,
};
pub use sections::{PageOrientation, Section, SectionGeometry, SectionTable};
pub use sprm::{PropertyGroup, Sprm, decode_grpprl};
pub use styles::{
    DefaultStyleFonts, InheritedStyleProperties, StyleDefinition, StyleKind, StyleSheet,
};
pub use tables::{
    CellFormat, CellMargins, HorizontalMerge, Table, TableCell, TableCollection,
    TablePropertyDelta, TableRow, TableRowDefinition, VerticalMerge, apply_table_sprms,
};
