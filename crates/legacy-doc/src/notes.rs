//! Source-backed footnote and endnote reconstruction.

use crate::{
    DocError, DocLimits, FcLcb, Result, Story, StoryKind, WordBinaryDocument,
    binary::{ByteCursor, checked_slice},
};

/// Kind of note subdocument and reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    Footnote,
    Endnote,
}

/// One reference mark in the main document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteReference {
    pub kind: NoteKind,
    /// Global source CP of the reference character.
    pub cp: u32,
    /// OOXML-compatible positive identifier, scoped to the note kind.
    pub note_id: u32,
    pub automatic: bool,
}

/// One note body range in its source story.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceNote {
    pub kind: NoteKind,
    pub note_id: u32,
    pub reference_cp: u32,
    pub automatic: bool,
    /// Global source CP of the first body character.
    pub cp_start: u32,
    /// Exclusive global source CP, including the required final paragraph mark.
    pub cp_end: u32,
}

/// Validated note references and their one-to-one body ranges.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NoteCollection {
    notes: Vec<SourceNote>,
    references: Vec<NoteReference>,
}

impl NoteCollection {
    /// Parse footnote and endnote PLCs referenced by the FIB.
    pub(crate) fn parse(document: &WordBinaryDocument, limits: DocLimits) -> Result<Self> {
        let mut result = Self::default();
        parse_kind(
            document,
            NoteKind::Footnote,
            StoryKind::Footnotes,
            document.fib().locations.footnote_references(),
            document.fib().locations.footnote_text(),
            limits,
            &mut result,
        )?;
        parse_kind(
            document,
            NoteKind::Endnote,
            StoryKind::Endnotes,
            document.fib().locations.endnote_references(),
            document.fib().locations.endnote_text(),
            limits,
            &mut result,
        )?;
        result.references.sort_by_key(|reference| reference.cp);
        Ok(result)
    }

    #[must_use]
    pub fn notes(&self) -> &[SourceNote] {
        &self.notes
    }

    #[must_use]
    pub fn references(&self) -> &[NoteReference] {
        &self.references
    }

    #[must_use]
    pub fn reference_at(&self, cp: u32) -> Option<NoteReference> {
        self.references
            .binary_search_by_key(&cp, |reference| reference.cp)
            .ok()
            .map(|index| self.references[index])
    }
}

#[allow(clippy::too_many_arguments)]
fn parse_kind(
    document: &WordBinaryDocument,
    kind: NoteKind,
    story_kind: StoryKind,
    reference_location: Option<FcLcb>,
    text_location: Option<FcLcb>,
    limits: DocLimits,
    result: &mut NoteCollection,
) -> Result<()> {
    let story = document.story(story_kind);
    let reference_location = reference_location.filter(|location| !location.is_empty());
    let text_location = text_location.filter(|location| !location.is_empty());
    match (story, reference_location, text_location) {
        (None, None, None) => Ok(()),
        (
            Some(Story {
                cp_start, cp_end, ..
            }),
            None,
            None,
        ) if cp_start == cp_end => Ok(()),
        (Some(story), Some(reference_location), Some(text_location)) => {
            let reference_bytes = checked_slice(
                document.table_stream(),
                reference_location.offset,
                reference_location.length,
                reference_structure(kind),
            )?;
            let text_bytes = checked_slice(
                document.table_stream(),
                text_location.offset,
                text_location.length,
                text_structure(kind),
            )?;
            append_kind(
                document,
                kind,
                story,
                reference_bytes,
                text_bytes,
                limits,
                result,
            )
        }
        _ => Err(DocError::InvalidNote(format!(
            "{} story and both PLCs must either all be present or all be absent",
            kind_name(kind)
        ))),
    }
}

fn append_kind(
    document: &WordBinaryDocument,
    kind: NoteKind,
    story: &Story,
    reference_bytes: &[u8],
    text_bytes: &[u8],
    limits: DocLimits,
    result: &mut NoteCollection,
) -> Result<()> {
    let references = parse_references(kind, reference_bytes, document.fib().stories.main)?;
    let projected_total = result
        .notes
        .len()
        .checked_add(references.len())
        .ok_or_else(|| DocError::InvalidNote("note count overflow".into()))?;
    if projected_total > limits.max_notes {
        return Err(DocError::ResourceLimit {
            resource: "footnote/endnote",
            actual: u64::try_from(projected_total).unwrap_or(u64::MAX),
            limit: u64::try_from(limits.max_notes).unwrap_or(u64::MAX),
        });
    }
    let boundaries = parse_boundaries(kind, text_bytes, references.len(), story)?;
    for (index, ((reference_cp, automatic), range)) in references
        .into_iter()
        .zip(boundaries.windows(2))
        .enumerate()
    {
        let note_id = u32::try_from(index + 1)
            .map_err(|_| DocError::InvalidNote("note identifier overflow".into()))?;
        let cp_start = story.cp_start.checked_add(range[0]).ok_or_else(|| {
            DocError::InvalidNote(format!("{} body start CP overflow", kind_name(kind)))
        })?;
        let cp_end = story.cp_start.checked_add(range[1]).ok_or_else(|| {
            DocError::InvalidNote(format!("{} body end CP overflow", kind_name(kind)))
        })?;
        validate_body_end(document, kind, cp_start, cp_end)?;
        result.references.push(NoteReference {
            kind,
            cp: reference_cp,
            note_id,
            automatic,
        });
        result.notes.push(SourceNote {
            kind,
            note_id,
            reference_cp,
            automatic,
            cp_start,
            cp_end,
        });
    }
    Ok(())
}

