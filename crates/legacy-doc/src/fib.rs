//! File Information Block parsing for Word 97–2003 binary documents.

use crate::{DocError, DocLimits, Result, binary::ByteCursor};

const WORD_BINARY_IDENT: u16 = 0xA5EC;
const WORD_6_IDENT: u16 = 0xA5DC;

const INDEX_STSHF: usize = 1;
const INDEX_PLCF_FND_REF: usize = 2;
const INDEX_PLCF_FND_TXT: usize = 3;
const INDEX_PLCF_AND_REF: usize = 4;
const INDEX_PLCF_AND_TXT: usize = 5;
const INDEX_PLCF_SED: usize = 6;
const INDEX_PLCF_HDD: usize = 11;
const INDEX_PLCF_BTE_CHPX: usize = 12;
const INDEX_PLCF_BTE_PAPX: usize = 13;
const INDEX_STTBF_FFN: usize = 15;
const INDEX_PLCF_FLD_MOM: usize = 16;
const INDEX_PLCF_FLD_HDR: usize = 17;
const INDEX_PLCF_FLD_FTN: usize = 18;
const INDEX_PLCF_FLD_ATN: usize = 19;
const INDEX_CLX: usize = 33;
const INDEX_GRP_XST_ATN_OWNERS: usize = 36;
const INDEX_STTBF_ATN_BKMK: usize = 37;
const INDEX_PLCF_ATN_BKF: usize = 42;
const INDEX_PLCF_ATN_BKL: usize = 43;
const INDEX_PLF_LST: usize = 73;
const INDEX_PLF_LFO: usize = 74;
const INDEX_PLCF_END_REF: usize = 46;
const INDEX_PLCF_END_TXT: usize = 47;
const INDEX_PLCF_FLD_EDN: usize = 48;
const INDEX_PLCF_TXBX_TXT: usize = 56;
const INDEX_PLCF_FLD_TXBX: usize = 57;
const INDEX_PLCF_HDR_TXBX_TXT: usize = 58;
const INDEX_PLCF_FLD_HDR_TXBX: usize = 59;

/// Offset/length pair stored in `FibRgFcLcb`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FcLcb {
    /// Byte offset in the stream defined by the corresponding FIB field.
    pub offset: u32,
    /// Byte length of the referenced structure.
    pub length: u32,
}

impl FcLcb {
    /// Whether the referenced structure is absent.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.length == 0
    }
}

/// Fixed 32-byte `FibBase` fields relevant to parsing and security.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FibBase {
    /// Version recorded in the fixed header.
    pub version: u16,
    /// Installation language identifier.
    pub language_id: u16,
    /// Whether `1Table` is selected instead of `0Table`.
    pub use_table1: bool,
    /// Whether the document contains picture records.
    pub has_pictures: bool,
    /// Whether the last save was incremental/complex.
    pub is_complex: bool,
}

/// Declared CP lengths of the Word subdocuments (stories).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StoryLengths {
    /// Main document story.
    pub main: u32,
    /// Footnote story.
    pub footnotes: u32,
    /// Header/footer story.
    pub headers: u32,
    /// Comment story.
    pub comments: u32,
    /// Endnote story.
    pub endnotes: u32,
    /// Main-document textbox story.
    pub textboxes: u32,
    /// Header textbox story.
    pub header_textboxes: u32,
}

impl StoryLengths {
    /// Sum of declared story lengths, excluding the optional terminal CLX CP.
    ///
    /// # Errors
    ///
    /// Returns [`DocError::InvalidFib`] if the sum overflows `u32`.
    pub fn total(self) -> Result<u32> {
        [
            self.main,
            self.footnotes,
            self.headers,
            self.comments,
            self.endnotes,
            self.textboxes,
            self.header_textboxes,
        ]
        .into_iter()
        .try_fold(0_u32, |sum, value| {
            sum.checked_add(value).ok_or_else(|| {
                DocError::InvalidFib("story character counts overflow u32".to_string())
            })
        })
    }

