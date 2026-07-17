//! High-level CFB/FIB/CLX document assembly.

use std::io::Cursor;

use office_oxide::cfb::CfbReader;

use crate::{
    CommentCollection, DecodedText, DocError, DocLimits, Fib, FieldCollection, FontTable,
    FormattingIndex, ListCollection, LogicalFormattingIndex, NoteCollection, PieceTable, Result,
    SectionTable, SemanticFormattingIndex, StyleSheet, StyledFormattingIndex,
    binary::checked_slice,
};

/// Logical Word subdocument stored in the shared CP stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StoryKind {
    /// Main document body.
    Main,
    /// Footnote bodies.
    Footnotes,
    /// Headers and footers.
    Headers,
    /// Comment bodies.
    Comments,
    /// Endnote bodies.
    Endnotes,
    /// Textboxes anchored in the main document.
    Textboxes,
    /// Textboxes anchored in headers.
    HeaderTextboxes,
}

/// One decoded story retaining its global source CP range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Story {
    /// Story kind.
    pub kind: StoryKind,
    /// Global source CP start.
    pub cp_start: u32,
    /// Global source CP end.
    pub cp_end: u32,
    /// Source-aligned decoded text.
    pub content: DecodedText,
}

/// Parsed Word 97–2003 binary document foundation.
#[derive(Debug, Clone)]
pub struct WordBinaryDocument {
    fib: Fib,
    piece_table: PieceTable,
    stories: Vec<Story>,
    word_document: Vec<u8>,
    table_stream: Vec<u8>,
    data_stream: Option<Vec<u8>>,
    table_stream_name: &'static str,
}

