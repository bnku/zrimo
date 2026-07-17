//! Bounded STSH/STD/UPX parsing and cycle-safe style inheritance.

use crate::{
    CharacterPropertyDelta, DocError, DocLimits, Fib, ParagraphPropertyDelta, PropertyGroup,
    Result, Sprm, apply_character_sprms, apply_paragraph_sprms, binary::checked_slice,
    decode_grpprl,
};

const STSHIF_SIZE: usize = 18;
const NO_BASE_STYLE: u16 = 0x0FFF;

/// Style type stored in `StdfBase.stk`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleKind {
    Paragraph,
    Character,
    Table,
    Numbering,
}

/// Default font slots from `STSHI.Stshif`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultStyleFonts {
    pub ascii: i16,
    pub east_asian: i16,
    pub other: i16,
    pub bidi: Option<i16>,
}

/// One non-empty style definition in the STSH array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleDefinition {
    pub index: u16,
    /// Invariant built-in style identifier, or 0x0FFE for user-defined styles.
    pub invariant_id: u16,
    pub kind: StyleKind,
    pub base_style: Option<u16>,
    pub next_style: u16,
    pub linked_style: Option<u16>,
    /// Primary name followed by aliases, separated by commas as stored.
    pub name: String,
    /// Redundant `UpxPapx.istd` value when that optional field is present.
    /// Some real Word-produced files store zero here; it is retained rather
    /// than trusted over the containing STSH array index.
    pub paragraph_upx_style_index: Option<u16>,
    /// Paragraph differences defined directly by this style.
    pub paragraph_sprms: Vec<Sprm>,
    /// Character differences defined directly by this style.
    pub character_sprms: Vec<Sprm>,
    /// Table differences retained for the table reconstruction stage.
    pub table_sprms: Vec<Sprm>,
    /// Revision-marking UPX records retained but not applied.
    pub revision_upx: Vec<Vec<u8>>,
}

/// Inherited property arrays in base-to-derived application order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InheritedStyleProperties {
    pub paragraph_sprms: Vec<Sprm>,
    pub character_sprms: Vec<Sprm>,
    pub table_sprms: Vec<Sprm>,
    pub paragraph: ParagraphPropertyDelta,
    pub character: CharacterPropertyDelta,
}

/// Parsed document stylesheet, including empty fixed-index entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleSheet {
    styles: Vec<Option<StyleDefinition>>,
    default_fonts: DefaultStyleFonts,
    max_inheritance_depth: usize,
}

impl StyleSheet {
    /// Parses the FIB-referenced STSH structure.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed length prefixes, invalid UTF-16,
    /// unsupported style types, bad references, inheritance cycles, or limits.
    pub fn parse(fib: &Fib, table_stream: &[u8], limits: DocLimits) -> Result<Self> {
        let location = fib
            .locations
            .stylesheet()
            .filter(|location| !location.is_empty())
            .ok_or_else(|| DocError::InvalidStyle("STSH location is empty".into()))?;
        let data = checked_slice(table_stream, location.offset, location.length, "STSH")?;
        Self::parse_bytes(data, limits)
    }

