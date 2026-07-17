//! Source-backed Word comment references, anchors, and body ranges.

use std::collections::{HashMap, HashSet};

use crate::{
    DocError, DocLimits, FcLcb, Story, StoryKind, WordBinaryDocument,
    binary::{ByteCursor, checked_slice},
};

const MAX_COMMENT_AUTHORS: usize = 0x7FFF;
const MAX_AUTHOR_NAME_CHARS: usize = 55;

/// Metadata and source ranges for one Word comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceComment {
    /// Stable positive identifier used by a future OOXML comment projection.
    pub comment_id: u32,
    /// Global source CP of the comment reference character in the main story.
    pub reference_cp: u32,
    /// Global source CP of the required leading comment marker (`0x0005`).
    pub cp_start: u32,
    /// Exclusive global source CP, including the terminal paragraph mark.
    pub cp_end: u32,
    /// Author initials retained directly from `ATRDPre10`.
    pub initials: String,
    /// Index into the source comment-owner string table.
    pub author_index: u16,
    /// Full author name resolved through `GrpXstAtnOwners`.
    pub author: String,
    /// Annotation bookmark tag, or `-1` for a zero-length anchor.
    pub bookmark_tag: i32,
    /// Inclusive source CP where the annotated main-story range begins.
    pub anchor_cp_start: Option<u32>,
    /// Exclusive source CP where the annotated main-story range ends.
    pub anchor_cp_end: Option<u32>,
}

impl SourceComment {
    /// First CP containing visible comment body text (after the required marker).
    #[must_use]
    pub const fn content_cp_start(&self) -> u32 {
        self.cp_start + 1
    }

    /// Private-use marker emitted into the intermediate IR and replaced by
    /// the legacy Office bridge with an OOXML `commentReference` element.
    #[doc(hidden)]
    #[must_use]
    pub fn projection_marker(&self) -> String {
        format!(
            "\u{F0000}DOCS_VIEWER_WASM_COMMENT_{:08X}\u{F0001}",
            self.comment_id
        )
    }

    /// Private marker replaced with `w:commentRangeStart` by the bridge.
    #[doc(hidden)]
    #[must_use]
    pub fn range_start_projection_marker(&self) -> String {
        format!(
            "\u{F0000}DOCS_VIEWER_WASM_COMMENT_RANGE_START_{:08X}\u{F0001}",
            self.comment_id
        )
    }

    /// Private marker replaced with `w:commentRangeEnd` by the bridge.
    #[doc(hidden)]
    #[must_use]
    pub fn range_end_projection_marker(&self) -> String {
        format!(
            "\u{F0000}DOCS_VIEWER_WASM_COMMENT_RANGE_END_{:08X}\u{F0001}",
            self.comment_id
        )
    }
}

/// Validated one-to-one mapping between main-story references and comment bodies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommentCollection {
    authors: Vec<String>,
    comments: Vec<SourceComment>,
    range_starts: Vec<(u32, usize)>,
    range_ends: Vec<(u32, usize)>,
}

impl CommentCollection {
    pub(crate) fn parse(document: &WordBinaryDocument, limits: DocLimits) -> crate::Result<Self> {
        let story = document.story(StoryKind::Comments);
        let reference_location = document
            .fib()
            .locations
            .comment_references()
            .filter(|location| !location.is_empty());
        let text_location = document
            .fib()
            .locations
            .comment_text()
            .filter(|location| !location.is_empty());
        let author_location = document
            .fib()
            .locations
            .comment_authors()
            .filter(|location| !location.is_empty());

        match (story, reference_location, text_location, author_location) {
            (None, None, None, None) => Ok(Self::default()),
            (
                Some(Story {
                    cp_start, cp_end, ..
                }),
                None,
                None,
                None,
            ) if cp_start == cp_end => Ok(Self::default()),
            (
                Some(story),
                Some(reference_location),
                Some(text_location),
                Some(author_location),
            ) => {
                let references = checked_slice(
                    document.table_stream(),
                    reference_location.offset,
                    reference_location.length,
                    "PlcfandRef",
                )?;
                let boundaries = checked_slice(
                    document.table_stream(),
                    text_location.offset,
                    text_location.length,
                    "PlcfandTxt",
                )?;
                let authors = checked_slice(
                    document.table_stream(),
                    author_location.offset,
                    author_location.length,
                    "GrpXstAtnOwners",
                )?;
                Self::from_plcs(document, story, references, boundaries, authors, limits)?
                    .attach_annotation_bookmarks(document, limits)
            }
            _ => Err(DocError::InvalidComment(
                "comment story, PlcfandRef, PlcfandTxt, and GrpXstAtnOwners must all be present or all be absent".into(),
            )),
        }
    }

