//! Source-backed Word list definitions and format overrides.

use std::collections::HashSet;

use crate::{
    DocError, DocLimits, PropertyGroup, Sprm, WordBinaryDocument,
    binary::{ByteCursor, checked_slice},
    decode_grpprl,
};

const LSTF_SIZE: usize = 28;
const LVLF_SIZE: usize = 28;
const LFO_SIZE: usize = 16;
const LFOLVL_BASE_SIZE: usize = 8;
const MAX_LEVELS: usize = 9;
const MAX_LEVELS_U8: u8 = 9;
const MAX_LEVELS_U16: u16 = 9;

/// Character emitted after a rendered list marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListFollow {
    /// A tab follows the marker.
    Tab,
    /// A space follows the marker.
    Space,
    /// Nothing follows the marker.
    Nothing,
}

/// Formatting for one level of a Word list definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListLevel {
    /// Initial sequence value.
    pub start_at: u32,
    /// Source `MSONFC` numbering-format code.
    pub number_format: u8,
    /// Marker justification: 0 left, 1 center, 2 right.
    pub justification: u8,
    /// Force inherited placeholders to Arabic numbering.
    pub legal_numbering: bool,
    /// Do not restart at every more-significant level.
    pub no_restart: bool,
    /// First significant level after which this level does not restart.
    pub restart_limit: u8,
    /// Character following the list marker.
    pub follow: ListFollow,
    /// Raw placeholder positions retained from `LVLF.rgbxchNums`.
    pub placeholder_offsets: [u8; MAX_LEVELS],
    /// Marker template as UTF-16; placeholder units are level indexes.
    pub level_text: Vec<u16>,
    /// Paragraph properties applied by this list level.
    pub paragraph_sprms: Vec<Sprm>,
    /// Character properties applied to the marker.
    pub character_sprms: Vec<Sprm>,
}

impl ListLevel {
    /// Whether the level is a bullet rather than a numeric sequence.
    #[must_use]
    pub const fn is_bullet(&self) -> bool {
        self.number_format == 0x17
    }

    /// Convert Word placeholder units into OOXML `%1`…`%9` tokens.
    #[must_use]
    pub fn ooxml_level_text(&self) -> String {
        let placeholders = self
            .placeholder_offsets
            .iter()
            .copied()
            .take_while(|offset| *offset != 0)
            .collect::<HashSet<_>>();
        let mut result = String::new();
        for (index, unit) in self.level_text.iter().copied().enumerate() {
            let one_based = u8::try_from(index + 1).unwrap_or(u8::MAX);
            if placeholders.contains(&one_based) && unit < MAX_LEVELS_U16 {
                result.push('%');
                result.push(char::from(b'1' + u8::try_from(unit).unwrap_or(8)));
            } else if self.is_bullet() {
                result.push(char::from_u32(u32::from(unit & 0x0FFF)).unwrap_or('\u{FFFD}'));
            } else {
                result.push(char::from_u32(u32::from(unit)).unwrap_or('\u{FFFD}'));
            }
        }
        result
    }
}

/// One `LSTF` and its one or nine appended levels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListDefinition {
    /// Unique source list identifier.
    pub lsid: i32,
    /// Whether only level zero exists.
    pub simple: bool,
    /// Whether unused levels are tentative hybrid-list levels.
    pub hybrid: bool,
    /// Paragraph style indexes linked to levels (`0x0FFF` means none).
    pub paragraph_styles: [u16; MAX_LEVELS],
    /// Source levels in level order.
    pub levels: Vec<ListLevel>,
}

/// Per-level formatting or start-value override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListLevelOverride {
    /// Zero-based list level.
    pub level: u8,
    /// Start override when no complete formatting override is present.
    pub start_at: Option<u32>,
    /// Complete replacement level.
    pub formatting: Option<ListLevel>,
}

