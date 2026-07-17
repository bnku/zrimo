//! CLX/Pcdt/PlcPcd parsing and CP-preserving text retrieval.

use crate::{DocError, DocLimits, PropertyGroup, Result, Sprm, binary::ByteCursor, decode_grpprl};

const COMPRESSED_FLAG: u32 = 0x4000_0000;
const RESERVED_FLAG: u32 = 0x8000_0000;
const FC_MASK: u32 = 0x3FFF_FFFF;

/// Physical encoding of a Word text piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceEncoding {
    /// One byte per CP using the `FcCompressed` 8-bit mapping.
    Compressed,
    /// UTF-16LE, two bytes per CP.
    Utf16,
}

/// One source-backed range from `PlcPcd`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPiece {
    /// First character position in the logical document stream.
    pub cp_start: u32,
    /// Exclusive logical character end.
    pub cp_end: u32,
    /// Byte offset in the `WordDocument` stream.
    pub file_offset: u32,
    /// Physical text encoding.
    pub encoding: PieceEncoding,
    /// Property modifier attached to the PCD.
    pub prm: u16,
}

impl TextPiece {
    /// Number of logical CPs in the piece.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.cp_end - self.cp_start
    }

    /// Whether the piece is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.cp_start == self.cp_end
    }

    pub(crate) const fn bytes_per_cp(self) -> u32 {
        match self.encoding {
            PieceEncoding::Compressed => 1,
            PieceEncoding::Utf16 => 2,
        }
    }

    pub(crate) fn byte_length(self) -> Result<u32> {
        match self.encoding {
            PieceEncoding::Compressed => Ok(self.len()),
            PieceEncoding::Utf16 => self.len().checked_mul(2).ok_or_else(|| {
                DocError::InvalidPieceTable("UTF-16 piece byte length overflow".to_string())
            }),
        }
    }

    pub(crate) fn file_end(self) -> Result<u32> {
        self.file_offset
            .checked_add(self.byte_length()?)
            .ok_or_else(|| {
                DocError::InvalidPieceTable("text piece file range overflow".to_string())
            })
    }
}

/// Decoded text with its original CP range and UTF-16 units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedText {
    /// Source CP start.
    pub cp_start: u32,
    /// Source CP end.
    pub cp_end: u32,
    /// Source-aligned UTF-16 code units (one entry per CP).
    pub utf16: Vec<u16>,
    /// Displayable Unicode string; unpaired surrogates become U+FFFD.
    pub text: String,
}

/// Validated Word text piece table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceTable {
    pieces: Vec<TextPiece>,
    property_groups: Vec<Vec<u8>>,
    cp_end: u32,
}

/// Properties referenced by a PCD's compact `Prm` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiecePropertyModifier {
    /// The special zero-valued `Prm0`, which has no effect.
    None,
    /// A compact `Prm0`; unknown `isprm` values remain explicitly unresolved.
    Simple {
        /// Seven-bit compact property identifier.
        isprm: u8,
        /// One-byte property operand.
        value: u8,
        /// Exact expanded SPRM when this implementation knows the mapping.
        sprm: Option<Sprm>,
    },
    /// A `Prm1` resolved through the zero-based CLX `RgPrc` array.
    Complex {
        /// Zero-based `RgPrc` index.
        index: u16,
        /// Exactly framed properties from the referenced `Prc`.
        sprms: Vec<Sprm>,
    },
}

impl PiecePropertyModifier {
    /// Returns properties from this modifier that affect one property family.
    #[must_use]
    pub fn sprms_for(&self, group: PropertyGroup) -> Vec<Sprm> {
        match self {
            Self::Simple {
                sprm: Some(sprm), ..
            } if sprm.group == group => vec![sprm.clone()],
            Self::Complex { sprms, .. } => sprms
                .iter()
                .filter(|sprm| sprm.group == group)
                .cloned()
                .collect(),
            Self::None | Self::Simple { .. } => Vec::new(),
        }
    }
}

