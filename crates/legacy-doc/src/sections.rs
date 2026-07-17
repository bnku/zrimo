//! Main-document PLCFSED/SED/SEPX parsing and source-proven section geometry.

use crate::{
    DocError, DocLimits, Fib, PropertyGroup, Result, Sprm, binary::checked_slice, decode_grpprl,
};

const SED_SIZE: usize = 12;

/// Explicit page orientation stored in section properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageOrientation {
    /// Portrait orientation.
    Portrait,
    /// Landscape orientation.
    Landscape,
}

/// Source-proven section layout properties. `None` means not explicitly stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SectionGeometry {
    /// Section break code from `sprmSBkc`.
    pub break_code: Option<u8>,
    /// Whether the first page uses distinct headers and footers.
    pub title_page: Option<bool>,
    /// Explicit page orientation.
    pub orientation: Option<PageOrientation>,
    /// Page width in twips.
    pub page_width_twips: Option<u16>,
    /// Page height in twips.
    pub page_height_twips: Option<u16>,
    /// Left margin in twips.
    pub margin_left_twips: Option<u16>,
    /// Right margin in twips.
    pub margin_right_twips: Option<u16>,
    /// Signed top margin in twips; sign retains fixed/minimum semantics.
    pub margin_top_twips: Option<i16>,
    /// Signed bottom margin in twips; sign retains fixed/minimum semantics.
    pub margin_bottom_twips: Option<i16>,
    /// Gutter in twips.
    pub gutter_twips: Option<u16>,
    /// Header distance from the top edge in twips.
    pub header_distance_twips: Option<u16>,
    /// Footer distance from the bottom edge in twips.
    pub footer_distance_twips: Option<u16>,
    /// Explicit number of columns.
    pub column_count: Option<u16>,
    /// Equal-column spacing in twips.
    pub column_spacing_twips: Option<u16>,
    /// Whether columns are evenly spaced.
    pub evenly_spaced_columns: Option<bool>,
    /// Whether a separator is drawn between columns.
    pub line_between_columns: Option<bool>,
    /// Whether the section uses right-to-left layout.
    pub bidi: Option<bool>,
}

/// One section of the main story.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Inclusive main-story CP.
    pub cp_start: u32,
    /// Exclusive main-story CP, clipped to `ccpText`.
    pub cp_end: u32,
    /// Exact decoded SEPX modifiers in source order.
    pub sprms: Vec<Sprm>,
    /// Known layout properties derived only from explicit SPRMs.
    pub geometry: SectionGeometry,
    /// Well-framed but currently unsupported section opcodes.
    pub unsupported_opcodes: Vec<u16>,
}

/// Ordered main-document sections.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SectionTable {
    sections: Vec<Section>,
}

impl SectionTable {
    /// Parses the FIB-referenced PLCFSED and all source-backed SEPX records.
    ///
    /// # Errors
    ///
    /// Returns a typed [`DocError`] for malformed counts/ranges, invalid SEPX
    /// offsets or property values, and configured section budget violations.
    pub fn parse(
        fib: &Fib,
        word_document: &[u8],
        table_stream: &[u8],
        limits: DocLimits,
    ) -> Result<Self> {
        let Some(location) = fib
            .locations
            .sections()
            .filter(|location| !location.is_empty())
        else {
            return Ok(Self::default());
        };
        let plc = checked_slice(table_stream, location.offset, location.length, "PlcfSed")?;
        if plc.len() < 4 || !(plc.len() - 4).is_multiple_of(16) {
            return Err(DocError::InvalidSection(format!(
                "PlcfSed length {} is not 4 + 16*n",
                plc.len()
            )));
        }
        let count = (plc.len() - 4) / 16;
        if count == 0 {
            return Err(DocError::InvalidSection("PlcfSed has no sections".into()));
        }
        if count > limits.max_sections {
            return Err(DocError::ResourceLimit {
                resource: "section",
                actual: u64::try_from(count).unwrap_or(u64::MAX),
                limit: u64::try_from(limits.max_sections).unwrap_or(u64::MAX),
            });
        }
        let cp_bytes = (count + 1) * 4;
        let mut cps = Vec::with_capacity(count + 1);
        for chunk in plc[..cp_bytes].chunks_exact(4) {
            let cp = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            cps.push(u32::try_from(cp).map_err(|_| {
                DocError::InvalidSection(format!("PlcfSed contains negative CP {cp}"))
            })?);
        }
        if cps.first().copied() != Some(0) || cps.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(DocError::InvalidSection(
                "PlcfSed CP boundaries must start at zero and increase strictly".into(),
            ));
        }
        if cps[count] < fib.stories.main {
            return Err(DocError::InvalidSection(format!(
                "PlcfSed terminal CP {} precedes main story end {}",
                cps[count], fib.stories.main
            )));
        }

