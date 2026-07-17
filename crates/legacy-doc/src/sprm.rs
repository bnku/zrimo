//! Version-independent Word 97+ SPRM framing.

use crate::{DocError, Result};

const SPRM_T_DEF_TABLE: u16 = 0xD608;
const SPRM_P_CHG_TABS: u16 = 0xC615;

/// Document property family modified by an SPRM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyGroup {
    /// Paragraph property.
    Paragraph,
    /// Character property.
    Character,
    /// Picture property.
    Picture,
    /// Section property.
    Section,
    /// Table property.
    Table,
}

/// One exactly framed property modifier and its raw operand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprm {
    /// Complete 16-bit opcode.
    pub opcode: u16,
    /// Property identifier inside its group.
    pub property_id: u16,
    /// Special-operand flag from the opcode.
    pub special: bool,
    /// Property family.
    pub group: PropertyGroup,
    /// Operand bytes, including a variable-length prefix where applicable.
    pub operand: Vec<u8>,
}

/// Decodes a complete `grpprl` into exactly bounded SPRM records.
///
/// This function frames operands but deliberately does not invent semantics for
/// unknown opcodes. Callers can retain or warn about an unknown [`Sprm`] while
/// continuing with the following record.
///
/// # Errors
///
/// Returns [`DocError::InvalidFormatting`] for truncated opcodes/operands,
/// invalid property groups, arithmetic overflow, or malformed specialized
/// variable-length operands.
pub fn decode_grpprl(data: &[u8]) -> Result<Vec<Sprm>> {
    let mut offset = 0_usize;
    let mut result = Vec::new();
    while offset < data.len() {
        let opcode_end = offset
            .checked_add(2)
            .ok_or_else(|| DocError::InvalidFormatting("SPRM opcode offset overflow".into()))?;
        let opcode_bytes = data.get(offset..opcode_end).ok_or_else(|| {
            DocError::InvalidFormatting(format!("truncated SPRM opcode at byte {offset}"))
        })?;
        let opcode = u16::from_le_bytes([opcode_bytes[0], opcode_bytes[1]]);
        offset = opcode_end;

        let group_bits = ((opcode >> 10) & 0x7) as u8;
        let group = match group_bits {
            1 => PropertyGroup::Paragraph,
            2 => PropertyGroup::Character,
            3 => PropertyGroup::Picture,
            4 => PropertyGroup::Section,
            5 => PropertyGroup::Table,
            _ => {
                return Err(DocError::InvalidFormatting(format!(
                    "SPRM 0x{opcode:04X} has invalid property group {group_bits}"
                )));
            }
        };
        let spra = u8::try_from((opcode >> 13) & 0x7).map_err(|_| {
            DocError::InvalidFormatting(format!("SPRM 0x{opcode:04X} has invalid spra bits"))
        })?;
        let operand_length = operand_length(data, offset, opcode, spra)?;
        let operand_end = offset
            .checked_add(operand_length)
            .ok_or_else(|| DocError::InvalidFormatting("SPRM operand offset overflow".into()))?;
        let operand = data.get(offset..operand_end).ok_or_else(|| {
            DocError::InvalidFormatting(format!(
                "SPRM 0x{opcode:04X} operand [{offset}, {operand_end}) exceeds grpprl length {}",
                data.len()
            ))
        })?;
        result.push(Sprm {
            opcode,
            property_id: opcode & 0x01FF,
            special: opcode & 0x0200 != 0,
            group,
            operand: operand.to_vec(),
        });
        offset = operand_end;
    }
    Ok(result)
}

fn operand_length(data: &[u8], offset: usize, opcode: u16, spra: u8) -> Result<usize> {
    match spra {
        0 | 1 => Ok(1),
        2 | 4 | 5 => Ok(2),
        3 => Ok(4),
        7 => Ok(3),
        6 if opcode == SPRM_T_DEF_TABLE => {
            let prefix = data.get(offset..offset + 2).ok_or_else(|| {
                DocError::InvalidFormatting("truncated sprmTDefTable length".into())
            })?;
            let cb = usize::from(u16::from_le_bytes([prefix[0], prefix[1]]));
            if cb == 0 {
                return Err(DocError::InvalidFormatting(
                    "sprmTDefTable length is zero".into(),
                ));
            }
            cb.checked_add(1)
                .ok_or_else(|| DocError::InvalidFormatting("sprmTDefTable length overflow".into()))
        }
        6 => {
            let cb = usize::from(*data.get(offset).ok_or_else(|| {
                DocError::InvalidFormatting(format!(
                    "truncated variable SPRM 0x{opcode:04X} length"
                ))
            })?);
            if opcode == SPRM_P_CHG_TABS && cb == 0xFF {
                return extended_p_chg_tabs_length(data, offset);
            }
            cb.checked_add(1).ok_or_else(|| {
                DocError::InvalidFormatting(format!("variable SPRM 0x{opcode:04X} length overflow"))
            })
        }
        _ => Err(DocError::InvalidFormatting(format!(
            "SPRM 0x{opcode:04X} has invalid spra {spra}"
        ))),
    }
}