    fn parse_bytes(data: &[u8], limits: DocLimits) -> Result<Self> {
        let cb_stshi = usize::from(read_u16(data, 0, "LPStshi.cbStshi")?);
        if cb_stshi < STSHIF_SIZE {
            return Err(DocError::InvalidStyle(format!(
                "STSHI is {cb_stshi} bytes; expected at least {STSHIF_SIZE}"
            )));
        }
        let styles_offset = 2_usize
            .checked_add(cb_stshi)
            .ok_or_else(|| DocError::InvalidStyle("STSHI offset overflow".into()))?;
        let stshi = data.get(2..styles_offset).ok_or_else(|| {
            DocError::InvalidStyle(format!(
                "STSHI ends at {styles_offset}, beyond STSH length {}",
                data.len()
            ))
        })?;
        let style_count = usize::from(read_u16(stshi, 0, "Stshif.cstd")?);
        if !(15..0x0FFE).contains(&style_count) {
            return Err(DocError::InvalidStyle(format!(
                "Stshif.cstd {style_count} is outside 15..0x0FFE"
            )));
        }
        if style_count > limits.max_styles {
            return Err(DocError::ResourceLimit {
                resource: "style",
                actual: u64::try_from(style_count).unwrap_or(u64::MAX),
                limit: u64::try_from(limits.max_styles).unwrap_or(u64::MAX),
            });
        }
        let stdf_size = usize::from(read_u16(stshi, 2, "Stshif.cbSTDBaseInFile")?);
        if !matches!(stdf_size, 10 | 18) {
            return Err(DocError::InvalidStyle(format!(
                "cbSTDBaseInFile is {stdf_size}; expected 10 or 18"
            )));
        }
        let default_fonts = DefaultStyleFonts {
            ascii: read_i16(stshi, 12, "Stshif.ftcAsci")?,
            east_asian: read_i16(stshi, 14, "Stshif.ftcFE")?,
            other: read_i16(stshi, 16, "Stshif.ftcOther")?,
            bidi: (stshi.len() >= 20)
                .then(|| read_i16(stshi, 18, "STSHI.ftcBi"))
                .transpose()?,
        };

        let mut offset = styles_offset;
        let mut styles = Vec::with_capacity(style_count);
        for index in 0..style_count {
            let cb_std = read_i16(data, offset, "LPStd.cbStd")?;
            if cb_std < 0 {
                return Err(DocError::InvalidStyle(format!(
                    "style {index} has negative cbStd {cb_std}"
                )));
            }
            let cb_std = usize::try_from(cb_std)
                .map_err(|_| DocError::InvalidStyle("cbStd conversion failed".into()))?;
            offset = offset
                .checked_add(2)
                .ok_or_else(|| DocError::InvalidStyle("LPStd offset overflow".into()))?;
            if cb_std == 0 {
                styles.push(None);
                continue;
            }
            let end = offset
                .checked_add(cb_std)
                .ok_or_else(|| DocError::InvalidStyle("STD end overflow".into()))?;
            let bytes = data.get(offset..end).ok_or_else(|| {
                DocError::InvalidStyle(format!(
                    "style {index} ends at {end}, beyond STSH length {}",
                    data.len()
                ))
            })?;
            styles.push(Some(parse_style(
                u16::try_from(index)
                    .map_err(|_| DocError::InvalidStyle("style index overflow".into()))?,
                bytes,
                stdf_size,
            )?));
            offset = end
                .checked_add(cb_std % 2)
                .ok_or_else(|| DocError::InvalidStyle("LPStd padding overflow".into()))?;
            if offset > data.len() {
                return Err(DocError::InvalidStyle(format!(
                    "style {index} padding exceeds STSH"
                )));
            }
        }
        if offset != data.len() {
            return Err(DocError::InvalidStyle(format!(
                "{} trailing bytes follow the STSH style array",
                data.len() - offset
            )));
        }

        let result = Self {
            styles,
            default_fonts,
            max_inheritance_depth: limits.max_style_depth,
        };
        result.validate_references_and_cycles()?;
        Ok(result)
    }

    #[must_use]
    pub fn styles(&self) -> &[Option<StyleDefinition>] {
        &self.styles
    }

    #[must_use]
    pub const fn default_fonts(&self) -> DefaultStyleFonts {
        self.default_fonts
    }

    /// Character font modifiers derived from source STSH default slots.
    #[must_use]
    pub fn default_character_sprms(&self) -> Vec<Sprm> {
        let mut result = Vec::with_capacity(4);
        for (opcode, value) in [
            (0x4A4F, Some(self.default_fonts.ascii)),
            (0x4A50, Some(self.default_fonts.east_asian)),
            (0x4A51, Some(self.default_fonts.other)),
            (0x4A5E, self.default_fonts.bidi),
        ] {
            if let Some(value) = value.and_then(|value| u16::try_from(value).ok()) {
                result.push(Sprm {
                    opcode,
                    property_id: opcode & 0x01FF,
                    special: opcode & 0x0200 != 0,
                    group: PropertyGroup::Character,
                    operand: value.to_le_bytes().to_vec(),
                });
            }
        }
        result
    }

    #[must_use]
    pub fn get(&self, index: u16) -> Option<&StyleDefinition> {
        self.styles.get(usize::from(index))?.as_ref()
    }

