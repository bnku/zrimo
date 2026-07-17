//! Typed, source-proven direct character and paragraph properties.

use crate::{DocError, PropertyGroup, Result, Sprm};

/// A character toggle before style-relative values are resolved through STSH.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleValue {
    /// Force the property off.
    Off,
    /// Force the property on.
    On,
    /// Copy the value from the current style.
    SameAsStyle,
    /// Invert the value from the current style.
    OppositeStyle,
}

/// Explicit superscript/subscript state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignment {
    /// Normal baseline.
    Baseline,
    /// Superscript.
    Superscript,
    /// Subscript.
    Subscript,
}

/// Word line-spacing data retained without converting it to CSS heuristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineSpacing {
    /// Signed `dyaLine` value in twips or 240ths of a line.
    pub value: i16,
    /// Whether `value` is a line multiplier rather than an absolute distance.
    pub multiple: bool,
}

/// One custom paragraph tab stop from a source tab-change operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabStop {
    pub position_twips: i16,
    /// Raw three-bit tab justification code.
    pub alignment: u8,
    /// Raw three-bit leader code.
    pub leader: u8,
}

/// Ordered additions and removals from one tab-change SPRM.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TabChange {
    pub delete_positions_twips: Vec<i16>,
    /// Close ranges for `sprmPChgTabs`; absent for `sprmPChgTabsPapx`.
    pub delete_close_twips: Option<Vec<u16>>,
    pub additions: Vec<TabStop>,
    /// The ignorable extended 0xFF representation was used.
    pub extended: bool,
}

/// Known direct character properties. `None` means not explicitly modified.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CharacterPropertyDelta {
    pub style_index: Option<u16>,
    /// Byte offset in the `Data` stream supplied by `sprmCPicLocation`.
    pub picture_location: Option<u32>,
    /// Whether `sprmCPlain` requested removal of non-style properties.
    pub plain: bool,
    /// Whether a U+0001 picture character names `NilPICFAndBinData` rather
    /// than `PICFAndOfficeArtData` (`sprmCFData`).
    pub binary_data: Option<bool>,
    /// Whether a field separator is a placeholder for an OLE object
    /// (`sprmCFOle2`). The object is never executed by this parser.
    pub ole2: Option<bool>,
    pub bold: Option<ToggleValue>,
    pub italic: Option<ToggleValue>,
    pub strike: Option<ToggleValue>,
    pub double_strike: Option<ToggleValue>,
    pub outline: Option<ToggleValue>,
    pub shadow: Option<ToggleValue>,
    pub small_caps: Option<ToggleValue>,
    pub caps: Option<ToggleValue>,
    pub hidden: Option<ToggleValue>,
    pub imprint: Option<ToggleValue>,
    pub emboss: Option<ToggleValue>,
    pub special: Option<ToggleValue>,
    pub object: Option<ToggleValue>,
    pub bidi: Option<ToggleValue>,
    pub bidi_bold: Option<ToggleValue>,
    pub bidi_italic: Option<ToggleValue>,
    pub complex_scripts: Option<ToggleValue>,
    pub underline: Option<u8>,
    pub character_spacing_twips: Option<i16>,
    pub color_index: Option<u8>,
    pub color_ref: Option<u32>,
    pub underline_color_ref: Option<u32>,
    pub highlight_index: Option<u8>,
    pub font_size_half_points: Option<u16>,
    pub baseline_offset_half_points: Option<i16>,
    pub vertical_alignment: Option<VerticalAlignment>,
    pub font_ascii: Option<u16>,
    pub font_east_asian: Option<u16>,
    pub font_other: Option<u16>,
    pub font_bidi: Option<u16>,
    pub language_ascii: Option<u16>,
    pub language_east_asian: Option<u16>,
    pub language_bidi: Option<u16>,
    /// Character-family opcodes that were framed but are not interpreted yet.
    pub unsupported_opcodes: Vec<u16>,
    /// Well-framed modifiers belonging to another property family.
    pub other_group_opcodes: Vec<u16>,
}