impl PieceTable {
    /// Parse a CLX and validate its terminal CP against the FIB.
    ///
    /// # Errors
    ///
    /// Returns a typed [`DocError`] when the CLX is truncated, malformed,
    /// inconsistent with the FIB, or exceeds the configured limits.
    pub fn parse(clx: &[u8], expected_cp_end: u32, limits: DocLimits) -> Result<Self> {
        let mut cursor = ByteCursor::new(clx, "CLX");
        let mut property_groups = Vec::new();
        let pcdt = loop {
            let marker = *cursor
                .take(1)?
                .first()
                .ok_or_else(|| DocError::InvalidPieceTable("CLX has no Pcdt".to_string()))?;
            match marker {
                0x01 => {
                    let length = usize::from(cursor.read_u16()?);
                    if property_groups.len() >= limits.max_piece_property_groups {
                        return Err(DocError::ResourceLimit {
                            resource: "piece-property-group",
                            actual: u64::try_from(property_groups.len() + 1).unwrap_or(u64::MAX),
                            limit: u64::try_from(limits.max_piece_property_groups)
                                .unwrap_or(u64::MAX),
                        });
                    }
                    property_groups.push(cursor.take(length)?.to_vec());
                }
                0x02 => {
                    let length = usize::try_from(cursor.read_u32()?).map_err(|_| {
                        DocError::InvalidPieceTable("Pcdt length does not fit usize".to_string())
                    })?;
                    let bytes = cursor.take(length)?;
                    if cursor.position() != clx.len() {
                        return Err(DocError::InvalidPieceTable(format!(
                            "{} trailing bytes follow Pcdt",
                            clx.len() - cursor.position()
                        )));
                    }
                    break bytes;
                }
                other => {
                    return Err(DocError::InvalidPieceTable(format!(
                        "unexpected CLX marker 0x{other:02X} at offset {}",
                        cursor.position() - 1
                    )));
                }
            }
        };

        let pieces = parse_plc_pcd(pcdt, limits)?;
        let cp_end = pieces.last().map_or(0, |piece| piece.cp_end);
        if cp_end != expected_cp_end {
            return Err(DocError::InvalidPieceTable(format!(
                "terminal CP is {cp_end}; FIB requires {expected_cp_end}"
            )));
        }
        Ok(Self {
            pieces,
            property_groups,
            cp_end,
        })
    }

    /// Ordered text pieces.
    #[must_use]
    pub fn pieces(&self) -> &[TextPiece] {
        &self.pieces
    }

    /// Exclusive terminal CP of the piece table.
    #[must_use]
    pub const fn cp_end(&self) -> u32 {
        self.cp_end
    }

    /// Reusable raw `grpprl` records retained from the CLX `RgPrc` array.
    #[must_use]
    pub fn property_groups(&self) -> &[Vec<u8>] {
        &self.property_groups
    }

    /// Resolves a raw PCD `Prm0` or `Prm1` without discarding unknown values.
    ///
    /// # Errors
    ///
    /// Returns [`DocError::InvalidPieceTable`] for an out-of-range `Prm1`
    /// index, or [`DocError::InvalidFormatting`] when its selected `grpprl`
    /// cannot be framed exactly.
    pub fn resolve_prm(&self, raw: u16) -> Result<PiecePropertyModifier> {
        if raw & 1 == 0 {
            let isprm = u8::try_from((raw >> 1) & 0x7F)
                .map_err(|_| DocError::InvalidPieceTable("Prm0 isprm conversion failed".into()))?;
            let value = u8::try_from(raw >> 8).map_err(|_| {
                DocError::InvalidPieceTable("Prm0 operand conversion failed".into())
            })?;
            if isprm == 0 && value == 0 {
                return Ok(PiecePropertyModifier::None);
            }
            let sprm = simple_prm_opcode(isprm)
                .map(|opcode| {
                    let [low, high] = opcode.to_le_bytes();
                    decode_grpprl(&[low, high, value])
                })
                .transpose()?
                .and_then(|mut decoded| decoded.pop());
            return Ok(PiecePropertyModifier::Simple { isprm, value, sprm });
        }

        let index = raw >> 1;
        let raw_group = self
            .property_groups
            .get(usize::from(index))
            .ok_or_else(|| {
                DocError::InvalidPieceTable(format!(
                    "Prm1 index {index} exceeds CLX RgPrc count {}",
                    self.property_groups.len()
                ))
            })?;
        Ok(PiecePropertyModifier::Complex {
            index,
            sprms: decode_grpprl(raw_group).map_err(|error| {
                DocError::InvalidFormatting(format!(
                    "CLX RgPrc[{index}] cannot be framed: {error}; bytes={raw_group:02X?}"
                ))
            })?,
        })
    }