/// One `LFO` list instance referenced by paragraph `sprmPIlfo` values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListOverride {
    /// Stable OOXML-compatible 1-based instance identifier.
    pub num_id: u32,
    /// Identifier of the corresponding source definition.
    pub lsid: i32,
    /// First main-story paragraph CP, when recorded by Word.
    pub first_cp: Option<u32>,
    /// Per-level overrides.
    pub levels: Vec<ListLevelOverride>,
}

/// Resolved list identity attached to one projected paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedListParagraph {
    /// Main-story paragraph CP used to make the projection marker unique.
    pub cp: u32,
    /// 1-based list instance identifier.
    pub num_id: u32,
    /// Zero-based level.
    pub level: u8,
    /// Negative `ilfo` preserves direct paragraph indents.
    pub preserve_indents: bool,
}

impl ResolvedListParagraph {
    /// Private-use marker replaced by the bridge with an OOXML `numPr`.
    #[doc(hidden)]
    #[must_use]
    pub fn projection_marker(self) -> String {
        format!(
            "\u{F0000}ZRIMO_LIST_{:08X}_{:08X}_{:02X}\u{F0001}",
            self.cp, self.num_id, self.level
        )
    }
}

/// Validated Word list definitions and instances.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListCollection {
    definitions: Vec<ListDefinition>,
    overrides: Vec<ListOverride>,
}

impl ListCollection {
    pub(crate) fn parse(document: &WordBinaryDocument, limits: DocLimits) -> crate::Result<Self> {
        let definition_location = document
            .fib()
            .locations
            .list_definitions()
            .filter(|location| !location.is_empty());
        let override_location = document
            .fib()
            .locations
            .list_overrides()
            .filter(|location| !location.is_empty());
        if definition_location.is_none() && override_location.is_none() {
            return Ok(Self::default());
        }
        let Some(definition_location) = definition_location else {
            return Err(DocError::InvalidList(
                "PlfLfo is present while PlfLst is absent".into(),
            ));
        };
        let definitions = parse_definitions(
            document.table_stream(),
            definition_location.offset,
            definition_location.length,
            limits,
        )?;
        let overrides = if let Some(location) = override_location {
            let bytes = checked_slice(
                document.table_stream(),
                location.offset,
                location.length,
                "PlfLfo",
            )?;
            parse_overrides(bytes, document.fib().stories.main, limits)?
        } else {
            Vec::new()
        };
        for list in &overrides {
            if !definitions
                .iter()
                .any(|definition| definition.lsid == list.lsid)
            {
                return Err(DocError::InvalidList(format!(
                    "LFO numId {} references missing LSTF lsid {}",
                    list.num_id, list.lsid
                )));
            }
        }
        Ok(Self {
            definitions,
            overrides,
        })
    }

    /// Source definitions in `PlfLst` order.
    #[must_use]
    pub fn definitions(&self) -> &[ListDefinition] {
        &self.definitions
    }

    /// Source instances in 1-based `ilfo` order.
    #[must_use]
    pub fn overrides(&self) -> &[ListOverride] {
        &self.overrides
    }

    /// Find a definition by its source list identifier.
    #[must_use]
    pub fn definition(&self, lsid: i32) -> Option<&ListDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.lsid == lsid)
    }

    /// Resolve paragraph `ilfo`/`ilvl` values to a validated list instance.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an out-of-range list instance or level.
    pub fn resolve_paragraph(
        &self,
        cp: u32,
        ilfo: i16,
        ilvl: u8,
    ) -> crate::Result<Option<ResolvedListParagraph>> {
        if ilfo == 0 || ilfo == -2047 {
            return Ok(None);
        }
        let preserve_indents = ilfo < 0;
        let absolute = i32::from(ilfo).unsigned_abs();
        if absolute == 0 || absolute > 0x07FE {
            return Err(DocError::InvalidList(format!(
                "paragraph at CP {cp} has invalid ilfo {ilfo}"
            )));
        }
        if ilvl > 8 {
            return Err(DocError::InvalidList(format!(
                "paragraph at CP {cp} has invalid ilvl {ilvl}"
            )));
        }
        let list = self
            .overrides
            .get(usize::try_from(absolute - 1).unwrap_or(usize::MAX))
            .ok_or_else(|| {
                DocError::InvalidList(format!(
                    "paragraph at CP {cp} references missing LFO {absolute}"
                ))
            })?;
        let definition = self.definition(list.lsid).ok_or_else(|| {
            DocError::InvalidList(format!(
                "LFO {absolute} references missing LSTF lsid {}",
                list.lsid
            ))
        })?;
        if usize::from(ilvl) >= definition.levels.len() {
            return Err(DocError::InvalidList(format!(
                "paragraph at CP {cp} selects level {ilvl} of {}-level list {}",
                definition.levels.len(),
                list.lsid
            )));
        }
        Ok(Some(ResolvedListParagraph {
            cp,
            num_id: absolute,
            level: ilvl,
            preserve_indents,
        }))
    }
}