        let mut sections = Vec::with_capacity(count);
        for index in 0..count {
            let cp_start = cps[index];
            if cp_start >= fib.stories.main && !(index == 0 && fib.stories.main == 0) {
                return Err(DocError::InvalidSection(format!(
                    "section {index} starts at CP {cp_start} outside main story"
                )));
            }
            let sed_start = cp_bytes + index * SED_SIZE;
            let sed = &plc[sed_start..sed_start + SED_SIZE];
            let fc_sepx = i32::from_le_bytes([sed[2], sed[3], sed[4], sed[5]]);
            let sprms = parse_sepx(word_document, fc_sepx)?;
            let (geometry, unsupported_opcodes) = apply_section_properties(&sprms)?;
            sections.push(Section {
                cp_start,
                cp_end: cps[index + 1].min(fib.stories.main),
                sprms,
                geometry,
                unsupported_opcodes,
            });
        }
        Ok(Self { sections })
    }

    /// Sections in main-story order.
    #[must_use]
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// Whether the table contains no section descriptors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    /// Number of sections.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sections.len()
    }
}

fn parse_sepx(word_document: &[u8], fc_sepx: i32) -> Result<Vec<Sprm>> {
    if fc_sepx == -1 {
        return Ok(Vec::new());
    }
    let offset = u32::try_from(fc_sepx)
        .map_err(|_| DocError::InvalidSection(format!("invalid negative fcSepx {fc_sepx}")))?;
    let prefix = checked_slice(word_document, offset, 2, "Sepx.cb")?;
    let length = i16::from_le_bytes([prefix[0], prefix[1]]);
    let length = u32::try_from(length).map_err(|_| {
        DocError::InvalidSection(format!("Sepx has negative grpprl length {length}"))
    })?;
    let grpprl_offset = offset
        .checked_add(2)
        .ok_or_else(|| DocError::InvalidSection("Sepx grpprl offset overflow".into()))?;
    let grpprl = checked_slice(word_document, grpprl_offset, length, "Sepx.grpprl")?;
    decode_grpprl(grpprl).map_err(|error| {
        DocError::InvalidSection(format!(
            "SEPX at FC {offset} cannot be framed: {error}; bytes={grpprl:02X?}"
        ))
    })
}

fn apply_section_properties(sprms: &[Sprm]) -> Result<(SectionGeometry, Vec<u16>)> {
    let mut geometry = SectionGeometry::default();
    let mut unsupported = Vec::new();
    for sprm in sprms {
        if sprm.group != PropertyGroup::Section {
            return Err(DocError::InvalidSection(format!(
                "SEPX contains non-section SPRM 0x{:04X}",
                sprm.opcode
            )));
        }
        match sprm.opcode {
            0x3005 => geometry.evenly_spaced_columns = Some(bool8(sprm)?),
            0x3009 => geometry.break_code = Some(byte(sprm)?),
            0x300A => geometry.title_page = Some(bool8(sprm)?),
            0x500B => {
                let columns_minus_one = word(sprm)?;
                if columns_minus_one > 43 {
                    return Err(DocError::InvalidSection(format!(
                        "sprmSCcolumns value {columns_minus_one} exceeds 43"
                    )));
                }
                geometry.column_count = Some(columns_minus_one + 1);
            }
            0x900C => geometry.column_spacing_twips = Some(word(sprm)?),
            0xB017 => geometry.header_distance_twips = Some(word(sprm)?),
            0xB018 => geometry.footer_distance_twips = Some(word(sprm)?),
            0x3019 => geometry.line_between_columns = Some(bool8(sprm)?),
            0x301D => {
                geometry.orientation = Some(match byte(sprm)? {
                    1 => PageOrientation::Portrait,
                    2 => PageOrientation::Landscape,
                    value => {
                        return Err(DocError::InvalidSection(format!(
                            "invalid sprmSBOrientation value {value}"
                        )));
                    }
                });
            }
            0xB01F => geometry.page_width_twips = Some(page_dimension(sprm)?),
            0xB020 => geometry.page_height_twips = Some(page_dimension(sprm)?),
            0xB021 => geometry.margin_left_twips = Some(word(sprm)?),
            0xB022 => geometry.margin_right_twips = Some(word(sprm)?),
            0x9023 => geometry.margin_top_twips = Some(signed_word(sprm)?),
            0x9024 => geometry.margin_bottom_twips = Some(signed_word(sprm)?),
            0xB025 => geometry.gutter_twips = Some(word(sprm)?),
            0x3228 => geometry.bidi = Some(bool8(sprm)?),
            _ => unsupported.push(sprm.opcode),
        }
    }
    Ok((geometry, unsupported))
}