/// Known direct paragraph properties. `None` means not explicitly modified.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParagraphPropertyDelta {
    /// Paragraph style selected by an explicit `sprmPIstd`, if present.
    pub style_index: Option<u16>,
    /// Physical/legacy or logical justification code as stored by Word.
    pub justification: Option<u8>,
    pub keep_together: Option<bool>,
    pub keep_with_next: Option<bool>,
    pub page_break_before: Option<bool>,
    pub list_level: Option<u8>,
    pub list_id: Option<i16>,
    pub indent_right_twips: Option<i16>,
    pub indent_left_twips: Option<i16>,
    pub first_line_indent_twips: Option<i16>,
    pub line_spacing: Option<LineSpacing>,
    /// Tab changes are cumulative and therefore remain in source order.
    pub tab_changes: Vec<TabChange>,
    pub space_before_twips: Option<u16>,
    pub space_after_twips: Option<u16>,
    pub in_table: Option<bool>,
    pub table_terminating_paragraph: Option<bool>,
    pub outline_level: Option<u8>,
    pub bidi: Option<bool>,
    pub table_depth: Option<i32>,
    pub table_depth_delta: Option<i32>,
    pub inner_table_cell: Option<bool>,
    pub inner_table_terminating_paragraph: Option<bool>,
    pub automatic_space_before: Option<bool>,
    pub automatic_space_after: Option<bool>,
    pub contextual_spacing: Option<bool>,
    pub mirror_indents: Option<bool>,
    /// Paragraph-family opcodes that were framed but are not interpreted yet.
    pub unsupported_opcodes: Vec<u16>,
    /// Well-framed modifiers belonging to another property family.
    pub other_group_opcodes: Vec<u16>,
}

/// Applies character SPRMs in source order. Later modifiers win.
///
/// # Errors
///
/// Returns [`DocError::InvalidFormatting`] when a known modifier has a value
/// outside the range defined by MS-DOC.
pub fn apply_character_sprms(sprms: &[Sprm]) -> Result<CharacterPropertyDelta> {
    let mut result = CharacterPropertyDelta::default();
    for sprm in sprms {
        if sprm.group != PropertyGroup::Character {
            result.other_group_opcodes.push(sprm.opcode);
            continue;
        }
        match sprm.opcode {
            0x0806 => result.binary_data = Some(bool8(sprm)?),
            0x080A => result.ole2 = Some(bool8(sprm)?),
            0x2A0C => result.highlight_index = Some(color_index(sprm)?),
            0x6A03 => result.picture_location = Some(dword(sprm)?),
            0x4A30 => result.style_index = Some(word(sprm)?),
            0x2A33 => result.plain = true,
            0x0835 => result.bold = Some(toggle(sprm)?),
            0x0836 => result.italic = Some(toggle(sprm)?),
            0x0837 => result.strike = Some(toggle(sprm)?),
            0x0838 => result.outline = Some(toggle(sprm)?),
            0x0839 => result.shadow = Some(toggle(sprm)?),
            0x083A => result.small_caps = Some(toggle(sprm)?),
            0x083B => result.caps = Some(toggle(sprm)?),
            0x083C => result.hidden = Some(toggle(sprm)?),
            0x2A3E => result.underline = Some(byte(sprm)?),
            0x8840 => result.character_spacing_twips = Some(signed_word(sprm)?),
            0x2A42 | 0x4A60 => result.color_index = Some(color_index(sprm)?),
            0x4A43 => {
                let value = word(sprm)?;
                if !(2..=3276).contains(&value) {
                    return invalid(sprm, format!("font size {value} is outside 2..=3276"));
                }
                result.font_size_half_points = Some(value);
            }
            0x4845 => {
                let value = signed_word(sprm)?;
                if !(-3168..=3168).contains(&value) {
                    return invalid(sprm, format!("baseline offset {value} is out of range"));
                }
                result.baseline_offset_half_points = Some(value);
            }
            0x2A48 => {
                result.vertical_alignment = Some(match byte(sprm)? {
                    0 => VerticalAlignment::Baseline,
                    1 => VerticalAlignment::Superscript,
                    2 => VerticalAlignment::Subscript,
                    value => return invalid(sprm, format!("invalid vertical alignment {value}")),
                });
            }
            0x4A4F => result.font_ascii = Some(word(sprm)?),
            0x4A50 => result.font_east_asian = Some(word(sprm)?),
            0x4A51 => result.font_other = Some(word(sprm)?),
            0x2A53 => result.double_strike = Some(toggle(sprm)?),
            0x0854 => result.imprint = Some(toggle(sprm)?),
            0x0855 => result.special = Some(toggle(sprm)?),
            0x0856 => result.object = Some(toggle(sprm)?),
            0x0858 => result.emboss = Some(toggle(sprm)?),
            0x085A => result.bidi = Some(toggle(sprm)?),
            0x085C => result.bidi_bold = Some(toggle(sprm)?),
            0x085D => result.bidi_italic = Some(toggle(sprm)?),
            0x4A5E => result.font_bidi = Some(word(sprm)?),
            0x485F => result.language_bidi = Some(word(sprm)?),
            0x4A61 => {
                let value = word(sprm)?;
                if !(2..=3276).contains(&value) {
                    return invalid(sprm, format!("bidi font size {value} is outside 2..=3276"));
                }
                result.font_size_half_points = Some(value);
            }
            0x486D | 0x4873 => result.language_ascii = Some(word(sprm)?),
            0x486E | 0x4874 => result.language_east_asian = Some(word(sprm)?),
            0x6870 => result.color_ref = Some(dword(sprm)?),
            0x6877 => result.underline_color_ref = Some(dword(sprm)?),
            0x0882 => result.complex_scripts = Some(toggle(sprm)?),
            _ => result.unsupported_opcodes.push(sprm.opcode),
        }
    }
    Ok(result)
}