fn parse_definitions(
    table_stream: &[u8],
    offset: u32,
    length: u32,
    limits: DocLimits,
) -> crate::Result<Vec<ListDefinition>> {
    let header = checked_slice(table_stream, offset, length, "PlfLst")?;
    let mut cursor = ByteCursor::new(header, "PlfLst");
    let count = i16::from_le_bytes(cursor.take(2)?.try_into().expect("two-byte slice"));
    let count = usize::try_from(count)
        .map_err(|_| DocError::InvalidList(format!("PlfLst cLst is negative: {count}")))?;
    enforce_count("list definition", count, limits.max_lists)?;
    let expected = 2_usize
        .checked_add(
            count
                .checked_mul(LSTF_SIZE)
                .ok_or_else(|| DocError::InvalidList("PlfLst LSTF byte count overflow".into()))?,
        )
        .ok_or_else(|| DocError::InvalidList("PlfLst length overflow".into()))?;
    if header.len() != expected {
        return Err(DocError::InvalidList(format!(
            "PlfLst header has {} bytes; expected {expected} for {count} LSTFs",
            header.len()
        )));
    }
    let mut definitions = Vec::with_capacity(count);
    let mut identifiers = HashSet::with_capacity(count);
    for _ in 0..count {
        let raw = cursor.take(LSTF_SIZE)?;
        let lsid = i32::from_le_bytes(raw[0..4].try_into().expect("four-byte slice"));
        if lsid == -1 || !identifiers.insert(lsid) {
            return Err(DocError::InvalidList(format!(
                "LSTF lsid {lsid} is reserved or duplicated"
            )));
        }
        let mut paragraph_styles = [0_u16; MAX_LEVELS];
        for (index, slot) in paragraph_styles.iter_mut().enumerate() {
            let start = 8 + index * 2;
            *slot = u16::from_le_bytes([raw[start], raw[start + 1]]);
            if *slot > 0x0FFF {
                return Err(DocError::InvalidList(format!(
                    "LSTF {lsid} has invalid linked style index {:#06X}",
                    *slot
                )));
            }
        }
        let flags = raw[26];
        if flags & 0xE0 != 0 {
            return Err(DocError::InvalidList(format!(
                "LSTF {lsid} has nonzero reserved flag bits {:#04X}",
                flags & 0xE0
            )));
        }
        definitions.push(ListDefinition {
            lsid,
            simple: flags & 0x01 != 0,
            hybrid: flags & 0x10 != 0,
            paragraph_styles,
            levels: Vec::new(),
        });
    }
    let level_offset = usize::try_from(offset)
        .ok()
        .and_then(|value| value.checked_add(header.len()))
        .ok_or_else(|| DocError::InvalidList("PlfLst appended LVL offset overflow".into()))?;
    let level_bytes = table_stream
        .get(level_offset..)
        .ok_or(DocError::OutOfBounds {
            structure: "PlfLst appended LVLs",
            offset: level_offset,
            end: level_offset,
            available: table_stream.len(),
        })?;
    let mut levels = ByteCursor::new(level_bytes, "PlfLst appended LVLs");
    for definition in &mut definitions {
        let level_count = if definition.simple { 1 } else { MAX_LEVELS };
        for level_index in 0..level_count {
            definition.levels.push(parse_level(
                &mut levels,
                u8::try_from(level_index).unwrap_or(8),
            )?);
        }
    }
    Ok(definitions)
}