fn byte(sprm: &Sprm) -> Result<u8> {
    sprm.operand.first().copied().ok_or_else(|| {
        DocError::InvalidSection(format!("SPRM 0x{:04X} has no byte operand", sprm.opcode))
    })
}

fn bool8(sprm: &Sprm) -> Result<bool> {
    match byte(sprm)? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(DocError::InvalidSection(format!(
            "SPRM 0x{:04X} has invalid Bool8 value {value}",
            sprm.opcode
        ))),
    }
}

fn word(sprm: &Sprm) -> Result<u16> {
    let operand: [u8; 2] = sprm.operand.as_slice().try_into().map_err(|_| {
        DocError::InvalidSection(format!(
            "SPRM 0x{:04X} does not have a two-byte operand",
            sprm.opcode
        ))
    })?;
    Ok(u16::from_le_bytes(operand))
}

fn signed_word(sprm: &Sprm) -> Result<i16> {
    let operand: [u8; 2] = sprm.operand.as_slice().try_into().map_err(|_| {
        DocError::InvalidSection(format!(
            "SPRM 0x{:04X} does not have a two-byte operand",
            sprm.opcode
        ))
    })?;
    Ok(i16::from_le_bytes(operand))
}

fn page_dimension(sprm: &Sprm) -> Result<u16> {
    let value = word(sprm)?;
    if (144..=31_680).contains(&value) {
        Ok(value)
    } else {
        Err(DocError::InvalidSection(format!(
            "SPRM 0x{:04X} page dimension {value} is outside 144..=31680",
            sprm.opcode
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_source_proven_page_geometry_and_columns() {
        let sprms = decode_grpprl(&[
            0x1F, 0xB0, 0xD0, 0x2F, // width 12240
            0x20, 0xB0, 0xE0, 0x3D, // height 15840
            0x21, 0xB0, 0xA0, 0x05, // left 1440
            0x23, 0x90, 0xA0, 0x05, // top 1440
            0x0B, 0x50, 0x01, 0x00, // two columns
            0x1D, 0x30, 0x02, // landscape
        ])
        .unwrap();
        let (geometry, unsupported) = apply_section_properties(&sprms).unwrap();
        assert_eq!(geometry.page_width_twips, Some(12_240));
        assert_eq!(geometry.page_height_twips, Some(15_840));
        assert_eq!(geometry.margin_left_twips, Some(1_440));
        assert_eq!(geometry.margin_top_twips, Some(1_440));
        assert_eq!(geometry.column_count, Some(2));
        assert_eq!(geometry.orientation, Some(PageOrientation::Landscape));
        assert!(unsupported.is_empty());
    }

    #[test]
    fn retains_unknown_section_sprms_and_rejects_invalid_values() {
        let sprms = decode_grpprl(&[0x00, 0x30, 4]).unwrap();
        let (_, unsupported) = apply_section_properties(&sprms).unwrap();
        assert_eq!(unsupported, [0x3000]);

        let invalid = decode_grpprl(&[0x1F, 0xB0, 1, 0]).unwrap();
        assert!(matches!(
            apply_section_properties(&invalid),
            Err(DocError::InvalidSection(_))
        ));
    }
}