    /// Validate all physical piece ranges against meaningful `WordDocument` bytes.
    ///
    /// # Errors
    ///
    /// Returns [`DocError::OutOfBounds`] if a piece references bytes outside the
    /// meaningful `WordDocument` range.
    pub fn validate_word_bounds(&self, meaningful_word_bytes: usize) -> Result<()> {
        for piece in &self.pieces {
            let start = usize::try_from(piece.file_offset).map_err(|_| DocError::OutOfBounds {
                structure: "text piece",
                offset: usize::MAX,
                end: usize::MAX,
                available: meaningful_word_bytes,
            })?;
            let byte_length =
                usize::try_from(piece.byte_length()?).map_err(|_| DocError::OutOfBounds {
                    structure: "text piece",
                    offset: start,
                    end: usize::MAX,
                    available: meaningful_word_bytes,
                })?;
            let end = start
                .checked_add(byte_length)
                .ok_or(DocError::OutOfBounds {
                    structure: "text piece",
                    offset: start,
                    end: usize::MAX,
                    available: meaningful_word_bytes,
                })?;
            if end > meaningful_word_bytes {
                return Err(DocError::OutOfBounds {
                    structure: "text piece",
                    offset: start,
                    end,
                    available: meaningful_word_bytes,
                });
            }
        }
        Ok(())
    }

    /// Decode an arbitrary source CP range without losing UTF-16 alignment.
    ///
    /// # Errors
    ///
    /// Returns a typed [`DocError`] when the requested range is invalid,
    /// uncovered, or references malformed stream bytes.
    pub fn decode_range(
        &self,
        word_document: &[u8],
        cp_start: u32,
        cp_end: u32,
    ) -> Result<DecodedText> {
        if cp_start > cp_end || cp_end > self.cp_end {
            return Err(DocError::InvalidPieceTable(format!(
                "requested CP range [{cp_start}, {cp_end}) exceeds [0, {})",
                self.cp_end
            )));
        }
        let capacity = usize::try_from(cp_end - cp_start).map_err(|_| DocError::ResourceLimit {
            resource: "decoded character",
            actual: u64::from(cp_end - cp_start),
            limit: usize::MAX as u64,
        })?;
        let mut utf16 = Vec::with_capacity(capacity);
        for piece in self
            .pieces
            .iter()
            .filter(|piece| piece.cp_start < cp_end && piece.cp_end > cp_start)
        {
            let overlap_start = piece.cp_start.max(cp_start);
            let overlap_end = piece.cp_end.min(cp_end);
            decode_piece(
                word_document,
                *piece,
                overlap_start,
                overlap_end,
                &mut utf16,
            )?;
        }
        if utf16.len() != capacity {
            return Err(DocError::InvalidPieceTable(format!(
                "decoded {} UTF-16 units for a {capacity}-CP range",
                utf16.len()
            )));
        }
        let text = String::from_utf16_lossy(&utf16);
        Ok(DecodedText {
            cp_start,
            cp_end,
            utf16,
            text,
        })
    }
}