/// Applies paragraph SPRMs in source order. Later modifiers win.
///
/// # Errors
///
/// Returns [`DocError::InvalidFormatting`] when a known modifier has a value
/// outside the range defined by MS-DOC.
pub fn apply_paragraph_sprms(sprms: &[Sprm]) -> Result<ParagraphPropertyDelta> {
    let mut result = ParagraphPropertyDelta::default();
    for sprm in sprms {
        if sprm.group != PropertyGroup::Paragraph {
            result.other_group_opcodes.push(sprm.opcode);
            continue;
        }
        match sprm.opcode {
            0x4600 => result.style_index = Some(word(sprm)?),
            0x2403 | 0x2461 => {
                let value = byte(sprm)?;
                if value > 9 {
                    return invalid(sprm, format!("justification {value} exceeds 9"));
                }
                result.justification = Some(value);
            }
            0x2405 => result.keep_together = Some(bool8(sprm)?),
            0x2406 => result.keep_with_next = Some(bool8(sprm)?),
            0x2407 => result.page_break_before = Some(bool8(sprm)?),
            0x260A => result.list_level = Some(byte(sprm)?),
            0x460B => result.list_id = Some(signed_word(sprm)?),
            0x840E | 0x845D => result.indent_right_twips = Some(signed_word(sprm)?),
            0x840F | 0x845E | 0x4610 | 0x465F => {
                result.indent_left_twips = Some(signed_word(sprm)?);
            }
            0x8411 | 0x8460 => result.first_line_indent_twips = Some(signed_word(sprm)?),
            0x6412 => {
                let bytes = exact_operand(sprm, 4)?;
                let value = i16::from_le_bytes([bytes[0], bytes[1]]);
                let multiple = i16::from_le_bytes([bytes[2], bytes[3]]);
                if !matches!(multiple, 0 | 1) {
                    return invalid(sprm, format!("invalid line-spacing mode {multiple}"));
                }
                result.line_spacing = Some(LineSpacing {
                    value,
                    multiple: multiple == 1,
                });
            }
            0xA413 => result.space_before_twips = Some(paragraph_spacing(sprm)?),
            0xA414 => result.space_after_twips = Some(paragraph_spacing(sprm)?),
            0xC60D => result.tab_changes.push(tab_change(sprm, false)?),
            0xC615 => result.tab_changes.push(tab_change(sprm, true)?),
            0x2416 => result.in_table = Some(bool8(sprm)?),
            0x2417 => result.table_terminating_paragraph = Some(bool8(sprm)?),
            0x2640 => {
                let value = byte(sprm)?;
                if value > 9 {
                    return invalid(sprm, format!("outline level {value} exceeds 9"));
                }
                result.outline_level = Some(value);
            }
            0x2441 => result.bidi = Some(bool8(sprm)?),
            0x6649 => {
                let value = signed_dword(sprm)?;
                if value < 0 {
                    return invalid(sprm, format!("negative table depth {value}"));
                }
                result.table_depth = Some(value);
            }
            0x664A => result.table_depth_delta = Some(signed_dword(sprm)?),
            0x244B => result.inner_table_cell = Some(bool8(sprm)?),
            0x244C => result.inner_table_terminating_paragraph = Some(bool8(sprm)?),
            0x245B => result.automatic_space_before = Some(bool8(sprm)?),
            0x245C => result.automatic_space_after = Some(bool8(sprm)?),
            0x246D => result.contextual_spacing = Some(bool8(sprm)?),
            0x2470 => result.mirror_indents = Some(bool8(sprm)?),
            _ => result.unsupported_opcodes.push(sprm.opcode),
        }
    }
    Ok(result)
}