    /// CP expected as the last entry in `PlcPcd`.
    ///
    /// # Errors
    ///
    /// Returns [`DocError::InvalidFib`] if the terminal CP overflows `u32`.
    pub fn piece_table_end(self) -> Result<u32> {
        let total = self.total()?;
        if self.has_auxiliary_story() {
            total.checked_add(1).ok_or_else(|| {
                DocError::InvalidFib("piece-table terminal CP overflows u32".to_string())
            })
        } else {
            Ok(total)
        }
    }

    const fn has_auxiliary_story(self) -> bool {
        self.footnotes != 0
            || self.headers != 0
            || self.comments != 0
            || self.endnotes != 0
            || self.textboxes != 0
            || self.header_textboxes != 0
    }
}

/// Variable `FibRgFcLcb` locations retained for later structural parsers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FibLocations {
    pairs: Vec<FcLcb>,
}

impl FibLocations {
    /// Return a raw pair by its normative `FibRgFcLcb97` index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<FcLcb> {
        self.pairs.get(index).copied()
    }

    /// Number of location pairs present in this FIB revision.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Whether no pairs were present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Style sheet (`STSH`).
    #[must_use]
    pub fn stylesheet(&self) -> Option<FcLcb> {
        self.get(INDEX_STSHF)
    }

    /// Section descriptor PLC (`PlcfSed`).
    #[must_use]
    pub fn sections(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_SED)
    }

    /// Footnote reference PLC (`PlcffndRef`).
    #[must_use]
    pub fn footnote_references(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_FND_REF)
    }

    /// Footnote text-boundary PLC (`PlcffndTxt`).
    #[must_use]
    pub fn footnote_text(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_FND_TXT)
    }

    /// Comment reference PLC (`PlcfandRef`).
    #[must_use]
    pub fn comment_references(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_AND_REF)
    }

    /// Comment text-boundary PLC (`PlcfandTxt`).
    #[must_use]
    pub fn comment_text(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_AND_TXT)
    }

    /// Array of comment-author names (`GrpXstAtnOwners`).
    #[must_use]
    pub fn comment_authors(&self) -> Option<FcLcb> {
        self.get(INDEX_GRP_XST_ATN_OWNERS)
    }

    /// Annotation bookmark metadata (`SttbfAtnBkmk`).
    #[must_use]
    pub fn comment_bookmarks(&self) -> Option<FcLcb> {
        self.get(INDEX_STTBF_ATN_BKMK)
    }

    /// Annotation bookmark start PLC (`PlcfAtnBkf`).
    #[must_use]
    pub fn comment_bookmark_starts(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_ATN_BKF)
    }

    /// Annotation bookmark end PLC (`PlcfAtnBkl`).
    #[must_use]
    pub fn comment_bookmark_ends(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_ATN_BKL)
    }

    /// List definitions (`PlfLst`); appended `LVL` records follow this range.
    #[must_use]
    pub fn list_definitions(&self) -> Option<FcLcb> {
        self.get(INDEX_PLF_LST)
    }

    /// List format overrides (`PlfLfo`).
    #[must_use]
    pub fn list_overrides(&self) -> Option<FcLcb> {
        self.get(INDEX_PLF_LFO)
    }

    /// Header/footer boundary PLC (`PlcfHdd`).
    #[must_use]
    pub fn headers(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_HDD)
    }

    /// Character formatting bin-table PLC.
    #[must_use]
    pub fn character_bte(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_BTE_CHPX)
    }

    /// Paragraph formatting bin-table PLC.
    #[must_use]
    pub fn paragraph_bte(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_BTE_PAPX)
    }

    /// Font table (`SttbfFfn`).
    #[must_use]
    pub fn fonts(&self) -> Option<FcLcb> {
        self.get(INDEX_STTBF_FFN)
    }

    /// Main-document field PLC (`PlcfFldMom`).
    #[must_use]
    pub fn main_fields(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_FLD_MOM)
    }

    /// Header/footer field PLC (`PlcfFldHdr`).
    #[must_use]
    pub fn header_fields(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_FLD_HDR)
    }

    /// Footnote field PLC (`PlcfFldFtn`).
    #[must_use]
    pub fn footnote_fields(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_FLD_FTN)
    }

    /// Comment field PLC (`PlcfFldAtn`).
    #[must_use]
    pub fn comment_fields(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_FLD_ATN)
    }

    /// Text piece table (`Clx`).
    #[must_use]
    pub fn clx(&self) -> Option<FcLcb> {
        self.get(INDEX_CLX)
    }

    /// Endnote reference PLC.
    #[must_use]
    pub fn endnote_references(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_END_REF)
    }

    /// Endnote text-boundary PLC.
    #[must_use]
    pub fn endnote_text(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_END_TXT)
    }

    /// Endnote field PLC (`PlcfFldEdn`).
    #[must_use]
    pub fn endnote_fields(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_FLD_EDN)
    }

    /// Main textbox text-boundary PLC.
    #[must_use]
    pub fn textbox_text(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_TXBX_TXT)
    }

    /// Main-textbox field PLC (`PlcfFldTxbx`).
    #[must_use]
    pub fn textbox_fields(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_FLD_TXBX)
    }

    /// Header textbox text-boundary PLC.
    #[must_use]
    pub fn header_textbox_text(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_HDR_TXBX_TXT)
    }

    /// Header-textbox field PLC (`PlcfFldHdrTxbx`).
    #[must_use]
    pub fn header_textbox_fields(&self) -> Option<FcLcb> {
        self.get(INDEX_PLCF_FLD_HDR_TXBX)
    }
}