fn parse_level(cursor: &mut ByteCursor<'_>, level_index: u8) -> crate::Result<ListLevel> {
    let raw = cursor.take(LVLF_SIZE)?;
    let start_at = i32::from_le_bytes(raw[0..4].try_into().expect("four-byte slice"));
    let number_format = raw[4];
    if matches!(number_format, 0x08 | 0x09 | 0x0F | 0x13) {
        return Err(DocError::InvalidList(format!(
            "level {level_index} uses reserved MSONFC {number_format:#04X}"
        )));
    }
    let start_at = if matches!(number_format, 0x17 | 0xFF) {
        0
    } else {
        u32::try_from(start_at).map_err(|_| {
            DocError::InvalidList(format!("level {level_index} has negative start {start_at}"))
        })?
    };
    if start_at > 0x7FFF {
        return Err(DocError::InvalidList(format!(
            "level {level_index} start {start_at} exceeds 0x7FFF"
        )));
    }
    let info = raw[5];
    let justification = info & 0x03;
    if justification == 3 {
        return Err(DocError::InvalidList(format!(
            "level {level_index} has invalid justification 3"
        )));
    }
    let placeholder_offsets: [u8; MAX_LEVELS] =
        raw[6..15].try_into().expect("nine-byte placeholder slice");
    validate_placeholder_shape(&placeholder_offsets, level_index)?;
    let follow = match raw[15] {
        0 => ListFollow::Tab,
        1 => ListFollow::Space,
        2 => ListFollow::Nothing,
        value => {
            return Err(DocError::InvalidList(format!(
                "level {level_index} has invalid ixchFollow {value}"
            )));
        }
    };
    let chpx_length = usize::from(raw[24]);
    let papx_length = usize::from(raw[25]);
    let restart_limit = raw[26];
    if info & 0x08 != 0 && restart_limit > level_index {
        return Err(DocError::InvalidList(format!(
            "level {level_index} restart limit {restart_limit} exceeds its level"
        )));
    }
    let papx = cursor.take(papx_length)?;
    let chpx = cursor.take(chpx_length)?;
    let paragraph_sprms = decode_list_grpprl(papx, PropertyGroup::Paragraph, level_index)?;
    let character_sprms = decode_list_grpprl(chpx, PropertyGroup::Character, level_index)?;
    let text_length = usize::from(cursor.read_u16()?);
    let text_bytes = cursor.take(
        text_length
            .checked_mul(2)
            .ok_or_else(|| DocError::InvalidList("LVL xst byte length overflow".into()))?,
    )?;
    let level_text = text_bytes
        .chunks_exact(2)
        .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
        .collect::<Vec<_>>();
    validate_placeholders(
        &placeholder_offsets,
        &level_text,
        level_index,
        number_format,
    )?;
    Ok(ListLevel {
        start_at,
        number_format,
        justification,
        legal_numbering: info & 0x04 != 0,
        no_restart: info & 0x08 != 0,
        restart_limit,
        follow,
        placeholder_offsets,
        level_text,
        paragraph_sprms,
        character_sprms,
    })
}

fn decode_list_grpprl(
    bytes: &[u8],
    expected: PropertyGroup,
    level: u8,
) -> crate::Result<Vec<Sprm>> {
    let sprms = decode_grpprl(bytes).map_err(|error| {
        DocError::InvalidList(format!(
            "level {level} {expected:?} grpprl cannot be framed: {error}"
        ))
    })?;
    if let Some(sprm) = sprms.iter().find(|sprm| sprm.group != expected) {
        return Err(DocError::InvalidList(format!(
            "level {level} {expected:?} grpprl contains {:?} SPRM 0x{:04X}",
            sprm.group, sprm.opcode
        )));
    }
    Ok(sprms)
}