fn exact_operand(sprm: &Sprm, length: usize) -> Result<&[u8]> {
    if sprm.operand.len() != length {
        return invalid(
            sprm,
            format!(
                "operand has {} bytes; expected exactly {length}",
                sprm.operand.len()
            ),
        );
    }
    Ok(&sprm.operand)
}

fn byte(sprm: &Sprm) -> Result<u8> {
    Ok(exact_operand(sprm, 1)?[0])
}

fn word(sprm: &Sprm) -> Result<u16> {
    Ok(u16::from_le_bytes(
        exact_operand(sprm, 2)?.try_into().unwrap(),
    ))
}

fn signed_word(sprm: &Sprm) -> Result<i16> {
    Ok(i16::from_le_bytes(
        exact_operand(sprm, 2)?.try_into().unwrap(),
    ))
}

fn dword(sprm: &Sprm) -> Result<u32> {
    Ok(u32::from_le_bytes(
        exact_operand(sprm, 4)?.try_into().unwrap(),
    ))
}

fn signed_dword(sprm: &Sprm) -> Result<i32> {
    Ok(i32::from_le_bytes(
        exact_operand(sprm, 4)?.try_into().unwrap(),
    ))
}

fn bool8(sprm: &Sprm) -> Result<bool> {
    match byte(sprm)? {
        0 => Ok(false),
        1 => Ok(true),
        value => invalid(sprm, format!("invalid Bool8 value {value}")),
    }
}

fn toggle(sprm: &Sprm) -> Result<ToggleValue> {
    match byte(sprm)? {
        0x00 => Ok(ToggleValue::Off),
        0x01 => Ok(ToggleValue::On),
        0x80 => Ok(ToggleValue::SameAsStyle),
        0x81 => Ok(ToggleValue::OppositeStyle),
        value => invalid(sprm, format!("invalid toggle value 0x{value:02X}")),
    }
}

fn color_index(sprm: &Sprm) -> Result<u8> {
    let value = byte(sprm)?;
    if value > 16 {
        return invalid(sprm, format!("color index {value} exceeds 16"));
    }
    Ok(value)
}

fn paragraph_spacing(sprm: &Sprm) -> Result<u16> {
    let value = word(sprm)?;
    if value > 0x7BC0 {
        return invalid(sprm, format!("paragraph spacing {value} exceeds 0x7BC0"));
    }
    Ok(value)
}