fn simple_prm_opcode(isprm: u8) -> Option<u16> {
    Some(match isprm {
        0x05 => 0x2403, // sprmPJc (physical Word 97 form)
        0x07 => 0x2405, // sprmPFKeep
        0x08 => 0x2406, // sprmPFKeepFollow
        0x09 => 0x2407, // sprmPFPageBreakBefore
        0x0C => 0x260A, // sprmPIlvl
        0x0D => 0x2470, // sprmPFMirrorIndents
        0x18 => 0x2416, // sprmPFInTable
        0x19 => 0x2417, // sprmPFTtp
        0x4D => 0x2A0C, // sprmCHighlight
        0x4E => 0x0858, // sprmCFEmboss
        0x53 => 0x2A33, // sprmCPlain
        0x55 => 0x0835, // sprmCFBold
        0x56 => 0x0836, // sprmCFItalic
        0x57 => 0x0837, // sprmCFStrike
        0x58 => 0x0838, // sprmCFOutline
        0x59 => 0x0839, // sprmCFShadow
        0x5A => 0x083A, // sprmCFSmallCaps
        0x5B => 0x083B, // sprmCFCaps
        0x5C => 0x083C, // sprmCFVanish
        0x5E => 0x2A3E, // sprmCKul
        0x62 => 0x2A42, // sprmCIco
        0x68 => 0x2A48, // sprmCIss
        0x73 => 0x2A53, // sprmCFDStrike
        0x74 => 0x0854, // sprmCFImprint
        0x75 => 0x0855, // sprmCFSpec
        0x76 => 0x0856, // sprmCFObj
        0x78 => 0x2640, // sprmPOutLvl
        _ => return None,
    })
}

fn parse_plc_pcd(data: &[u8], limits: DocLimits) -> Result<Vec<TextPiece>> {
    if data.len() < 4 || !(data.len() - 4).is_multiple_of(12) {
        return Err(DocError::InvalidPieceTable(format!(
            "PlcPcd length {} is not 4 + 12*n",
            data.len()
        )));
    }
    let piece_count = (data.len() - 4) / 12;
    if piece_count > limits.max_pieces {
        return Err(DocError::ResourceLimit {
            resource: "text-piece",
            actual: u64::try_from(piece_count).unwrap_or(u64::MAX),
            limit: u64::try_from(limits.max_pieces).unwrap_or(u64::MAX),
        });
    }
    let cp_bytes = (piece_count + 1)
        .checked_mul(4)
        .ok_or_else(|| DocError::InvalidPieceTable("CP array overflow".to_string()))?;
    let mut cps = Vec::with_capacity(piece_count + 1);
    let mut cp_cursor = ByteCursor::new(&data[..cp_bytes], "PlcPcd CP array");
    for _ in 0..=piece_count {
        let cp = cp_cursor.read_i32()?;
        cps.push(u32::try_from(cp).map_err(|_| {
            DocError::InvalidPieceTable(format!("negative character position {cp}"))
        })?);
    }
    if cps.first().copied().unwrap_or_default() != 0 {
        return Err(DocError::InvalidPieceTable(
            "first character position is not zero".to_string(),
        ));
    }
    if cps.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DocError::InvalidPieceTable(
            "character positions are not strictly increasing".to_string(),
        ));
    }

    let mut pcd_cursor = ByteCursor::new(&data[cp_bytes..], "PlcPcd PCD array");
    let mut pieces = Vec::with_capacity(piece_count);
    for index in 0..piece_count {
        pcd_cursor.skip(2)?;
        let raw_fc = pcd_cursor.read_u32()?;
        let prm = pcd_cursor.read_u16()?;
        if raw_fc & RESERVED_FLAG != 0 {
            return Err(DocError::InvalidPieceTable(format!(
                "PCD {index} sets reserved FcCompressed bit"
            )));
        }
        let compressed = raw_fc & COMPRESSED_FLAG != 0;
        let encoded_fc = raw_fc & FC_MASK;
        if compressed && !encoded_fc.is_multiple_of(2) {
            return Err(DocError::InvalidPieceTable(format!(
                "PCD {index} has odd compressed fc {encoded_fc}"
            )));
        }
        pieces.push(TextPiece {
            cp_start: cps[index],
            cp_end: cps[index + 1],
            file_offset: if compressed {
                encoded_fc / 2
            } else {
                encoded_fc
            },
            encoding: if compressed {
                PieceEncoding::Compressed
            } else {
                PieceEncoding::Utf16
            },
            prm,
        });
    }
    Ok(pieces)
}