impl WordBinaryDocument {
    /// Parse DOC bytes from an OLE/CFB container using default limits.
    ///
    /// # Errors
    ///
    /// Returns a typed [`DocError`] when the compound file or Word binary
    /// streams are unsupported, malformed, encrypted, or exceed a limit.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Self::from_bytes_with_limits(data, DocLimits::default())
    }

    /// Parse DOC bytes from an OLE/CFB container using explicit limits.
    ///
    /// # Errors
    ///
    /// Returns a typed [`DocError`] when the compound file or Word binary
    /// streams are unsupported, malformed, encrypted, or exceed a limit.
    pub fn from_bytes_with_limits(data: &[u8], limits: DocLimits) -> Result<Self> {
        if data.len() > limits.max_input_bytes {
            return Err(DocError::InputTooLarge {
                actual: data.len(),
                limit: limits.max_input_bytes,
            });
        }
        let mut cfb = CfbReader::new(Cursor::new(data))
            .map_err(|error| DocError::CompoundFile(error.to_string()))?;
        let word_document = read_required_stream(&mut cfb, "WordDocument", limits)?;
        let fib = Fib::parse(&word_document, limits)?;
        let table_stream_name = if fib.base.use_table1 {
            "1Table"
        } else {
            "0Table"
        };
        let table_stream = read_required_stream(&mut cfb, table_stream_name, limits)?;
        let data_stream = if cfb.has_stream("Data") {
            Some(read_required_stream(&mut cfb, "Data", limits)?)
        } else {
            None
        };
        Self::from_streams(
            word_document,
            table_stream,
            data_stream,
            table_stream_name,
            limits,
        )
    }

    /// Assemble a document from already extracted CFB streams.
    ///
    /// This entry point supports deterministic byte-level tests and fuzzing
    /// without requiring every generated case to construct an OLE container.
    ///
    /// # Errors
    ///
    /// Returns a typed [`DocError`] when the streams are unsupported, malformed,
    /// inconsistent, or exceed a limit.
    pub fn from_streams(
        word_document: Vec<u8>,
        table_stream: Vec<u8>,
        data_stream: Option<Vec<u8>>,
        table_stream_name: &'static str,
        limits: DocLimits,
    ) -> Result<Self> {
        enforce_stream_limit("WordDocument", word_document.len(), limits)?;
        enforce_stream_limit(table_stream_name, table_stream.len(), limits)?;
        if let Some(data) = &data_stream {
            enforce_stream_limit("Data", data.len(), limits)?;
        }

        let fib = Fib::parse(&word_document, limits)?;
        let expected_table_stream_name = if fib.base.use_table1 {
            "1Table"
        } else {
            "0Table"
        };
        if table_stream_name != expected_table_stream_name {
            return Err(DocError::InvalidFib(format!(
                "FIB selects {expected_table_stream_name}, got {table_stream_name}"
            )));
        }
        let meaningful_word_bytes =
            usize::try_from(fib.word_document_length).map_err(|_| DocError::OutOfBounds {
                structure: "FibRgLw.cbMac",
                offset: 0,
                end: usize::MAX,
                available: word_document.len(),
            })?;
        if meaningful_word_bytes < fib.byte_length || meaningful_word_bytes > word_document.len() {
            return Err(DocError::OutOfBounds {
                structure: "FibRgLw.cbMac",
                offset: 0,
                end: meaningful_word_bytes,
                available: word_document.len(),
            });
        }
        let clx_location = fib
            .locations
            .clx()
            .filter(|location| !location.is_empty())
            .ok_or_else(|| DocError::InvalidFib("CLX location is empty".to_string()))?;
        let clx = checked_slice(
            &table_stream,
            clx_location.offset,
            clx_location.length,
            "CLX",
        )?;
        let piece_table = PieceTable::parse(clx, fib.stories.piece_table_end()?, limits)?;
        piece_table.validate_word_bounds(meaningful_word_bytes)?;
        let stories = decode_stories(&fib, &piece_table, &word_document)?;

        Ok(Self {
            fib,
            piece_table,
            stories,
            word_document,
            table_stream,
            data_stream,
            table_stream_name,
        })
    }

    /// Parsed FIB and all retained structural locations.
    #[must_use]
    pub const fn fib(&self) -> &Fib {
        &self.fib
    }

    /// Validated CLX piece table.
    #[must_use]
    pub const fn piece_table(&self) -> &PieceTable {
        &self.piece_table
    }

    /// Stories in normative global CP order.
    #[must_use]
    pub fn stories(&self) -> &[Story] {
        &self.stories
    }

    /// Find one story by kind.
    #[must_use]
    pub fn story(&self, kind: StoryKind) -> Option<&Story> {
        self.stories.iter().find(|story| story.kind == kind)
    }

    /// Decode any global source CP range.
    ///
    /// # Errors
    ///
    /// Returns a typed [`DocError`] when the range is invalid, uncovered, or
    /// references malformed Word stream bytes.
    pub fn decode_range(&self, cp_start: u32, cp_end: u32) -> Result<DecodedText> {
        self.piece_table
            .decode_range(&self.word_document, cp_start, cp_end)
    }

    /// Parses direct paragraph and character formatting runs from BTE/FKP data.
    ///
    /// The result remains source-backed in physical FC ranges. Mapping those
    /// ranges to final styled CP runs is performed by the higher formatting
    /// layer together with styles and piece PRMs.
    ///
    /// # Errors
    ///
    /// Returns a typed [`DocError`] when a BTE/FKP range is malformed, outside
    /// its stream, or exceeds a configured formatting budget.
    pub fn formatting_index(&self, limits: DocLimits) -> Result<FormattingIndex> {
        FormattingIndex::parse(
            &self.fib,
            self.word_document_stream(),
            &self.table_stream,
            limits,
        )
    }

    /// Parses direct formatting and converts its physical FC ranges to CP ranges.
    ///
    /// # Errors
    ///
    /// Returns a typed [`DocError`] for malformed BTE/FKP data or FC boundaries
    /// that cannot be mapped exactly through the document piece table.
    pub fn logical_formatting(&self, limits: DocLimits) -> Result<LogicalFormattingIndex> {
        self.formatting_index(limits)?.to_logical(&self.piece_table)
    }

    /// Parses, maps, and applies direct FKP and piece-level properties.
    ///
    /// Style inheritance is intentionally deferred until the STSH layer; the
    /// returned toggles retain `same as style` and `opposite of style` states.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed formatting data, invalid known
    /// property operands, or unresolved out-of-range CLX property references.
    pub fn semantic_formatting(&self, limits: DocLimits) -> Result<SemanticFormattingIndex> {
        self.logical_formatting(limits)?
            .resolve_properties(&self.piece_table)
    }

    /// Parses the document stylesheet and validates all inheritance links.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed STSH/STD/UPX records, invalid style
    /// references or cycles, and configured resource limits.
    pub fn styles(&self, limits: DocLimits) -> Result<StyleSheet> {
        StyleSheet::parse(&self.fib, &self.table_stream, limits)
    }

    /// Parses source font names and substitution metadata.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed SttbfFfn/FFN records or limits.
    pub fn fonts(&self, limits: DocLimits) -> Result<FontTable> {
        FontTable::parse(&self.fib, &self.table_stream, limits)
    }

    /// Produces style-aware runs with direct formatting retained for auditing.
    ///
    /// # Errors
    ///
    /// Returns a typed error from formatting, stylesheet parsing, inheritance,
    /// or style-kind validation.
    pub fn styled_formatting(&self, limits: DocLimits) -> Result<StyledFormattingIndex> {
        self.semantic_formatting(limits)?
            .apply_styles(&self.styles(limits)?)
    }

    /// Parses main-document section ranges, raw SEPX properties, and geometry.
    ///
    /// # Errors
    ///
    /// Returns a typed [`DocError`] for malformed PLCFSED/SED/SEPX structures,
    /// invalid section properties, out-of-bounds offsets, or resource limits.
    pub fn sections(&self, limits: DocLimits) -> Result<SectionTable> {
        SectionTable::parse(
            &self.fib,
            self.word_document_stream(),
            &self.table_stream,
            limits,
        )
    }

    /// Parses source-backed footnote and endnote references and body ranges.
    ///
    /// # Errors
    ///
    /// Returns a typed error when either PLC is malformed, its counts disagree,
    /// a body range is invalid, or the configured note budget is exceeded.
    pub fn notes(&self, limits: DocLimits) -> Result<NoteCollection> {
        NoteCollection::parse(self, limits)
    }

    /// Parses source-backed comment references, metadata, and body ranges.
    ///
    /// # Errors
    ///
    /// Returns a typed error when comment PLC framing, markers, metadata,
    /// ranges, or configured budgets are invalid.
    pub fn comments(&self, limits: DocLimits) -> Result<CommentCollection> {
        CommentCollection::parse(self, limits)
    }

    /// Parses Word list definitions and per-instance formatting overrides.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed `PlfLst`/`PlfLfo` data, invalid
    /// level references, or configured list-count limits.
    pub fn lists(&self, limits: DocLimits) -> Result<ListCollection> {
        ListCollection::parse(self, limits)
    }

    /// Parses nested field character PLCs for every Word story.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed PLC framing, invalid field nesting,
    /// mismatched source control characters, or a field-character budget breach.
    pub fn fields(&self, limits: DocLimits) -> Result<FieldCollection> {
        FieldCollection::parse(self, limits)
    }

    /// Meaningful `WordDocument` bytes, including formatting pages.
    #[must_use]
    pub fn word_document_stream(&self) -> &[u8] {
        let meaningful =
            usize::try_from(self.fib.word_document_length).unwrap_or(self.word_document.len());
        &self.word_document[..meaningful.min(self.word_document.len())]
    }

    /// Selected `0Table` or `1Table` stream.
    #[must_use]
    pub fn table_stream(&self) -> &[u8] {
        &self.table_stream
    }

    /// Optional Data stream used by pictures and embedded objects.
    #[must_use]
    pub fn data_stream(&self) -> Option<&[u8]> {
        self.data_stream.as_deref()
    }

    /// Name of the table stream selected by the FIB.
    #[must_use]
    pub const fn table_stream_name(&self) -> &'static str {
        self.table_stream_name
    }
}