    fn from_plcs(
        document: &WordBinaryDocument,
        story: &Story,
        reference_bytes: &[u8],
        boundary_bytes: &[u8],
        author_bytes: &[u8],
        limits: DocLimits,
    ) -> crate::Result<Self> {
        let references = parse_references(reference_bytes, document.fib().stories.main)?;
        if references.len() > limits.max_comments {
            return Err(DocError::ResourceLimit {
                resource: "comment",
                actual: u64::try_from(references.len()).unwrap_or(u64::MAX),
                limit: u64::try_from(limits.max_comments).unwrap_or(u64::MAX),
            });
        }
        let authors = parse_authors(author_bytes)?;
        let boundaries = parse_boundaries(boundary_bytes, references.len(), story)?;
        let mut comments = Vec::with_capacity(references.len());
        for (index, (reference, range)) in references
            .into_iter()
            .zip(boundaries.windows(2))
            .enumerate()
        {
            let author = authors
                .get(usize::from(reference.author_index))
                .ok_or_else(|| {
                    DocError::InvalidComment(format!(
                        "ATRDPre10 author index {} is outside GrpXstAtnOwners count {}",
                        reference.author_index,
                        authors.len()
                    ))
                })?;
            validate_marker(document, reference.cp, "main comment reference")?;
            let cp_start = story
                .cp_start
                .checked_add(range[0])
                .ok_or_else(|| DocError::InvalidComment("comment body start CP overflow".into()))?;
            let cp_end = story
                .cp_start
                .checked_add(range[1])
                .ok_or_else(|| DocError::InvalidComment("comment body end CP overflow".into()))?;
            validate_body(document, cp_start, cp_end)?;
            comments.push(SourceComment {
                comment_id: u32::try_from(index + 1)
                    .map_err(|_| DocError::InvalidComment("comment identifier overflow".into()))?,
                reference_cp: reference.cp,
                cp_start,
                cp_end,
                initials: reference.initials,
                author_index: reference.author_index,
                author: author.clone(),
                bookmark_tag: reference.bookmark_tag,
                anchor_cp_start: None,
                anchor_cp_end: None,
            });
        }
        Ok(Self {
            authors,
            comments,
            range_starts: Vec::new(),
            range_ends: Vec::new(),
        })
    }