fn tab_change(sprm: &Sprm, has_close_ranges: bool) -> Result<TabChange> {
    let bytes = &sprm.operand;
    let cb = usize::from(*bytes.first().ok_or_else(|| {
        DocError::InvalidFormatting(format!(
            "SPRM 0x{:04X}: missing tab-change length",
            sprm.opcode
        ))
    })?);
    if cb != 0xFF && bytes.len() != cb + 1 {
        return invalid(
            sprm,
            format!(
                "tab-change cb {cb} does not match operand length {}",
                bytes.len()
            ),
        );
    }
    let mut offset = 1_usize;
    let delete_count = usize::from(tab_byte(sprm, bytes, offset, "delete count")?);
    offset += 1;
    if delete_count > 64 {
        return invalid(sprm, format!("tab delete count {delete_count} exceeds 64"));
    }
    let delete_positions_twips = tab_positions(sprm, bytes, &mut offset, delete_count, "delete")?;
    let delete_close_twips = if has_close_ranges {
        let mut close = Vec::with_capacity(delete_count);
        for _ in 0..delete_count {
            close.push(tab_u16(sprm, bytes, &mut offset, "delete-close range")?);
        }
        Some(close)
    } else {
        None
    };
    let add_count = usize::from(tab_byte(sprm, bytes, offset, "add count")?);
    offset += 1;
    if add_count > 64 {
        return invalid(sprm, format!("tab add count {add_count} exceeds 64"));
    }
    let add_positions = tab_positions(sprm, bytes, &mut offset, add_count, "add")?;
    let mut additions = Vec::with_capacity(add_count);
    for position_twips in add_positions {
        let descriptor = tab_byte(sprm, bytes, offset, "tab descriptor")?;
        offset += 1;
        if descriptor & 0xC0 != 0 {
            return invalid(
                sprm,
                format!("tab descriptor 0x{descriptor:02X} sets reserved bits"),
            );
        }
        additions.push(TabStop {
            position_twips,
            alignment: descriptor & 0x07,
            leader: (descriptor >> 3) & 0x07,
        });
    }
    if offset != bytes.len() {
        return invalid(
            sprm,
            format!(
                "{} trailing bytes in tab-change operand",
                bytes.len() - offset
            ),
        );
    }
    Ok(TabChange {
        delete_positions_twips,
        delete_close_twips,
        additions,
        extended: cb == 0xFF,
    })
}

fn tab_positions(
    sprm: &Sprm,
    bytes: &[u8],
    offset: &mut usize,
    count: usize,
    kind: &str,
) -> Result<Vec<i16>> {
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        let position = i16::from_le_bytes(
            tab_pair(sprm, bytes, *offset, &format!("{kind} position"))?
                .try_into()
                .unwrap(),
        );
        *offset += 2;
        if !(-31680..=31680).contains(&position) {
            return invalid(
                sprm,
                format!("{kind} tab position {position} is out of range"),
            );
        }
        result.push(position);
    }
    if result.windows(2).any(|pair| pair[0] >= pair[1]) {
        return invalid(sprm, format!("{kind} tab positions are not increasing"));
    }
    Ok(result)
}

fn tab_byte(sprm: &Sprm, bytes: &[u8], offset: usize, field: &str) -> Result<u8> {
    bytes.get(offset).copied().ok_or_else(|| {
        DocError::InvalidFormatting(format!(
            "SPRM 0x{:04X}: truncated tab-change {field}",
            sprm.opcode
        ))
    })
}

fn tab_pair<'a>(sprm: &Sprm, bytes: &'a [u8], offset: usize, field: &str) -> Result<&'a [u8]> {
    bytes.get(offset..offset + 2).ok_or_else(|| {
        DocError::InvalidFormatting(format!(
            "SPRM 0x{:04X}: truncated tab-change {field}",
            sprm.opcode
        ))
    })
}

fn tab_u16(sprm: &Sprm, bytes: &[u8], offset: &mut usize, field: &str) -> Result<u16> {
    let value = u16::from_le_bytes(tab_pair(sprm, bytes, *offset, field)?.try_into().unwrap());
    *offset += 2;
    Ok(value)
}