fn read_required_stream<R: std::io::Read + std::io::Seek>(
    cfb: &mut CfbReader<R>,
    name: &'static str,
    limits: DocLimits,
) -> Result<Vec<u8>> {
    if !cfb.has_stream(name) {
        return Err(DocError::MissingStream(name));
    }
    let stream = cfb
        .open_stream(name)
        .map_err(|error| DocError::CompoundFile(error.to_string()))?;
    enforce_stream_limit(name, stream.len(), limits)?;
    Ok(stream)
}

fn enforce_stream_limit(name: &'static str, length: usize, limits: DocLimits) -> Result<()> {
    if length > limits.max_stream_bytes {
        Err(DocError::StreamTooLarge {
            stream: name,
            actual: length,
            limit: limits.max_stream_bytes,
        })
    } else {
        Ok(())
    }
}

fn decode_stories(fib: &Fib, piece_table: &PieceTable, word_document: &[u8]) -> Result<Vec<Story>> {
    let definitions = [
        (StoryKind::Main, fib.stories.main),
        (StoryKind::Footnotes, fib.stories.footnotes),
        (StoryKind::Headers, fib.stories.headers),
        (StoryKind::Comments, fib.stories.comments),
        (StoryKind::Endnotes, fib.stories.endnotes),
        (StoryKind::Textboxes, fib.stories.textboxes),
        (StoryKind::HeaderTextboxes, fib.stories.header_textboxes),
    ];
    let mut cp_start = 0_u32;
    let mut stories = Vec::with_capacity(definitions.len());
    for (kind, length) in definitions {
        let cp_end = cp_start
            .checked_add(length)
            .ok_or_else(|| DocError::InvalidFib("story CP range overflow".to_string()))?;
        stories.push(Story {
            kind,
            cp_start,
            cp_end,
            content: piece_table.decode_range(word_document, cp_start, cp_end)?,
        });
        cp_start = cp_end;
    }
    Ok(stories)
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPRESSED_FLAG: u32 = 0x4000_0000;

    #[test]
    fn assembles_mixed_unicode_stories_from_source_cps() {
        let (word, table) = synthetic_streams();
        let document =
            WordBinaryDocument::from_streams(word, table, None, "1Table", DocLimits::default())
                .unwrap();
        assert_eq!(document.fib().effective_version, 0x00C1);
        assert_eq!(
            document.story(StoryKind::Main).unwrap().content.text,
            "Прив"
        );
        assert_eq!(
            document.story(StoryKind::Footnotes).unwrap().content.text,
            "A“"
        );
        assert_eq!(document.piece_table().cp_end(), 7);
        assert_eq!(document.decode_range(2, 5).unwrap().text, "ивA");
    }

    #[test]
    fn rejects_wrong_table_stream_and_oob_cb_mac() {
        let (word, table) = synthetic_streams();
        assert!(matches!(
            WordBinaryDocument::from_streams(
                word.clone(),
                table.clone(),
                None,
                "0Table",
                DocLimits::default()
            ),
            Err(DocError::InvalidFib(_))
        ));

        let mut truncated = word;
        truncated.truncate(1102);
        assert!(matches!(
            WordBinaryDocument::from_streams(
                truncated,
                table,
                None,
                "1Table",
                DocLimits::default()
            ),
            Err(DocError::OutOfBounds { .. })
        ));
    }

    #[test]
    fn enforces_input_limit_before_compound_parsing() {
        let limits = DocLimits {
            max_input_bytes: 3,
            ..DocLimits::default()
        };
        assert!(matches!(
            WordBinaryDocument::from_bytes_with_limits(&[0; 4], limits),
            Err(DocError::InputTooLarge {
                actual: 4,
                limit: 3
            })
        ));
    }

    fn synthetic_streams() -> (Vec<u8>, Vec<u8>) {
        let clx = make_clx(
            &[0, 4, 6, 7],
            &[
                (1024, 0),
                (COMPRESSED_FLAG | (1100 * 2), 0),
                (COMPRESSED_FLAG | (1102 * 2), 0),
            ],
        );
        let mut table = vec![0_u8; 128];
        table.extend_from_slice(&clx);

        let mut word = make_fib(1103, 4, 2, 128, u32::try_from(clx.len()).unwrap());
        word.resize(1103, 0);
        word[1024..1032].copy_from_slice(&[0x1F, 0x04, 0x40, 0x04, 0x38, 0x04, 0x32, 0x04]);
        word[1100..1102].copy_from_slice(&[b'A', 0x93]);
        word[1102] = b'\r';
        (word, table)
    }

    fn make_fib(cb_mac: u32, main: u32, footnotes: u32, fc_clx: u32, lcb_clx: u32) -> Vec<u8> {
        let mut data = vec![0_u8; 32];
        data[0..2].copy_from_slice(&0xA5EC_u16.to_le_bytes());
        data[2..4].copy_from_slice(&0x00C1_u16.to_le_bytes());
        data[6..8].copy_from_slice(&0x0409_u16.to_le_bytes());
        data[10..12].copy_from_slice(&(1_u16 << 9).to_le_bytes());
        data.extend_from_slice(&14_u16.to_le_bytes());
        data.extend_from_slice(&[0_u8; 28]);
        data.extend_from_slice(&22_u16.to_le_bytes());
        let mut lw = [0_u32; 22];
        lw[0] = cb_mac;
        lw[3] = main;
        lw[4] = footnotes;
        for value in lw {
            data.extend_from_slice(&value.to_le_bytes());
        }
        data.extend_from_slice(&34_u16.to_le_bytes());
        for index in 0..34 {
            let (fc, lcb) = if index == 33 {
                (fc_clx, lcb_clx)
            } else {
                (0, 0)
            };
            data.extend_from_slice(&fc.to_le_bytes());
            data.extend_from_slice(&lcb.to_le_bytes());
        }
        data.extend_from_slice(&0_u16.to_le_bytes());
        data
    }

    fn make_clx(cps: &[u32], pieces: &[(u32, u16)]) -> Vec<u8> {
        let mut plc = Vec::new();
        for cp in cps {
            plc.extend_from_slice(&cp.to_le_bytes());
        }
        for (fc, prm) in pieces {
            plc.extend_from_slice(&0_u16.to_le_bytes());
            plc.extend_from_slice(&fc.to_le_bytes());
            plc.extend_from_slice(&prm.to_le_bytes());
        }
        let mut clx = vec![0x02];
        clx.extend_from_slice(&u32::try_from(plc.len()).unwrap().to_le_bytes());
        clx.extend_from_slice(&plc);
        clx
    }
}
