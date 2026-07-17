//! Source-backed Word field reconstruction from per-story `PlcfFld` PLCs.

use crate::{
    DocError, DocLimits, Result, Story, StoryKind, WordBinaryDocument,
    binary::{ByteCursor, checked_slice},
};

/// A balanced field range. CPs are global source positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub story: StoryKind,
    pub cp_begin: u32,
    pub cp_separator: Option<u32>,
    pub cp_end: u32,
    /// Last parsed `flt` code from the begin record.
    pub field_type: u8,
    /// Zero for a top-level field, positive for nested fields.
    pub depth: u32,
}

impl Field {
    #[must_use]
    pub const fn instruction_start(self) -> u32 {
        self.cp_begin + 1
    }

    #[must_use]
    pub fn instruction_end(self) -> u32 {
        self.cp_separator.unwrap_or(self.cp_end)
    }

    #[must_use]
    pub fn result_range(self) -> Option<(u32, u32)> {
        self.cp_separator
            .map(|separator| (separator + 1, self.cp_end))
    }
}

/// Validated fields from all document stories.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldCollection {
    fields: Vec<Field>,
}

impl FieldCollection {
    pub(crate) fn parse(document: &WordBinaryDocument, limits: DocLimits) -> Result<Self> {
        let locations = [
            (StoryKind::Main, document.fib().locations.main_fields()),
            (
                StoryKind::Footnotes,
                document.fib().locations.footnote_fields(),
            ),
            (StoryKind::Headers, document.fib().locations.header_fields()),
            (
                StoryKind::Comments,
                document.fib().locations.comment_fields(),
            ),
            (
                StoryKind::Endnotes,
                document.fib().locations.endnote_fields(),
            ),
            (
                StoryKind::Textboxes,
                document.fib().locations.textbox_fields(),
            ),
            (
                StoryKind::HeaderTextboxes,
                document.fib().locations.header_textbox_fields(),
            ),
        ];
        let mut fields = Vec::new();
        let mut character_count = 0_usize;
        for (kind, location) in locations {
            let Some(location) = location.filter(|value| !value.is_empty()) else {
                continue;
            };
            let story = document.story(kind).ok_or_else(|| {
                DocError::InvalidField(format!("{kind:?} field PLC has no corresponding story"))
            })?;
            let bytes = checked_slice(
                document.table_stream(),
                location.offset,
                location.length,
                field_structure(kind),
            )?;
            let count = field_character_count(kind, bytes.len())?;
            character_count = character_count
                .checked_add(count)
                .ok_or_else(|| DocError::InvalidField("field-character count overflow".into()))?;
            if character_count > limits.max_field_characters {
                return Err(DocError::ResourceLimit {
                    resource: "field character",
                    actual: u64::try_from(character_count).unwrap_or(u64::MAX),
                    limit: u64::try_from(limits.max_field_characters).unwrap_or(u64::MAX),
                });
            }
            parse_story(document, story, bytes, count, &mut fields)?;
        }
        fields.sort_by_key(|field| field.cp_begin);
        Ok(Self { fields })
    }

    #[must_use]
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    pub(crate) fn top_level_in(
        &self,
        cp_start: u32,
        cp_end: u32,
    ) -> impl Iterator<Item = Field> + '_ {
        self.fields.iter().copied().filter(move |field| {
            field.depth == 0 && field.cp_begin >= cp_start && field.cp_end < cp_end
        })
    }
}

fn field_character_count(kind: StoryKind, length: usize) -> Result<usize> {
    if length < 4 || !(length - 4).is_multiple_of(6) {
        return Err(DocError::InvalidField(format!(
            "{} length {length} is not 6*n+4",
            field_structure(kind)
        )));
    }
    Ok((length - 4) / 6)
}