    /// Resolves a style's base chain in base-to-derived order.
    ///
    /// # Errors
    ///
    /// Returns a typed stylesheet or formatting error if references changed,
    /// a depth budget is exceeded, or a known style property is invalid.
    pub fn inherited_properties(&self, index: u16) -> Result<InheritedStyleProperties> {
        let mut order = Vec::new();
        let mut current = Some(index);
        while let Some(style_index) = current {
            if order.len() >= self.max_inheritance_depth {
                return Err(DocError::ResourceLimit {
                    resource: "style-inheritance-depth",
                    actual: u64::try_from(order.len() + 1).unwrap_or(u64::MAX),
                    limit: u64::try_from(self.max_inheritance_depth).unwrap_or(u64::MAX),
                });
            }
            if order.contains(&style_index) {
                return Err(DocError::InvalidStyle(format!(
                    "style inheritance cycle reaches {style_index}"
                )));
            }
            let style = self.get(style_index).ok_or_else(|| {
                DocError::InvalidStyle(format!("style {style_index} is absent or empty"))
            })?;
            order.push(style_index);
            current = style.base_style;
        }
        order.reverse();

        let mut paragraph_sprms = Vec::new();
        let mut character_sprms = Vec::new();
        let mut table_sprms = Vec::new();
        for style_index in order {
            let style = self.get(style_index).ok_or_else(|| {
                DocError::InvalidStyle(format!("style {style_index} disappeared"))
            })?;
            paragraph_sprms.extend(style.paragraph_sprms.iter().cloned());
            character_sprms.extend(style.character_sprms.iter().cloned());
            table_sprms.extend(style.table_sprms.iter().cloned());
        }
        let paragraph = apply_paragraph_sprms(&paragraph_sprms)?;
        let character = apply_character_sprms(&character_sprms)?;
        Ok(InheritedStyleProperties {
            paragraph_sprms,
            character_sprms,
            table_sprms,
            paragraph,
            character,
        })
    }

    fn validate_references_and_cycles(&self) -> Result<()> {
        for style in self.styles.iter().flatten() {
            if let Some(base) = style.base_style {
                let parent = self.get(base).ok_or_else(|| {
                    DocError::InvalidStyle(format!(
                        "style {} references absent base style {base}",
                        style.index
                    ))
                })?;
                if parent.kind != style.kind {
                    return Err(DocError::InvalidStyle(format!(
                        "style {} ({:?}) inherits from style {base} ({:?})",
                        style.index, style.kind, parent.kind
                    )));
                }
            }
            if self.get(style.next_style).is_none() {
                return Err(DocError::InvalidStyle(format!(
                    "style {} references absent next style {}",
                    style.index, style.next_style
                )));
            }
            if let Some(linked) = style.linked_style
                && self.get(linked).is_none()
            {
                return Err(DocError::InvalidStyle(format!(
                    "style {} references absent linked style {linked}",
                    style.index
                )));
            }
            self.inheritance_chain(style.index)?;
        }
        Ok(())
    }

    fn inheritance_chain(&self, start: u16) -> Result<()> {
        let mut visited = Vec::new();
        let mut current = Some(start);
        while let Some(index) = current {
            if visited.len() >= self.max_inheritance_depth {
                return Err(DocError::ResourceLimit {
                    resource: "style-inheritance-depth",
                    actual: u64::try_from(visited.len() + 1).unwrap_or(u64::MAX),
                    limit: u64::try_from(self.max_inheritance_depth).unwrap_or(u64::MAX),
                });
            }
            if visited.contains(&index) {
                return Err(DocError::InvalidStyle(format!(
                    "style inheritance cycle reaches {index}"
                )));
            }
            visited.push(index);
            current = self
                .get(index)
                .ok_or_else(|| DocError::InvalidStyle(format!("style {index} is empty")))?
                .base_style;
        }
        Ok(())
    }
}