    fn attach_annotation_bookmarks(
        mut self,
        document: &WordBinaryDocument,
        limits: DocLimits,
    ) -> crate::Result<Self> {
        let metadata = document
            .fib()
            .locations
            .comment_bookmarks()
            .filter(|location| !location.is_empty());
        let starts = document
            .fib()
            .locations
            .comment_bookmark_starts()
            .filter(|location| !location.is_empty());
        let ends = document
            .fib()
            .locations
            .comment_bookmark_ends()
            .filter(|location| !location.is_empty());
        let has_ranged_comment = self
            .comments
            .iter()
            .any(|comment| comment.bookmark_tag != -1);

        let (metadata, starts, ends) = match (metadata, starts, ends) {
            (None, None, None) if !has_ranged_comment => return Ok(self),
            (Some(metadata), Some(starts), Some(ends)) => (metadata, starts, ends),
            (None, None, None) => {
                return Err(DocError::InvalidComment(
                    "a ranged comment references an absent annotation bookmark table".into(),
                ));
            }
            _ => {
                return Err(DocError::InvalidComment(
                    "SttbfAtnBkmk, PlcfAtnBkf, and PlcfAtnBkl must all be present or all be absent"
                        .into(),
                ));
            }
        };

        let ranges = load_annotation_ranges(document, metadata, starts, ends, limits)?;

        let mut referenced = HashSet::new();
        for comment in &mut self.comments {
            if comment.bookmark_tag == -1 {
                continue;
            }
            if !referenced.insert(comment.bookmark_tag) {
                return Err(DocError::InvalidComment(format!(
                    "annotation bookmark tag {} is referenced by multiple comments",
                    comment.bookmark_tag
                )));
            }
            let (start, end) = ranges.get(&comment.bookmark_tag).copied().ok_or_else(|| {
                DocError::InvalidComment(format!(
                    "comment {} references missing annotation bookmark tag {}",
                    comment.comment_id, comment.bookmark_tag
                ))
            })?;
            comment.anchor_cp_start = Some(start);
            comment.anchor_cp_end = Some(end);
        }
        for (index, comment) in self.comments.iter().enumerate() {
            if let (Some(start), Some(end)) = (comment.anchor_cp_start, comment.anchor_cp_end) {
                self.range_starts.push((start, index));
                self.range_ends.push((end, index));
            }
        }
        self.range_starts.sort_unstable();
        self.range_ends.sort_unstable();
        Ok(self)
    }

    /// Unique author names in source string-table order.
    #[must_use]
    pub fn authors(&self) -> &[String] {
        &self.authors
    }

    #[must_use]
    pub fn comments(&self) -> &[SourceComment] {
        &self.comments
    }

    #[must_use]
    pub fn reference_at(&self, cp: u32) -> Option<&SourceComment> {
        self.comments
            .binary_search_by_key(&cp, |comment| comment.reference_cp)
            .ok()
            .map(|index| &self.comments[index])
    }

    /// Ranged comments beginning at a main-story CP.
    #[must_use]
    pub fn ranges_starting_at(&self, cp: u32) -> impl DoubleEndedIterator<Item = &SourceComment> {
        let start = self.range_starts.partition_point(|event| event.0 < cp);
        let end = self.range_starts.partition_point(|event| event.0 <= cp);
        self.range_starts[start..end]
            .iter()
            .map(|(_, index)| &self.comments[*index])
    }

    /// Ranged comments ending immediately before a main-story CP.
    #[must_use]
    pub fn ranges_ending_at(&self, cp: u32) -> impl DoubleEndedIterator<Item = &SourceComment> {
        let start = self.range_ends.partition_point(|event| event.0 < cp);
        let end = self.range_ends.partition_point(|event| event.0 <= cp);
        self.range_ends[start..end]
            .iter()
            .map(|(_, index)| &self.comments[*index])
    }
}

fn load_annotation_ranges(
    document: &WordBinaryDocument,
    metadata: FcLcb,
    starts: FcLcb,
    ends: FcLcb,
    limits: DocLimits,
) -> crate::Result<HashMap<i32, (u32, u32)>> {
    let table = document.table_stream();
    let metadata = checked_slice(table, metadata.offset, metadata.length, "SttbfAtnBkmk")?;
    let starts = checked_slice(table, starts.offset, starts.length, "PlcfAtnBkf")?;
    let ends = checked_slice(table, ends.offset, ends.length, "PlcfAtnBkl")?;
    let tags = parse_annotation_metadata(metadata, limits)?;
    let starts = parse_annotation_starts(starts, tags.len(), document.fib().stories.main)?;
    let ends = parse_annotation_ends(ends, tags.len(), document.fib().stories.main)?;
    let mut ranges = HashMap::with_capacity(tags.len());
    for (tag, (start, end_index)) in tags.into_iter().zip(starts) {
        let end = *ends.get(end_index).ok_or_else(|| {
            DocError::InvalidComment(format!(
                "annotation bookmark tag {tag} references missing end index {end_index}"
            ))
        })?;
        if start >= end {
            return Err(DocError::InvalidComment(format!(
                "annotation bookmark tag {tag} has empty or reversed range [{start}, {end})"
            )));
        }
        if ranges.insert(tag, (start, end)).is_some() {
            return Err(DocError::InvalidComment(format!(
                "duplicate annotation bookmark tag {tag}"
            )));
        }
    }
    Ok(ranges)
}