fn decode_piece(
    word_document: &[u8],
    piece: TextPiece,
    cp_start: u32,
    cp_end: u32,
    output: &mut Vec<u16>,
) -> Result<()> {
    let relative_start = cp_start - piece.cp_start;
    let cp_count = cp_end - cp_start;
    let bytes_per_cp = match piece.encoding {
        PieceEncoding::Compressed => 1_u32,
        PieceEncoding::Utf16 => 2_u32,
    };
    let byte_start = piece
        .file_offset
        .checked_add(relative_start.checked_mul(bytes_per_cp).ok_or_else(|| {
            DocError::InvalidPieceTable("piece range byte offset overflow".to_string())
        })?)
        .ok_or_else(|| {
            DocError::InvalidPieceTable("piece range file offset overflow".to_string())
        })?;
    let byte_length = cp_count.checked_mul(bytes_per_cp).ok_or_else(|| {
        DocError::InvalidPieceTable("piece range byte length overflow".to_string())
    })?;
    let start = usize::try_from(byte_start).map_err(|_| DocError::OutOfBounds {
        structure: "text piece",
        offset: usize::MAX,
        end: usize::MAX,
        available: word_document.len(),
    })?;
    let length = usize::try_from(byte_length).map_err(|_| DocError::OutOfBounds {
        structure: "text piece",
        offset: start,
        end: usize::MAX,
        available: word_document.len(),
    })?;
    let end = start.checked_add(length).ok_or(DocError::OutOfBounds {
        structure: "text piece",
        offset: start,
        end: usize::MAX,
        available: word_document.len(),
    })?;
    let bytes = word_document.get(start..end).ok_or(DocError::OutOfBounds {
        structure: "text piece",
        offset: start,
        end,
        available: word_document.len(),
    })?;
    match piece.encoding {
        PieceEncoding::Compressed => {
            output.extend(bytes.iter().copied().map(compressed_byte_to_utf16));
        }
        PieceEncoding::Utf16 => {
            output.extend(
                bytes
                    .chunks_exact(2)
                    .map(|pair| u16::from_le_bytes([pair[0], pair[1]])),
            );
        }
    }
    Ok(())
}