/// Parsed File Information Block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fib {
    /// Fixed header fields.
    pub base: FibBase,
    /// Effective FIB version after applying `nFibNew`, when present.
    pub effective_version: u16,
    /// Meaningful byte length of the `WordDocument` stream.
    pub word_document_length: u32,
    /// Story character counts.
    pub stories: StoryLengths,
    /// Locations of referenced structures.
    pub locations: FibLocations,
    /// Total bytes consumed by the variable FIB.
    pub byte_length: usize,
}

impl Fib {
    /// Parse the variable-length FIB using the normative count fields.
    ///
    /// # Errors
    ///
    /// Returns a typed [`DocError`] when the FIB is truncated, malformed,
    /// encrypted, unsupported, or exceeds the configured limits.
    pub fn parse(data: &[u8], limits: DocLimits) -> Result<Self> {
        if data.len() < 32 {
            return Err(DocError::InvalidFib(format!(
                "WordDocument stream has {} bytes; FibBase needs 32",
                data.len()
            )));
        }

        let ident = read_u16_at(data, 0)?;
        if ident == WORD_6_IDENT {
            return Err(DocError::UnsupportedVersion(read_u16_at(data, 2)?));
        }
        if ident != WORD_BINARY_IDENT {
            return Err(DocError::InvalidFib(format!(
                "unexpected wIdent 0x{ident:04X}"
            )));
        }

        let version = read_u16_at(data, 2)?;
        let language_id = read_u16_at(data, 6)?;
        let flags = read_u16_at(data, 10)?;
        if flags & (1 << 8) != 0 {
            return Err(DocError::PasswordProtected);
        }

        let mut cursor = ByteCursor::new(&data[32..], "FIB");
        let word_count = usize::from(cursor.read_u16()?);
        cursor.skip(
            word_count
                .checked_mul(2)
                .ok_or_else(|| DocError::InvalidFib("FibRgW byte count overflow".to_string()))?,
        )?;

        let long_word_count = usize::from(cursor.read_u16()?);
        if long_word_count < 11 {
            return Err(DocError::InvalidFib(format!(
                "FibRgLw has {long_word_count} values; at least 11 are required"
            )));
        }
        let lw_bytes =
            cursor.take(long_word_count.checked_mul(4).ok_or_else(|| {
                DocError::InvalidFib("FibRgLw byte count overflow".to_string())
            })?)?;
        let mut lw = ByteCursor::new(lw_bytes, "FibRgLw");
        let word_document_length = lw.read_u32()?;
        lw.skip(8)?;
        let stories = StoryLengths {
            main: read_non_negative_count(&mut lw, "ccpText")?,
            footnotes: read_non_negative_count(&mut lw, "ccpFtn")?,
            headers: read_non_negative_count(&mut lw, "ccpHdd")?,
            ..read_remaining_story_lengths(&mut lw)?
        };
        let total_characters = stories.total()?;
        if total_characters > limits.max_characters {
            return Err(DocError::ResourceLimit {
                resource: "character",
                actual: u64::from(total_characters),
                limit: u64::from(limits.max_characters),
            });
        }

        let pair_count = usize::from(cursor.read_u16()?);
        if pair_count > limits.max_fib_pairs {
            return Err(DocError::ResourceLimit {
                resource: "FIB location-pair",
                actual: u64::try_from(pair_count).unwrap_or(u64::MAX),
                limit: u64::try_from(limits.max_fib_pairs).unwrap_or(u64::MAX),
            });
        }
        let mut pairs = Vec::with_capacity(pair_count);
        for _ in 0..pair_count {
            pairs.push(FcLcb {
                offset: cursor.read_u32()?,
                length: cursor.read_u32()?,
            });
        }
        if pair_count <= INDEX_CLX {
            return Err(DocError::InvalidFib(format!(
                "FibRgFcLcb has {pair_count} pairs; CLX pair 33 is absent"
            )));
        }

        let csw_new = usize::from(cursor.read_u16()?);
        let new_words = cursor.take(csw_new.checked_mul(2).ok_or_else(|| {
            DocError::InvalidFib("FibRgCswNew byte count overflow".to_string())
        })?)?;
        let effective_version = if csw_new == 0 {
            version
        } else {
            read_u16_at(new_words, 0)?
        };
        ensure_supported_version(effective_version)?;

        Ok(Self {
            base: FibBase {
                version,
                language_id,
                use_table1: flags & (1 << 9) != 0,
                has_pictures: flags & (1 << 3) != 0,
                is_complex: flags & (1 << 2) != 0,
            },
            effective_version,
            word_document_length,
            stories,
            locations: FibLocations { pairs },
            byte_length: 32 + cursor.position(),
        })
    }
}