fn parse_annotation_metadata(bytes: &[u8], limits: DocLimits) -> crate::Result<Vec<i32>> {
    let mut cursor = ByteCursor::new(bytes, "SttbfAtnBkmk");
    if cursor.read_u16()? != 0xFFFF {
        return Err(DocError::InvalidComment(
            "SttbfAtnBkmk is not an extended STTB".into(),
        ));
    }
    let count = usize::from(cursor.read_u16()?);
    let max = limits.max_comments.min(0x3FFB);
    if count > max {
        return Err(DocError::ResourceLimit {
            resource: "annotation bookmark",
            actual: u64::try_from(count).unwrap_or(u64::MAX),
            limit: u64::try_from(max).unwrap_or(u64::MAX),
        });
    }
    if cursor.read_u16()? != 10 {
        return Err(DocError::InvalidComment(
            "SttbfAtnBkmk.cbExtra must be 10".into(),
        ));
    }
    let mut tags = Vec::with_capacity(count);
    for index in 0..count {
        if cursor.read_u16()? != 0 {
            return Err(DocError::InvalidComment(format!(
                "SttbfAtnBkmk entry {index} has a non-empty string"
            )));
        }
        if cursor.read_u16()? != 0x0100 {
            return Err(DocError::InvalidComment(format!(
                "SttbfAtnBkmk entry {index} is not an annotation bookmark"
            )));
        }
        let tag = cursor.read_u32()?;
        let tag = i32::try_from(tag).map_err(|_| {
            DocError::InvalidComment(format!(
                "SttbfAtnBkmk entry {index} tag does not fit ATRDPre10"
            ))
        })?;
        if cursor.read_i32()? != -1 {
            return Err(DocError::InvalidComment(format!(
                "SttbfAtnBkmk entry {index} lTagOld is not -1"
            )));
        }
        tags.push(tag);
    }
    if cursor.position() != bytes.len() {
        return Err(DocError::InvalidComment(
            "SttbfAtnBkmk contains trailing bytes".into(),
        ));
    }
    Ok(tags)
}

fn parse_annotation_starts(
    bytes: &[u8],
    count: usize,
    main_length: u32,
) -> crate::Result<Vec<(u32, usize)>> {
    let expected = count
        .checked_mul(8)
        .and_then(|value| value.checked_add(4))
        .ok_or_else(|| DocError::InvalidComment("PlcfAtnBkf length overflow".into()))?;
    if bytes.len() != expected {
        return Err(DocError::InvalidComment(format!(
            "PlcfAtnBkf has {} bytes; expected {expected} for {count} bookmarks",
            bytes.len()
        )));
    }
    let mut cursor = ByteCursor::new(bytes, "PlcfAtnBkf");
    let mut positions = Vec::with_capacity(count + 1);
    for _ in 0..=count {
        positions.push(cursor.read_u32()?);
    }
    validate_annotation_positions("PlcfAtnBkf", &positions, main_length)?;
    let mut end_indexes = HashSet::with_capacity(count);
    let mut starts = Vec::with_capacity(count);
    for (index, start) in positions.into_iter().take(count).enumerate() {
        let end_index = usize::from(cursor.read_u16()?);
        let bkc = cursor.read_u16()?;
        if end_index >= count || !end_indexes.insert(end_index) {
            return Err(DocError::InvalidComment(format!(
                "PlcfAtnBkf entry {index} has invalid or duplicate end index {end_index}"
            )));
        }
        if bkc & 0x8080 != 0 {
            return Err(DocError::InvalidComment(format!(
                "PlcfAtnBkf entry {index} has forbidden annotation BKC flags {bkc:#06X}"
            )));
        }
        starts.push((start, end_index));
    }
    Ok(starts)
}