fn parse_style(index: u16, data: &[u8], stdf_size: usize) -> Result<StyleDefinition> {
    if data.len() < stdf_size {
        return Err(DocError::InvalidStyle(format!(
            "style {index} is {} bytes, shorter than Stdf size {stdf_size}",
            data.len()
        )));
    }
    let word0 = read_u16(data, 0, "StdfBase.sti")?;
    let word1 = read_u16(data, 2, "StdfBase.stk")?;
    let word2 = read_u16(data, 4, "StdfBase.cupx")?;
    let recorded_size = usize::from(read_u16(data, 6, "StdfBase.bchUpe")?);
    if recorded_size != data.len() {
        return Err(DocError::InvalidStyle(format!(
            "style {index} bchUpe is {recorded_size}; LPStd.cbStd is {}",
            data.len()
        )));
    }
    let invariant_id = word0 & 0x0FFF;
    let kind = match word1 & 0x000F {
        1 => StyleKind::Paragraph,
        2 => StyleKind::Character,
        3 => StyleKind::Table,
        4 => StyleKind::Numbering,
        value => {
            return Err(DocError::InvalidStyle(format!(
                "style {index} has invalid stk {value}"
            )));
        }
    };
    let raw_base = word1 >> 4;
    let base_style = (raw_base != NO_BASE_STYLE).then_some(raw_base);
    let upx_count = usize::from(word2 & 0x000F);
    let next_style = word2 >> 4;
    let linked_style = if stdf_size == 18 {
        let post = read_u16(data, 10, "StdfPost2000.istdLink")?;
        let linked = post & 0x0FFF;
        (linked != 0).then_some(linked)
    } else {
        None
    };
    validate_upx_count(index, kind, upx_count)?;

    let (name, groups_offset) = parse_style_name(index, data, stdf_size)?;
    let groups = parse_upx_groups(index, data, groups_offset, upx_count)?;

    let mut paragraph_sprms = Vec::new();
    let mut paragraph_upx_style_index = None;
    let mut character_sprms = Vec::new();
    let mut table_sprms = Vec::new();
    let mut revision_upx = Vec::new();
    match kind {
        StyleKind::Paragraph => {
            (paragraph_upx_style_index, paragraph_sprms) = decode_style_papx(index, &groups[0])?;
            character_sprms = decode_expected_group(index, &groups[1], PropertyGroup::Character)?;
            revision_upx.extend(groups.into_iter().skip(2));
        }
        StyleKind::Character => {
            character_sprms = decode_expected_group(index, &groups[0], PropertyGroup::Character)?;
            revision_upx.extend(groups.into_iter().skip(1));
        }
        StyleKind::Table => {
            table_sprms = decode_expected_group(index, &groups[0], PropertyGroup::Table)?;
            (paragraph_upx_style_index, paragraph_sprms) = decode_style_papx(index, &groups[1])?;
            character_sprms = decode_expected_group(index, &groups[2], PropertyGroup::Character)?;
        }
        StyleKind::Numbering => {
            let group = &groups[0];
            paragraph_sprms = if group.len() == 2 {
                // An optional redundant istd with an empty grpprl.
                Vec::new()
            } else if group.len() > 2 && read_u16(group, 0, "numbering UpxPapx.istd")? == index {
                decode_expected_group(index, &group[2..], PropertyGroup::Paragraph)?
            } else {
                decode_expected_group(index, group, PropertyGroup::Paragraph)?
            };
        }
    }
    Ok(StyleDefinition {
        index,
        invariant_id,
        kind,
        base_style,
        next_style,
        linked_style,
        name,
        paragraph_upx_style_index,
        paragraph_sprms,
        character_sprms,
        table_sprms,
        revision_upx,
    })
}

fn parse_style_name(index: u16, data: &[u8], stdf_size: usize) -> Result<(String, usize)> {
    let name_length = usize::from(read_u16(data, stdf_size, "STD.xstzName.cch")?);
    let name_bytes = name_length
        .checked_mul(2)
        .ok_or_else(|| DocError::InvalidStyle("style-name byte length overflow".into()))?;
    let name_start = stdf_size + 2;
    let name_end = name_start
        .checked_add(name_bytes)
        .ok_or_else(|| DocError::InvalidStyle("style-name end overflow".into()))?;
    let terminator_end = name_end
        .checked_add(2)
        .ok_or_else(|| DocError::InvalidStyle("style-name terminator overflow".into()))?;
    let encoded_name = data.get(name_start..name_end).ok_or_else(|| {
        DocError::InvalidStyle(format!("style {index} name exceeds its STD record"))
    })?;
    if read_u16(data, name_end, "STD.xstzName.terminator")? != 0 {
        return Err(DocError::InvalidStyle(format!(
            "style {index} name has a nonzero terminator"
        )));
    }
    let utf16 = encoded_name
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let name = String::from_utf16(&utf16)
        .map_err(|_| DocError::InvalidStyle(format!("style {index} name is invalid UTF-16")))?;
    if name.is_empty() || name.split(',').any(str::is_empty) {
        return Err(DocError::InvalidStyle(format!(
            "style {index} contains an empty style name"
        )));
    }
    Ok((name, terminator_end))
}