fn parse_story(
    document: &WordBinaryDocument,
    story: &Story,
    bytes: &[u8],
    count: usize,
    output: &mut Vec<Field>,
) -> Result<()> {
    let mut cursor = ByteCursor::new(bytes, field_structure(story.kind));
    let mut positions = Vec::with_capacity(count + 1);
    for _ in 0..=count {
        positions.push(cursor.read_u32()?);
    }
    if positions.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DocError::InvalidField(format!(
            "{} CPs are not strictly increasing: {positions:?}",
            field_structure(story.kind)
        )));
    }
    let story_length = story.cp_end - story.cp_start;
    if positions[..count].iter().any(|cp| *cp >= story_length) {
        return Err(DocError::InvalidField(format!(
            "{} field CP is outside story length {story_length}",
            field_structure(story.kind)
        )));
    }
    let mut records = Vec::with_capacity(count);
    for local_cp in positions.into_iter().take(count) {
        let first = cursor.read_u16()?;
        let kind = u8::try_from(first & 0x001F).unwrap_or_default();
        let properties = u8::try_from(first >> 8).unwrap_or_default();
        if !matches!(kind, 0x13..=0x15) {
            return Err(DocError::InvalidField(format!(
                "{} has invalid field character 0x{kind:02X}",
                field_structure(story.kind)
            )));
        }
        let cp = story.cp_start.checked_add(local_cp).ok_or_else(|| {
            DocError::InvalidField(format!(
                "{} global CP overflow",
                field_structure(story.kind)
            ))
        })?;
        let source = document.decode_range(cp, cp + 1)?;
        if source.utf16 != [u16::from(kind)] {
            return Err(DocError::InvalidField(format!(
                "{} record 0x{kind:02X} at CP {cp} disagrees with source {:?}",
                field_structure(story.kind),
                source.utf16
            )));
        }
        records.push((cp, kind, properties));
    }
    balance_records(story.kind, &records, output)
}

#[derive(Debug, Clone, Copy)]
struct OpenField {
    cp_begin: u32,
    cp_separator: Option<u32>,
    field_type: u8,
    depth: u32,
}

fn balance_records(
    story: StoryKind,
    records: &[(u32, u8, u8)],
    output: &mut Vec<Field>,
) -> Result<()> {
    let mut stack = Vec::<OpenField>::new();
    for &(cp, kind, properties) in records {
        match kind {
            0x13 => {
                let depth = u32::try_from(stack.len()).map_err(|_| {
                    DocError::InvalidField(format!(
                        "{} nesting depth overflow",
                        field_structure(story)
                    ))
                })?;
                stack.push(OpenField {
                    cp_begin: cp,
                    cp_separator: None,
                    field_type: properties,
                    depth,
                });
            }
            0x14 => {
                let current = stack.last_mut().ok_or_else(|| {
                    DocError::InvalidField(format!(
                        "{} separator at CP {cp} has no open field",
                        field_structure(story)
                    ))
                })?;
                if current.cp_separator.replace(cp).is_some() {
                    return Err(DocError::InvalidField(format!(
                        "{} field at CP {} has two separators",
                        field_structure(story),
                        current.cp_begin
                    )));
                }
            }
            0x15 => {
                let current = stack.pop().ok_or_else(|| {
                    DocError::InvalidField(format!(
                        "{} end at CP {cp} has no open field",
                        field_structure(story)
                    ))
                })?;
                output.push(Field {
                    story,
                    cp_begin: current.cp_begin,
                    cp_separator: current.cp_separator,
                    cp_end: cp,
                    field_type: current.field_type,
                    depth: current.depth,
                });
            }
            _ => unreachable!(),
        }
    }
    if !stack.is_empty() {
        return Err(DocError::InvalidField(format!(
            "{} ends with {} unclosed fields",
            field_structure(story),
            stack.len()
        )));
    }
    Ok(())
}

const fn field_structure(kind: StoryKind) -> &'static str {
    match kind {
        StoryKind::Main => "PlcfFldMom",
        StoryKind::Footnotes => "PlcfFldFtn",
        StoryKind::Headers => "PlcfFldHdr",
        StoryKind::Comments => "PlcfFldAtn",
        StoryKind::Endnotes => "PlcfFldEdn",
        StoryKind::Textboxes => "PlcfFldTxbx",
        StoryKind::HeaderTextboxes => "PlcfFldHdrTxbx",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balances_nested_fields() {
        let records = [
            (1, 0x13, 33),
            (3, 0x13, 26),
            (5, 0x15, 0),
            (7, 0x14, 0),
            (9, 0x15, 0),
        ];
        let mut fields = Vec::new();
        balance_records(StoryKind::Main, &records, &mut fields).unwrap();
        fields.sort_by_key(|field| field.cp_begin);
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].cp_separator, Some(7));
        assert_eq!(fields[0].depth, 0);
        assert_eq!(fields[1].depth, 1);
    }

    #[test]
    fn rejects_bad_plc_size_and_unbalanced_sequence() {
        assert!(field_character_count(StoryKind::Headers, 9).is_err());
        assert!(balance_records(StoryKind::Headers, &[(2, 0x14, 0)], &mut Vec::new()).is_err());
    }
}