fn read_remaining_story_lengths(cursor: &mut ByteCursor<'_>) -> Result<StoryLengths> {
    cursor.skip(4)?;
    Ok(StoryLengths {
        comments: read_non_negative_count(cursor, "ccpAtn")?,
        endnotes: read_non_negative_count(cursor, "ccpEdn")?,
        textboxes: read_non_negative_count(cursor, "ccpTxbx")?,
        header_textboxes: read_non_negative_count(cursor, "ccpHdrTxbx")?,
        ..StoryLengths::default()
    })
}

fn read_non_negative_count(cursor: &mut ByteCursor<'_>, name: &str) -> Result<u32> {
    let value = cursor.read_i32()?;
    u32::try_from(value).map_err(|_| DocError::InvalidFib(format!("{name} is negative: {value}")))
}

fn ensure_supported_version(version: u16) -> Result<()> {
    if matches!(version, 0x00C1 | 0x00D9 | 0x0101 | 0x010C | 0x0112) {
        Ok(())
    } else {
        Err(DocError::UnsupportedVersion(version))
    }
}

fn read_u16_at(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = data.get(offset..offset + 2).ok_or(DocError::OutOfBounds {
        structure: "FibBase",
        offset,
        end: offset + 2,
        available: data.len(),
    })?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fib_bytes(stories: StoryLengths) -> Vec<u8> {
        let mut data = vec![0_u8; 32];
        data[0..2].copy_from_slice(&WORD_BINARY_IDENT.to_le_bytes());
        data[2..4].copy_from_slice(&0x00C1_u16.to_le_bytes());
        data[6..8].copy_from_slice(&0x0419_u16.to_le_bytes());
        data[10..12].copy_from_slice(&(1_u16 << 9).to_le_bytes());
        data.extend_from_slice(&14_u16.to_le_bytes());
        data.extend_from_slice(&[0_u8; 28]);
        data.extend_from_slice(&22_u16.to_le_bytes());
        let mut lw = [0_u32; 22];
        lw[0] = 4096;
        lw[3] = stories.main;
        lw[4] = stories.footnotes;
        lw[5] = stories.headers;
        lw[7] = stories.comments;
        lw[8] = stories.endnotes;
        lw[9] = stories.textboxes;
        lw[10] = stories.header_textboxes;
        for value in lw {
            data.extend_from_slice(&value.to_le_bytes());
        }
        data.extend_from_slice(&37_u16.to_le_bytes());
        for index in 0..37_u32 {
            data.extend_from_slice(&(index * 10).to_le_bytes());
            data.extend_from_slice(&5_u32.to_le_bytes());
        }
        data.extend_from_slice(&0_u16.to_le_bytes());
        data
    }

    #[test]
    fn parses_variable_fib_and_named_locations() {
        let stories = StoryLengths {
            main: 8,
            footnotes: 3,
            headers: 2,
            ..StoryLengths::default()
        };
        let fib = Fib::parse(&fib_bytes(stories), DocLimits::default()).unwrap();
        assert_eq!(fib.base.language_id, 0x0419);
        assert!(fib.base.use_table1);
        assert_eq!(fib.stories, stories);
        assert_eq!(fib.stories.piece_table_end().unwrap(), 14);
        assert_eq!(fib.locations.clx().unwrap().offset, 330);
        assert_eq!(fib.locations.paragraph_bte().unwrap().offset, 130);
        assert_eq!(fib.locations.comment_authors().unwrap().offset, 360);
    }

    #[test]
    fn follows_count_fields_instead_of_fixed_offsets() {
        let mut bytes = fib_bytes(StoryLengths {
            main: 1,
            ..StoryLengths::default()
        });
        bytes[32..34].copy_from_slice(&15_u16.to_le_bytes());
        bytes.splice(62..62, [0_u8; 2]);
        let fib = Fib::parse(&bytes, DocLimits::default()).unwrap();
        assert_eq!(fib.stories.main, 1);
        assert_eq!(fib.locations.clx().unwrap().offset, 330);
    }

    #[test]
    fn rejects_encryption_negative_counts_and_missing_clx_pair() {
        let mut encrypted = fib_bytes(StoryLengths::default());
        encrypted[10..12].copy_from_slice(&(1_u16 << 8).to_le_bytes());
        assert_eq!(
            Fib::parse(&encrypted, DocLimits::default()),
            Err(DocError::PasswordProtected)
        );

        let mut negative = fib_bytes(StoryLengths::default());
        negative[76..80].copy_from_slice(&(-1_i32).to_le_bytes());
        assert!(matches!(
            Fib::parse(&negative, DocLimits::default()),
            Err(DocError::InvalidFib(_))
        ));

        let mut missing = fib_bytes(StoryLengths::default());
        let pair_count_offset = 32 + 2 + 28 + 2 + 88;
        missing[pair_count_offset..pair_count_offset + 2].copy_from_slice(&10_u16.to_le_bytes());
        missing.truncate(pair_count_offset + 2 + 10 * 8 + 2);
        assert!(matches!(
            Fib::parse(&missing, DocLimits::default()),
            Err(DocError::InvalidFib(_))
        ));
    }

    #[test]
    fn enforces_character_budget() {
        let bytes = fib_bytes(StoryLengths {
            main: 101,
            ..StoryLengths::default()
        });
        let limits = DocLimits {
            max_characters: 100,
            ..DocLimits::default()
        };
        assert!(matches!(
            Fib::parse(&bytes, limits),
            Err(DocError::ResourceLimit {
                resource: "character",
                ..
            })
        ));
    }
}