fn validate_placeholder_shape(offsets: &[u8; MAX_LEVELS], level: u8) -> crate::Result<()> {
    let mut previous = 0_u8;
    let mut terminated = false;
    let mut count = 0_usize;
    for offset in offsets {
        if *offset == 0 {
            terminated = true;
            continue;
        }
        if terminated || *offset <= previous {
            return Err(DocError::InvalidList(format!(
                "level {level} placeholder offsets are not a unique ascending zero-terminated prefix"
            )));
        }
        previous = *offset;
        count += 1;
    }
    if count > usize::from(level) + 1 {
        return Err(DocError::InvalidList(format!(
            "level {level} has {count} placeholders"
        )));
    }
    Ok(())
}

fn validate_placeholders(
    offsets: &[u8; MAX_LEVELS],
    text: &[u16],
    level: u8,
    number_format: u8,
) -> crate::Result<()> {
    let used = offsets.iter().copied().take_while(|offset| *offset != 0);
    if number_format == 0x17 {
        if text.len() != 1 || used.count() != 0 {
            return Err(DocError::InvalidList(format!(
                "bullet level {level} must contain exactly one non-placeholder character"
            )));
        }
        return Ok(());
    }
    for offset in used {
        let unit = text.get(usize::from(offset - 1)).ok_or_else(|| {
            DocError::InvalidList(format!(
                "level {level} placeholder offset {offset} exceeds xst length {}",
                text.len()
            ))
        })?;
        if *unit > u16::from(level) {
            return Err(DocError::InvalidList(format!(
                "level {level} placeholder references deeper level {unit}"
            )));
        }
    }
    Ok(())
}

#[derive(Debug)]
struct RawLfo {
    lsid: i32,
    override_count: u8,
}

fn parse_lfo_headers(
    cursor: &mut ByteCursor<'_>,
    total_length: usize,
    count: usize,
) -> crate::Result<Vec<RawLfo>> {
    let header_bytes = count
        .checked_mul(LFO_SIZE)
        .ok_or_else(|| DocError::InvalidList("PlfLfo LFO byte count overflow".into()))?;
    if total_length.saturating_sub(cursor.position()) < header_bytes {
        return Err(DocError::InvalidList(format!(
            "PlfLfo has {total_length} bytes but needs {header_bytes} LFO header bytes"
        )));
    }
    let mut raw_lfos = Vec::with_capacity(count);
    for _ in 0..count {
        let raw = cursor.take(LFO_SIZE)?;
        let lsid = i32::from_le_bytes(raw[0..4].try_into().expect("four-byte slice"));
        let override_count = raw[12];
        if override_count > MAX_LEVELS_U8 {
            return Err(DocError::InvalidList(format!(
                "LFO {lsid} has {override_count} level overrides"
            )));
        }
        if !matches!(raw[13], 0x00 | 0xFC..=0xFF) {
            return Err(DocError::InvalidList(format!(
                "LFO {lsid} has invalid ibstFltAutoNum {:#04X}",
                raw[13]
            )));
        }
        raw_lfos.push(RawLfo {
            lsid,
            override_count,
        });
    }
    Ok(raw_lfos)
}