fn extended_p_chg_tabs_length(data: &[u8], offset: usize) -> Result<usize> {
    // PChgTabsDelClose = cTabs + rgdxaDel[cTabs] + rgdxaClose[cTabs].
    // PChgTabsAdd = cTabs + rgdxaAdd[cTabs] + rgtbdAdd[cTabs].
    let delete_count = usize::from(*data.get(offset + 1).ok_or_else(|| {
        DocError::InvalidFormatting("truncated extended sprmPChgTabs delete count".into())
    })?);
    if delete_count > 64 {
        return Err(DocError::InvalidFormatting(format!(
            "extended sprmPChgTabs delete count {delete_count} exceeds 64"
        )));
    }
    let add_count_offset = offset
        .checked_add(2)
        .and_then(|value| value.checked_add(delete_count.checked_mul(4)?))
        .ok_or_else(|| {
            DocError::InvalidFormatting("extended sprmPChgTabs offset overflow".into())
        })?;
    let add_count = usize::from(*data.get(add_count_offset).ok_or_else(|| {
        DocError::InvalidFormatting("truncated extended sprmPChgTabs add count".into())
    })?);
    if add_count > 64 {
        return Err(DocError::InvalidFormatting(format!(
            "extended sprmPChgTabs add count {add_count} exceeds 64"
        )));
    }
    // Includes cb itself and both count bytes.
    3_usize
        .checked_add(delete_count.checked_mul(4).ok_or_else(|| {
            DocError::InvalidFormatting("extended sprmPChgTabs length overflow".into())
        })?)
        .and_then(|value| value.checked_add(add_count.checked_mul(3)?))
        .ok_or_else(|| DocError::InvalidFormatting("extended sprmPChgTabs length overflow".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_fixed_and_variable_operands_without_losing_unknown_sprms() {
        let data = [
            0x35, 0x24, 1, // paragraph, one-byte operand
            0x36, 0x6C, 0x34, 0x12, 0x56, 0x78, // picture, four-byte operand
            0x00, 0xC6, 3, 9, 8, 7, // unknown paragraph variable operand
        ];
        let decoded = decode_grpprl(&data).unwrap();
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].group, PropertyGroup::Paragraph);
        assert_eq!(decoded[0].operand, [1]);
        assert_eq!(decoded[1].group, PropertyGroup::Picture);
        assert_eq!(decoded[1].operand, [0x34, 0x12, 0x56, 0x78]);
        assert_eq!(decoded[2].operand, [3, 9, 8, 7]);
    }

    #[test]
    fn handles_tdef_table_two_byte_length_prefix() {
        let data = [0x08, 0xD6, 4, 0, 2, 10, 0];
        let decoded = decode_grpprl(&data).unwrap();
        assert_eq!(decoded[0].group, PropertyGroup::Table);
        assert_eq!(decoded[0].operand, [4, 0, 2, 10, 0]);
    }

    #[test]
    fn rejects_truncated_and_malformed_special_operands() {
        assert!(matches!(
            decode_grpprl(&[0x35]),
            Err(DocError::InvalidFormatting(_))
        ));
        assert!(matches!(
            decode_grpprl(&[0x15, 0xC6, 0xFF]),
            Err(DocError::InvalidFormatting(_))
        ));
    }

    #[test]
    fn frames_extended_p_chg_tabs_and_continues_with_the_next_sprm() {
        let data = [
            0x15, 0xC6, 0xFF, // sprmPChgTabs, specialized length marker
            1,    // one delete-close record
            10, 0, // rgdxaDel
            25, 0, // rgdxaClose
            1, // one add record
            20, 0, // rgdxaAdd
            0, // rgtbdAdd
            0x05, 0x24, 1, // sprmPFKeep
        ];
        let decoded = decode_grpprl(&data).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].operand, &data[2..12]);
        assert_eq!(decoded[1].opcode, 0x2405);
    }
}