fn parse_upx_groups(
    index: u16,
    data: &[u8],
    mut offset: usize,
    upx_count: usize,
) -> Result<Vec<Vec<u8>>> {
    let mut groups = Vec::with_capacity(upx_count);
    for group_index in 0..upx_count {
        let length = usize::from(read_u16(data, offset, "LPUpx.cbUpx")?);
        offset = offset
            .checked_add(2)
            .ok_or_else(|| DocError::InvalidStyle("UPX offset overflow".into()))?;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| DocError::InvalidStyle("UPX end overflow".into()))?;
        let upx = data.get(offset..end).ok_or_else(|| {
            DocError::InvalidStyle(format!(
                "style {index} UPX {group_index} exceeds its STD record"
            ))
        })?;
        groups.push(upx.to_vec());
        offset = end
            .checked_add(length % 2)
            .ok_or_else(|| DocError::InvalidStyle("UPX padding overflow".into()))?;
    }
    if offset != data.len() {
        return Err(DocError::InvalidStyle(format!(
            "style {index} has {} trailing bytes after UPX records",
            data.len().saturating_sub(offset)
        )));
    }
    Ok(groups)
}

fn validate_upx_count(index: u16, kind: StyleKind, count: usize) -> Result<()> {
    let valid = match kind {
        StyleKind::Paragraph => matches!(count, 2 | 3),
        StyleKind::Character => matches!(count, 1 | 2),
        StyleKind::Table => count == 3,
        StyleKind::Numbering => count == 1,
    };
    if valid {
        Ok(())
    } else {
        Err(DocError::InvalidStyle(format!(
            "style {index} ({kind:?}) has invalid cupx {count}"
        )))
    }
}

fn decode_style_papx(index: u16, upx: &[u8]) -> Result<(Option<u16>, Vec<Sprm>)> {
    let stored_style = read_u16(upx, 0, "UpxPapx.istd")?;
    let grpprl = &upx[2..];
    // Word 2003 sometimes emits a two-byte zero compatibility pad for an
    // otherwise empty UpxPapx. Zero is not a valid paragraph SPRM opcode, so
    // treating an all-zero remainder as padding is unambiguous.
    if grpprl.iter().all(|byte| *byte == 0) {
        return Ok((Some(stored_style), Vec::new()));
    }
    Ok((
        Some(stored_style),
        decode_expected_group(index, grpprl, PropertyGroup::Paragraph)?,
    ))
}

fn decode_expected_group(index: u16, data: &[u8], group: PropertyGroup) -> Result<Vec<Sprm>> {
    let sprms = decode_grpprl(data).map_err(|error| {
        DocError::InvalidStyle(format!(
            "style {index} {group:?} UPX cannot be framed: {error}; bytes={data:02X?}"
        ))
    })?;
    if let Some(sprm) = sprms.iter().find(|sprm| sprm.group != group) {
        return Err(DocError::InvalidStyle(format!(
            "style {index} {:?} UPX contains {:?} SPRM 0x{:04X}",
            group, sprm.group, sprm.opcode
        )));
    }
    Ok(sprms)
}