fn parse_annotation_ends(bytes: &[u8], count: usize, main_length: u32) -> crate::Result<Vec<u32>> {
    let expected = count
        .checked_add(1)
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| DocError::InvalidComment("PlcfAtnBkl length overflow".into()))?;
    if bytes.len() != expected {
        return Err(DocError::InvalidComment(format!(
            "PlcfAtnBkl has {} bytes; expected {expected} for {count} bookmarks",
            bytes.len()
        )));
    }
    let mut cursor = ByteCursor::new(bytes, "PlcfAtnBkl");
    let mut positions = Vec::with_capacity(count + 1);
    for _ in 0..=count {
        positions.push(cursor.read_u32()?);
    }
    validate_annotation_positions("PlcfAtnBkl", &positions, main_length)?;
    positions.pop();
    Ok(positions)
}

fn validate_annotation_positions(
    label: &str,
    positions: &[u32],
    main_length: u32,
) -> crate::Result<()> {
    if positions.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(DocError::InvalidComment(format!(
            "{label} CPs are not sorted: {positions:?}"
        )));
    }
    // Bookmark PLC limits may point one CP beyond the main story's terminal
    // paragraph mark; their ignored guard is one greater again.
    let guard = main_length
        .checked_add(2)
        .ok_or_else(|| DocError::InvalidComment("annotation bookmark guard CP overflow".into()))?;
    if positions.last().copied() != Some(guard) {
        return Err(DocError::InvalidComment(format!(
            "{label} guard CP {:?} does not equal main story length plus two {guard}",
            positions.last()
        )));
    }
    let max_position = main_length
        .checked_add(1)
        .ok_or_else(|| DocError::InvalidComment("annotation bookmark CP overflow".into()))?;
    if positions[..positions.len().saturating_sub(1)]
        .iter()
        .any(|cp| *cp > max_position)
    {
        return Err(DocError::InvalidComment(format!(
            "{label} contains a CP outside main story length {main_length}"
        )));
    }
    Ok(())
}