fn invalid<T>(sprm: &Sprm, message: impl std::fmt::Display) -> Result<T> {
    Err(DocError::InvalidFormatting(format!(
        "SPRM 0x{:04X}: {message}",
        sprm.opcode
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode_grpprl;

    #[test]
    fn character_properties_are_last_wins_and_keep_style_relative_toggles() {
        let sprms = decode_grpprl(&[
            0x06, 0x08, 1, // picture character is binary data
            0x0A, 0x08, 1, // OLE2 field separator
            0x35, 0x08, 1, // bold on
            0x35, 0x08, 0x81, // then opposite of style
            0x43, 0x4A, 24, 0, // 12pt
            0x48, 0x2A, 2, // subscript
        ])
        .unwrap();
        let properties = apply_character_sprms(&sprms).unwrap();
        assert_eq!(properties.binary_data, Some(true));
        assert_eq!(properties.ole2, Some(true));
        assert_eq!(properties.bold, Some(ToggleValue::OppositeStyle));
        assert_eq!(properties.font_size_half_points, Some(24));
        assert_eq!(
            properties.vertical_alignment,
            Some(VerticalAlignment::Subscript)
        );
    }

    #[test]
    fn rejects_non_bool_binary_data_and_ole_flags() {
        for opcode in [[0x06, 0x08, 2], [0x0A, 0x08, 0x80]] {
            let sprms = decode_grpprl(&opcode).unwrap();
            assert!(matches!(
                apply_character_sprms(&sprms),
                Err(DocError::InvalidFormatting(_))
            ));
        }
    }

    #[test]
    fn paragraph_properties_are_last_wins_and_retain_table_sprms() {
        let sprms = decode_grpprl(&[
            0x05, 0x24, 1, // keep
            0x05, 0x24, 0, // then do not keep
            0x13, 0xA4, 120, 0, // space before
            0x49, 0x66, 2, 0, 0, 0, // table depth
            0x08, 0xD6, 4, 0, 2, 10, 0, // table-family SPRM
        ])
        .unwrap();
        let properties = apply_paragraph_sprms(&sprms).unwrap();
        assert_eq!(properties.keep_together, Some(false));
        assert_eq!(properties.space_before_twips, Some(120));
        assert_eq!(properties.table_depth, Some(2));
        assert_eq!(properties.other_group_opcodes, [0xD608]);
    }

    #[test]
    fn rejects_invalid_known_values_without_rejecting_unknown_opcodes() {
        let invalid_toggle = decode_grpprl(&[0x35, 0x08, 2]).unwrap();
        assert!(matches!(
            apply_character_sprms(&invalid_toggle),
            Err(DocError::InvalidFormatting(_))
        ));

        let unknown = decode_grpprl(&[0x00, 0x2B, 7]).unwrap();
        let properties = apply_character_sprms(&unknown).unwrap();
        assert_eq!(properties.unsupported_opcodes, [0x2B00]);
    }

    #[test]
    fn decodes_normal_and_extended_tab_changes() {
        let papx = decode_grpprl(&[
            0x0D, 0xC6, 7, // cb
            1, 10, 0, // delete 10
            1, 20, 0,    // add 20
            0x11, // centered, dotted leader
        ])
        .unwrap();
        let properties = apply_paragraph_sprms(&papx).unwrap();
        assert_eq!(properties.tab_changes[0].delete_positions_twips, [10]);
        assert_eq!(properties.tab_changes[0].additions[0].alignment, 1);
        assert_eq!(properties.tab_changes[0].additions[0].leader, 2);

        let extended = decode_grpprl(&[
            0x15, 0xC6, 0xFF, // specialized marker
            1, 10, 0, 25, 0, // delete position and close range
            1, 20, 0, 0, // add position and descriptor
        ])
        .unwrap();
        let properties = apply_paragraph_sprms(&extended).unwrap();
        assert!(properties.tab_changes[0].extended);
        assert_eq!(properties.tab_changes[0].delete_close_twips, Some(vec![25]));
    }
}