fn read_u16(data: &[u8], offset: usize, field: &'static str) -> Result<u16> {
    let end = offset
        .checked_add(2)
        .ok_or_else(|| DocError::InvalidStyle(format!("{field} offset overflow")))?;
    let bytes = data.get(offset..end).ok_or_else(|| {
        DocError::InvalidStyle(format!(
            "{field} range [{offset}, {end}) exceeds {} bytes",
            data.len()
        ))
    })?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_i16(data: &[u8], offset: usize, field: &'static str) -> Result<i16> {
    Ok(i16::from_le_bytes(
        read_u16(data, offset, field)?.to_le_bytes(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToggleValue;

    #[test]
    fn parses_styles_and_applies_base_before_child() {
        let sheet = StyleSheet::parse_bytes(
            &stylesheet(&[
                paragraph_style(0, NO_BASE_STYLE, "Normal", &[0x35, 0x08, 0], &[]),
                paragraph_style(1, 0, "Heading 1", &[0x35, 0x08, 1], &[0x05, 0x24, 1]),
            ]),
            DocLimits::default(),
        )
        .unwrap();
        assert_eq!(sheet.styles().len(), 15);
        assert_eq!(sheet.get(1).unwrap().name, "Heading 1");
        let inherited = sheet.inherited_properties(1).unwrap();
        assert_eq!(inherited.character.bold, Some(ToggleValue::On));
        assert_eq!(inherited.paragraph.keep_together, Some(true));
        assert_eq!(
            inherited
                .character_sprms
                .iter()
                .map(|sprm| sprm.operand[0])
                .collect::<Vec<_>>(),
            [0, 1]
        );
    }

    #[test]
    fn rejects_cycles_and_invalid_base_references() {
        let cycle = stylesheet(&[
            paragraph_style(0, 1, "A", &[], &[]),
            paragraph_style(1, 0, "B", &[], &[]),
        ]);
        assert!(matches!(
            StyleSheet::parse_bytes(&cycle, DocLimits::default()),
            Err(DocError::InvalidStyle(_))
        ));

        let invalid = stylesheet(&[paragraph_style(0, 14, "A", &[], &[])]);
        assert!(matches!(
            StyleSheet::parse_bytes(&invalid, DocLimits::default()),
            Err(DocError::InvalidStyle(_))
        ));
    }

    fn stylesheet(nonempty: &[Vec<u8>]) -> Vec<u8> {
        let mut stshi = vec![0_u8; STSHIF_SIZE];
        stshi[0..2].copy_from_slice(&15_u16.to_le_bytes());
        stshi[2..4].copy_from_slice(&10_u16.to_le_bytes());
        stshi[4..6].copy_from_slice(&1_u16.to_le_bytes());
        stshi[8..10].copy_from_slice(&15_u16.to_le_bytes());
        stshi[12..14].copy_from_slice(&1_i16.to_le_bytes());
        stshi[14..16].copy_from_slice(&2_i16.to_le_bytes());
        stshi[16..18].copy_from_slice(&3_i16.to_le_bytes());
        let mut result = Vec::new();
        result.extend_from_slice(&u16::try_from(STSHIF_SIZE).unwrap().to_le_bytes());
        result.extend_from_slice(&stshi);
        for index in 0..15 {
            if let Some(style) = nonempty.get(index) {
                result.extend_from_slice(&i16::try_from(style.len()).unwrap().to_le_bytes());
                result.extend_from_slice(style);
                if style.len() % 2 != 0 {
                    result.push(0);
                }
            } else {
                result.extend_from_slice(&0_i16.to_le_bytes());
            }
        }
        result
    }

    fn paragraph_style(index: u16, base: u16, name: &str, chpx: &[u8], papx: &[u8]) -> Vec<u8> {
        let mut result = vec![0_u8; 10];
        result[0..2].copy_from_slice(&(0x0FFE_u16).to_le_bytes());
        result[2..4].copy_from_slice(&((base << 4) | 1).to_le_bytes());
        result[4..6].copy_from_slice(&((index << 4) | 2).to_le_bytes());
        let utf16 = name.encode_utf16().collect::<Vec<_>>();
        result.extend_from_slice(&u16::try_from(utf16.len()).unwrap().to_le_bytes());
        for unit in utf16 {
            result.extend_from_slice(&unit.to_le_bytes());
        }
        result.extend_from_slice(&0_u16.to_le_bytes());
        let mut upx_papx = index.to_le_bytes().to_vec();
        upx_papx.extend_from_slice(papx);
        push_upx(&mut result, &upx_papx);
        push_upx(&mut result, chpx);
        let size = u16::try_from(result.len()).unwrap();
        result[6..8].copy_from_slice(&size.to_le_bytes());
        result
    }

    fn push_upx(output: &mut Vec<u8>, bytes: &[u8]) {
        output.extend_from_slice(&u16::try_from(bytes.len()).unwrap().to_le_bytes());
        output.extend_from_slice(bytes);
        if !bytes.len().is_multiple_of(2) {
            output.push(0);
        }
    }
}