fn compressed_byte_to_utf16(byte: u8) -> u16 {
    match byte {
        0x82 => 0x201A,
        0x83 => 0x0192,
        0x84 => 0x201E,
        0x85 => 0x2026,
        0x86 => 0x2020,
        0x87 => 0x2021,
        0x88 => 0x02C6,
        0x89 => 0x2030,
        0x8A => 0x0160,
        0x8B => 0x2039,
        0x8C => 0x0152,
        0x91 => 0x2018,
        0x92 => 0x2019,
        0x93 => 0x201C,
        0x94 => 0x201D,
        0x95 => 0x2022,
        0x96 => 0x2013,
        0x97 => 0x2014,
        0x98 => 0x02DC,
        0x99 => 0x2122,
        0x9A => 0x0161,
        0x9B => 0x203A,
        0x9C => 0x0153,
        0x9F => 0x0178,
        _ => u16::from(byte),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clx(cps: &[u32], pieces: &[(u32, u16)]) -> Vec<u8> {
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

    #[test]
    fn parses_mixed_pieces_and_decodes_cp_ranges() {
        let table = PieceTable::parse(
            &clx(&[0, 3, 6], &[(COMPRESSED_FLAG | 0x28, 7), (100, 9)]),
            6,
            DocLimits::default(),
        )
        .unwrap();
        assert_eq!(table.pieces()[0].file_offset, 20);
        assert_eq!(table.pieces()[0].encoding, PieceEncoding::Compressed);
        assert_eq!(table.pieces()[1].encoding, PieceEncoding::Utf16);

        let mut word = vec![0_u8; 106];
        word[20..23].copy_from_slice(&[b'A', 0x93, b'B']);
        word[100..106].copy_from_slice(&[0x1F, 0x04, 0x40, 0x04, 0x38, 0x04]);
        table.validate_word_bounds(word.len()).unwrap();
        let text = table.decode_range(&word, 1, 5).unwrap();
        assert_eq!(text.cp_start, 1);
        assert_eq!(text.utf16, [0x201C, u16::from(b'B'), 0x041F, 0x0440]);
        assert_eq!(text.text, "“BПр");
    }

    #[test]
    fn retains_property_groups_and_requires_exact_pcdt_length() {
        let mut prefixed = vec![0x01, 3, 0, 9, 8, 7];
        prefixed.extend_from_slice(&clx(&[0, 1], &[(COMPRESSED_FLAG, 0)]));
        let table = PieceTable::parse(&prefixed, 1, DocLimits::default()).unwrap();
        assert_eq!(table.property_groups(), &[vec![9, 8, 7]]);
        prefixed.push(0);
        assert!(matches!(
            PieceTable::parse(&prefixed, 1, DocLimits::default()),
            Err(DocError::InvalidPieceTable(_))
        ));
    }

    #[test]
    fn resolves_simple_and_complex_piece_property_modifiers() {
        let mut data = vec![0x01, 3, 0, 0x36, 0x08, 1]; // Prc: italic on
        data.extend_from_slice(&clx(
            &[0, 1, 2],
            &[
                (COMPRESSED_FLAG, 0x01AA), // Prm0: isprm 0x55, bold on
                (COMPRESSED_FLAG | 2, 1),  // Prm1: RgPrc[0]
            ],
        ));
        let table = PieceTable::parse(&data, 2, DocLimits::default()).unwrap();
        let simple = table.resolve_prm(table.pieces()[0].prm).unwrap();
        assert!(matches!(
            simple,
            PiecePropertyModifier::Simple {
                isprm: 0x55,
                value: 1,
                sprm: Some(Sprm { opcode: 0x0835, .. })
            }
        ));
        let complex = table.resolve_prm(table.pieces()[1].prm).unwrap();
        assert!(matches!(
            complex,
            PiecePropertyModifier::Complex { index: 0, ref sprms }
                if sprms.len() == 1 && sprms[0].opcode == 0x0836
        ));
    }

    #[test]
    fn retains_unknown_simple_prm_and_rejects_bad_complex_index() {
        let table = PieceTable::parse(
            &clx(&[0, 1], &[(COMPRESSED_FLAG, 0x0192)]),
            1,
            DocLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            table.resolve_prm(0x0192).unwrap(),
            PiecePropertyModifier::Simple {
                isprm: 0x49,
                value: 1,
                sprm: None
            }
        ));
        assert!(matches!(
            table.resolve_prm(1),
            Err(DocError::InvalidPieceTable(_))
        ));
    }

    #[test]
    fn rejects_duplicate_cp_reserved_fc_and_wrong_terminal_cp() {
        assert!(matches!(
            PieceTable::parse(
                &clx(&[0, 0], &[(COMPRESSED_FLAG, 0)]),
                0,
                DocLimits::default()
            ),
            Err(DocError::InvalidPieceTable(_))
        ));
        assert!(matches!(
            PieceTable::parse(
                &clx(&[0, 1], &[(RESERVED_FLAG, 0)]),
                1,
                DocLimits::default()
            ),
            Err(DocError::InvalidPieceTable(_))
        ));
        assert!(matches!(
            PieceTable::parse(
                &clx(&[0, 2], &[(COMPRESSED_FLAG, 0)]),
                3,
                DocLimits::default()
            ),
            Err(DocError::InvalidPieceTable(_))
        ));
    }

    #[test]
    fn enforces_piece_budget_and_word_bounds() {
        let limits = DocLimits {
            max_pieces: 1,
            ..DocLimits::default()
        };
        assert!(matches!(
            PieceTable::parse(
                &clx(
                    &[0, 1, 2],
                    &[(COMPRESSED_FLAG, 0), (COMPRESSED_FLAG | 2, 0)]
                ),
                2,
                limits
            ),
            Err(DocError::ResourceLimit {
                resource: "text-piece",
                ..
            })
        ));

        let table = PieceTable::parse(
            &clx(&[0, 4], &[(COMPRESSED_FLAG | 0x14, 0)]),
            4,
            DocLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            table.validate_word_bounds(12),
            Err(DocError::OutOfBounds { .. })
        ));
    }
}