fn parse_authors(bytes: &[u8]) -> crate::Result<Vec<String>> {
    if bytes.is_empty() {
        return Err(DocError::InvalidComment(
            "GrpXstAtnOwners is empty while comments are present".into(),
        ));
    }
    let mut cursor = ByteCursor::new(bytes, "GrpXstAtnOwners");
    let mut authors = Vec::new();
    while cursor.position() < bytes.len() {
        if authors.len() == MAX_COMMENT_AUTHORS {
            return Err(DocError::InvalidComment(format!(
                "GrpXstAtnOwners exceeds {MAX_COMMENT_AUTHORS} entries"
            )));
        }
        let length = usize::from(cursor.read_u16()?);
        if length > MAX_AUTHOR_NAME_CHARS {
            return Err(DocError::InvalidComment(format!(
                "comment author name length {length} exceeds {MAX_AUTHOR_NAME_CHARS}"
            )));
        }
        let byte_length = length.checked_mul(2).ok_or_else(|| {
            DocError::InvalidComment("comment author byte length overflow".into())
        })?;
        let raw = cursor.take(byte_length)?;
        let utf16 = raw
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        let author = String::from_utf16(&utf16).map_err(|_| {
            DocError::InvalidComment("comment author contains invalid UTF-16".into())
        })?;
        if authors.contains(&author) {
            return Err(DocError::InvalidComment(format!(
                "duplicate comment author name {author:?}"
            )));
        }
        authors.push(author);
    }
    Ok(authors)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommentReference {
    cp: u32,
    initials: String,
    author_index: u16,
    bookmark_tag: i32,
}

fn parse_references(bytes: &[u8], main_length: u32) -> crate::Result<Vec<CommentReference>> {
    if bytes.len() < 4 || !(bytes.len() - 4).is_multiple_of(34) {
        return Err(DocError::InvalidComment(format!(
            "PlcfandRef length {} is not 34*n+4",
            bytes.len()
        )));
    }
    let count = (bytes.len() - 4) / 34;
    let mut cursor = ByteCursor::new(bytes, "PlcfandRef");
    let mut positions = Vec::with_capacity(count + 1);
    for _ in 0..=count {
        positions.push(cursor.read_u32()?);
    }
    ensure_strictly_increasing("comment reference", &positions)?;
    if let Some(cp) = positions[..count].iter().find(|cp| **cp >= main_length) {
        return Err(DocError::InvalidComment(format!(
            "comment reference CP {cp} is outside main story length {main_length}"
        )));
    }

    let mut references = Vec::with_capacity(count);
    for cp in positions.into_iter().take(count) {
        let initials_length = usize::from(cursor.read_u16()?);
        if initials_length > 9 {
            return Err(DocError::InvalidComment(format!(
                "ATRDPre10 initials length {initials_length} exceeds 9"
            )));
        }
        let mut initials_utf16 = [0_u16; 9];
        for value in &mut initials_utf16 {
            *value = cursor.read_u16()?;
        }
        let initials = String::from_utf16(&initials_utf16[..initials_length]).map_err(|_| {
            DocError::InvalidComment("ATRDPre10 initials contain invalid UTF-16".into())
        })?;
        let author_index = cursor.read_u16()?;
        let bits_not_used = cursor.read_u16()?;
        let grf_not_used = cursor.read_u16()?;
        if bits_not_used != 0 || grf_not_used != 0 {
            return Err(DocError::InvalidComment(format!(
                "ATRDPre10 reserved fields must be zero, got {bits_not_used:#06X}/{grf_not_used:#06X}"
            )));
        }
        let bookmark_tag = cursor.read_i32()?;
        references.push(CommentReference {
            cp,
            initials,
            author_index,
            bookmark_tag,
        });
    }
    Ok(references)
}

fn parse_boundaries(bytes: &[u8], count: usize, story: &Story) -> crate::Result<Vec<u32>> {
    let expected_count = count
        .checked_add(2)
        .ok_or_else(|| DocError::InvalidComment("comment boundary count overflow".into()))?;
    let expected_bytes = expected_count
        .checked_mul(4)
        .ok_or_else(|| DocError::InvalidComment("comment boundary byte count overflow".into()))?;
    if bytes.len() != expected_bytes {
        return Err(DocError::InvalidComment(format!(
            "PlcfandTxt has {} bytes; expected {expected_bytes} for {count} comments",
            bytes.len()
        )));
    }
    let mut cursor = ByteCursor::new(bytes, "PlcfandTxt");
    let mut positions = Vec::with_capacity(expected_count);
    for _ in 0..expected_count {
        positions.push(cursor.read_u32()?);
    }
    let used = &positions[..=count];
    ensure_strictly_increasing("comment body", used)?;
    let story_length = story.cp_end - story.cp_start;
    let expected_end = story_length.checked_sub(1).ok_or_else(|| {
        DocError::InvalidComment("comment story has no trailing guard character".into())
    })?;
    if used.last().copied() != Some(expected_end) {
        return Err(DocError::InvalidComment(format!(
            "final comment body boundary {:?} does not equal story length minus guard {expected_end}",
            used.last()
        )));
    }
    Ok(used.to_vec())
}

fn validate_body(document: &WordBinaryDocument, cp_start: u32, cp_end: u32) -> crate::Result<()> {
    if cp_end <= cp_start + 1 {
        return Err(DocError::InvalidComment(format!(
            "comment body [{cp_start}, {cp_end}) has no marker and terminal paragraph"
        )));
    }
    validate_marker(document, cp_start, "comment body")?;
    if document.decode_range(cp_end - 1, cp_end)?.utf16 != [0x000D] {
        return Err(DocError::InvalidComment(format!(
            "comment body [{cp_start}, {cp_end}) does not end in a paragraph mark"
        )));
    }
    Ok(())
}

fn validate_marker(document: &WordBinaryDocument, cp: u32, label: &str) -> crate::Result<()> {
    if document.decode_range(cp, cp + 1)?.utf16 != [0x0005] {
        return Err(DocError::InvalidComment(format!(
            "{label} CP {cp} is not the required 0x0005 marker"
        )));
    }
    Ok(())
}

fn ensure_strictly_increasing(label: &str, positions: &[u32]) -> crate::Result<()> {
    if positions.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DocError::InvalidComment(format!(
            "{label} CPs are not strictly increasing: {positions:?}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plcfandref_metadata_and_boundaries() {
        let mut references = Vec::new();
        references.extend_from_slice(&4_u32.to_le_bytes());
        references.extend_from_slice(&9_u32.to_le_bytes());
        references.extend_from_slice(&2_u16.to_le_bytes());
        for value in ['A' as u16, 'Б' as u16]
            .into_iter()
            .chain(std::iter::repeat_n(0, 7))
        {
            references.extend_from_slice(&value.to_le_bytes());
        }
        references.extend_from_slice(&3_u16.to_le_bytes());
        references.extend_from_slice(&0_u16.to_le_bytes());
        references.extend_from_slice(&0_u16.to_le_bytes());
        references.extend_from_slice(&(-1_i32).to_le_bytes());
        assert_eq!(
            parse_references(&references, 20).unwrap(),
            vec![CommentReference {
                cp: 4,
                initials: "AБ".into(),
                author_index: 3,
                bookmark_tag: -1,
            }]
        );

        let story = Story {
            kind: StoryKind::Comments,
            cp_start: 20,
            cp_end: 28,
            content: crate::DecodedText {
                cp_start: 20,
                cp_end: 28,
                text: String::new(),
                utf16: Vec::new(),
            },
        };
        let mut boundaries = Vec::new();
        for cp in [0_u32, 7, 999] {
            boundaries.extend_from_slice(&cp.to_le_bytes());
        }
        assert_eq!(
            parse_boundaries(&boundaries, 1, &story).unwrap(),
            vec![0, 7]
        );
    }

    #[test]
    fn rejects_bad_plc_shape_reserved_fields_and_duplicates() {
        assert!(parse_references(&[0; 9], 10).is_err());
        let mut duplicate = Vec::new();
        duplicate.extend_from_slice(&2_u32.to_le_bytes());
        duplicate.extend_from_slice(&2_u32.to_le_bytes());
        duplicate.extend_from_slice(&[0; 30]);
        assert!(parse_references(&duplicate, 10).is_err());

        let mut reserved = Vec::new();
        reserved.extend_from_slice(&2_u32.to_le_bytes());
        reserved.extend_from_slice(&3_u32.to_le_bytes());
        reserved.extend_from_slice(&[0; 22]);
        reserved.extend_from_slice(&1_u16.to_le_bytes());
        reserved.extend_from_slice(&0_u16.to_le_bytes());
        reserved.extend_from_slice(&(-1_i32).to_le_bytes());
        assert!(parse_references(&reserved, 10).is_err());
    }

    #[test]
    fn parses_unique_utf16_comment_authors_and_rejects_invalid_tables() {
        let mut bytes = Vec::new();
        for author in ["Anonymous", "Райан"] {
            let utf16 = author.encode_utf16().collect::<Vec<_>>();
            bytes.extend_from_slice(&u16::try_from(utf16.len()).unwrap().to_le_bytes());
            for value in utf16 {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        assert_eq!(
            parse_authors(&bytes).unwrap(),
            ["Anonymous".to_string(), "Райан".to_string()]
        );

        let duplicate = [1, 0, b'A', 0, 1, 0, b'A', 0];
        assert!(parse_authors(&duplicate).is_err());
        assert!(parse_authors(&[56, 0]).is_err());
        assert!(parse_authors(&[1, 0, b'A']).is_err());
    }

    #[test]
    fn parses_and_indexes_annotation_bookmark_ranges() {
        let mut metadata = Vec::new();
        metadata.extend_from_slice(&0xFFFF_u16.to_le_bytes());
        metadata.extend_from_slice(&2_u16.to_le_bytes());
        metadata.extend_from_slice(&10_u16.to_le_bytes());
        for tag in [17_u32, 23] {
            metadata.extend_from_slice(&0_u16.to_le_bytes());
            metadata.extend_from_slice(&0x0100_u16.to_le_bytes());
            metadata.extend_from_slice(&tag.to_le_bytes());
            metadata.extend_from_slice(&(-1_i32).to_le_bytes());
        }
        assert_eq!(
            parse_annotation_metadata(&metadata, DocLimits::default()).unwrap(),
            [17, 23]
        );

        let mut starts = Vec::new();
        for cp in [1_u32, 2, 12] {
            starts.extend_from_slice(&cp.to_le_bytes());
        }
        starts.extend_from_slice(&1_u16.to_le_bytes());
        starts.extend_from_slice(&0_u16.to_le_bytes());
        starts.extend_from_slice(&0_u16.to_le_bytes());
        starts.extend_from_slice(&0_u16.to_le_bytes());
        assert_eq!(
            parse_annotation_starts(&starts, 2, 10).unwrap(),
            [(1, 1), (2, 0)]
        );

        let mut ends = Vec::new();
        for cp in [4_u32, 8, 12] {
            ends.extend_from_slice(&cp.to_le_bytes());
        }
        assert_eq!(parse_annotation_ends(&ends, 2, 10).unwrap(), [4, 8]);

        let comments = vec![
            SourceComment {
                comment_id: 1,
                reference_cp: 8,
                cp_start: 10,
                cp_end: 13,
                initials: String::new(),
                author_index: 0,
                author: String::new(),
                bookmark_tag: 17,
                anchor_cp_start: Some(1),
                anchor_cp_end: Some(8),
            },
            SourceComment {
                comment_id: 2,
                reference_cp: 4,
                cp_start: 13,
                cp_end: 16,
                initials: String::new(),
                author_index: 0,
                author: String::new(),
                bookmark_tag: 23,
                anchor_cp_start: Some(2),
                anchor_cp_end: Some(4),
            },
        ];
        let collection = CommentCollection {
            authors: vec![String::new()],
            comments,
            range_starts: vec![(1, 0), (2, 1)],
            range_ends: vec![(4, 1), (8, 0)],
        };
        assert_eq!(
            collection
                .ranges_starting_at(2)
                .map(|comment| comment.comment_id)
                .collect::<Vec<_>>(),
            [2]
        );
        assert_eq!(
            collection
                .ranges_ending_at(8)
                .map(|comment| comment.comment_id)
                .collect::<Vec<_>>(),
            [1]
        );

        let mut forbidden = starts;
        let bkc_offset = (2 + 1) * 4 + 2;
        forbidden[bkc_offset..bkc_offset + 2].copy_from_slice(&0x8000_u16.to_le_bytes());
        assert!(parse_annotation_starts(&forbidden, 2, 10).is_err());
    }
}