fn parse_overrides(
    bytes: &[u8],
    main_length: u32,
    limits: DocLimits,
) -> crate::Result<Vec<ListOverride>> {
    let mut cursor = ByteCursor::new(bytes, "PlfLfo");
    let count = usize::try_from(cursor.read_u32()?)
        .map_err(|_| DocError::InvalidList("PlfLfo count does not fit usize".into()))?;
    enforce_count("list override", count, limits.max_lists)?;
    let raw_lfos = parse_lfo_headers(&mut cursor, bytes.len(), count)?;
    let mut overrides = Vec::with_capacity(count);
    for (index, raw_lfo) in raw_lfos.into_iter().enumerate() {
        let cp = cursor.read_u32()?;
        let first_cp = if cp == u32::MAX {
            None
        } else if cp < main_length {
            Some(cp)
        } else {
            return Err(DocError::InvalidList(format!(
                "LFO {} first CP {cp} is outside main story {main_length}",
                index + 1
            )));
        };
        let mut levels = Vec::with_capacity(usize::from(raw_lfo.override_count));
        let mut seen = HashSet::new();
        for _ in 0..raw_lfo.override_count {
            let base = cursor.take(LFOLVL_BASE_SIZE)?;
            let start = i32::from_le_bytes(base[0..4].try_into().expect("four-byte slice"));
            let flags = u32::from_le_bytes(base[4..8].try_into().expect("four-byte slice"));
            let level = u8::try_from(flags & 0x0F).unwrap_or(0xFF);
            if level > 8 || !seen.insert(level) {
                return Err(DocError::InvalidList(format!(
                    "LFO {} has invalid or duplicate override level {level}",
                    index + 1
                )));
            }
            let has_start = flags & 0x10 != 0;
            let has_formatting = flags & 0x20 != 0;
            let formatting = has_formatting
                .then(|| parse_level(&mut cursor, level))
                .transpose()?;
            let start_at = if has_start && !has_formatting {
                let value = u32::try_from(start).map_err(|_| {
                    DocError::InvalidList(format!(
                        "LFO {} level {level} has negative start override {start}",
                        index + 1
                    ))
                })?;
                if value > 0x7FFF {
                    return Err(DocError::InvalidList(format!(
                        "LFO {} level {level} start override {value} exceeds 0x7FFF",
                        index + 1
                    )));
                }
                Some(value)
            } else {
                None
            };
            levels.push(ListLevelOverride {
                level,
                start_at,
                formatting,
            });
        }
        overrides.push(ListOverride {
            num_id: u32::try_from(index + 1)
                .map_err(|_| DocError::InvalidList("LFO numId overflow".into()))?,
            lsid: raw_lfo.lsid,
            first_cp,
            levels,
        });
    }
    if cursor.position() != bytes.len() {
        return Err(DocError::InvalidList(format!(
            "PlfLfo has {} trailing bytes",
            bytes.len() - cursor.position()
        )));
    }
    Ok(overrides)
}

fn enforce_count(resource: &'static str, actual: usize, limit: usize) -> crate::Result<()> {
    if actual > limit {
        return Err(DocError::ResourceLimit {
            resource,
            actual: u64::try_from(actual).unwrap_or(u64::MAX),
            limit: u64::try_from(limit).unwrap_or(u64::MAX),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(number_format: u8, text: &[u16], placeholders: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0_u8; LVLF_SIZE];
        bytes[0..4].copy_from_slice(&1_i32.to_le_bytes());
        bytes[4] = number_format;
        bytes[15] = 1;
        bytes[6..6 + placeholders.len()].copy_from_slice(placeholders);
        bytes.extend_from_slice(&u16::try_from(text.len()).unwrap().to_le_bytes());
        for unit in text {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn parses_numeric_and_bullet_levels_and_converts_placeholders() {
        let numeric = level(0, &[0, u16::from(b'.'), 1, u16::from(b')')], &[1, 3]);
        let mut cursor = ByteCursor::new(&numeric, "test LVL");
        let parsed = parse_level(&mut cursor, 1).unwrap();
        assert_eq!(parsed.start_at, 1);
        assert_eq!(parsed.follow, ListFollow::Space);
        assert_eq!(parsed.ooxml_level_text(), "%1.%2)");

        let bullet = level(0x17, &[0xF0B7], &[]);
        let mut cursor = ByteCursor::new(&bullet, "test bullet LVL");
        let parsed = parse_level(&mut cursor, 0).unwrap();
        assert!(parsed.is_bullet());
        assert_eq!(parsed.ooxml_level_text(), "·");
    }

    #[test]
    fn rejects_reserved_formats_bad_placeholders_and_trailing_overrides() {
        let reserved = level(0x08, &[0], &[1]);
        assert!(parse_level(&mut ByteCursor::new(&reserved, "test"), 0).is_err());
        let bad_placeholder = level(0, &[1], &[1]);
        assert!(parse_level(&mut ByteCursor::new(&bad_placeholder, "test"), 0).is_err());
        assert!(parse_overrides(&[0, 0, 0, 0, 1], 10, DocLimits::default()).is_err());
    }
}