fn parse_references(kind: NoteKind, bytes: &[u8], main_length: u32) -> Result<Vec<(u32, bool)>> {
    if bytes.len() < 4 || !(bytes.len() - 4).is_multiple_of(6) {
        return Err(DocError::InvalidNote(format!(
            "{} reference PLC length {} is not 6*n+4",
            kind_name(kind),
            bytes.len()
        )));
    }
    let count = (bytes.len() - 4) / 6;
    let mut cursor = ByteCursor::new(bytes, reference_structure(kind));
    let mut positions = Vec::with_capacity(count + 1);
    for _ in 0..=count {
        positions.push(cursor.read_u32()?);
    }
    ensure_strictly_increasing(kind, "reference", &positions)?;
    if let Some(cp) = positions[..count].iter().find(|cp| **cp >= main_length) {
        return Err(DocError::InvalidNote(format!(
            "{} reference CP {cp} is outside main story length {main_length}",
            kind_name(kind)
        )));
    }
    let mut result = Vec::with_capacity(count);
    for cp in positions.into_iter().take(count) {
        result.push((cp, cursor.read_u16()? != 0));
    }
    Ok(result)
}

fn parse_boundaries(kind: NoteKind, bytes: &[u8], count: usize, story: &Story) -> Result<Vec<u32>> {
    let expected_count = count.checked_add(2).ok_or_else(|| {
        DocError::InvalidNote(format!("{} boundary count overflow", kind_name(kind)))
    })?;
    let expected_bytes = expected_count.checked_mul(4).ok_or_else(|| {
        DocError::InvalidNote(format!("{} boundary byte count overflow", kind_name(kind)))
    })?;
    if bytes.len() != expected_bytes {
        return Err(DocError::InvalidNote(format!(
            "{} text PLC has {} bytes; expected {expected_bytes} for {count} notes",
            kind_name(kind),
            bytes.len()
        )));
    }
    let mut cursor = ByteCursor::new(bytes, text_structure(kind));
    let mut positions = Vec::with_capacity(expected_count);
    for _ in 0..expected_count {
        positions.push(cursor.read_u32()?);
    }
    let used = &positions[..=count];
    ensure_strictly_increasing(kind, "body", used)?;
    let story_length = story.cp_end - story.cp_start;
    let expected_end = story_length.checked_sub(1).ok_or_else(|| {
        DocError::InvalidNote(format!("{} story has no guard character", kind_name(kind)))
    })?;
    if used.last().copied() != Some(expected_end) {
        return Err(DocError::InvalidNote(format!(
            "{} final body boundary {:?} does not equal story length minus guard {expected_end}",
            kind_name(kind),
            used.last()
        )));
    }
    Ok(used.to_vec())
}

fn validate_body_end(
    document: &WordBinaryDocument,
    kind: NoteKind,
    cp_start: u32,
    cp_end: u32,
) -> Result<()> {
    if cp_start >= cp_end {
        return Err(DocError::InvalidNote(format!(
            "{} body [{cp_start}, {cp_end}) is empty",
            kind_name(kind)
        )));
    }
    let terminal = document.decode_range(cp_end - 1, cp_end)?;
    if terminal.utf16 != [0x000D] {
        return Err(DocError::InvalidNote(format!(
            "{} body [{cp_start}, {cp_end}) does not end in a paragraph mark",
            kind_name(kind)
        )));
    }
    Ok(())
}

fn ensure_strictly_increasing(kind: NoteKind, label: &str, positions: &[u32]) -> Result<()> {
    if positions.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DocError::InvalidNote(format!(
            "{} {label} CPs are not strictly increasing: {positions:?}",
            kind_name(kind)
        )));
    }
    Ok(())
}

const fn kind_name(kind: NoteKind) -> &'static str {
    match kind {
        NoteKind::Footnote => "footnote",
        NoteKind::Endnote => "endnote",
    }
}

const fn reference_structure(kind: NoteKind) -> &'static str {
    match kind {
        NoteKind::Footnote => "PlcffndRef",
        NoteKind::Endnote => "PlcfendRef",
    }
}

const fn text_structure(kind: NoteKind) -> &'static str {
    match kind {
        NoteKind::Footnote => "PlcffndTxt",
        NoteKind::Endnote => "PlcfendTxt",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reference_and_body_plcs() {
        let mut references = Vec::new();
        for cp in [4_u32, 9, 12] {
            references.extend_from_slice(&cp.to_le_bytes());
        }
        references.extend_from_slice(&1_u16.to_le_bytes());
        references.extend_from_slice(&0_u16.to_le_bytes());
        assert_eq!(
            parse_references(NoteKind::Footnote, &references, 20).unwrap(),
            vec![(4, true), (9, false)]
        );

        let story = Story {
            kind: StoryKind::Footnotes,
            cp_start: 20,
            cp_end: 31,
            content: crate::DecodedText {
                cp_start: 20,
                cp_end: 31,
                text: String::new(),
                utf16: Vec::new(),
            },
        };
        let mut boundaries = Vec::new();
        for cp in [0_u32, 5, 10, 1234] {
            boundaries.extend_from_slice(&cp.to_le_bytes());
        }
        assert_eq!(
            parse_boundaries(NoteKind::Footnote, &boundaries, 2, &story).unwrap(),
            vec![0, 5, 10]
        );
    }

    #[test]
    fn rejects_mismatched_and_duplicate_plcs() {
        assert!(parse_references(NoteKind::Endnote, &[0; 9], 10).is_err());
        let mut references = Vec::new();
        for cp in [2_u32, 2] {
            references.extend_from_slice(&cp.to_le_bytes());
        }
        references.extend_from_slice(&1_u16.to_le_bytes());
        assert!(parse_references(NoteKind::Endnote, &references, 10).is_err());
    }
}
