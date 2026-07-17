//! Header/footer story boundaries and per-section inheritance.

use crate::{
    DocError, DocLimits, Result, Story, StoryKind, WordBinaryDocument, binary::checked_slice,
};

const SEPARATOR_STORY_COUNT: usize = 6;
const STORIES_PER_SECTION: usize = 6;

/// Semantic role of one `PlcfHdd` story.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderFooterKind {
    FootnoteSeparator,
    FootnoteContinuationSeparator,
    FootnoteContinuationNotice,
    EndnoteSeparator,
    EndnoteContinuationSeparator,
    EndnoteContinuationNotice,
    EvenHeader,
    OddHeader,
    EvenFooter,
    OddFooter,
    FirstHeader,
    FirstFooter,
}

/// One source story; `cp_end` includes its guard paragraph when non-empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderFooterStory {
    pub index: usize,
    pub kind: HeaderFooterKind,
    pub section_index: Option<usize>,
    pub cp_start: u32,
    pub cp_content_end: u32,
    pub cp_end: u32,
}

impl HeaderFooterStory {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.cp_start == self.cp_end
    }
}

/// A resolved section link to a non-empty source story.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderFooterLink {
    pub story_index: usize,
    /// True when an empty current-section story inherited this source.
    pub inherited: bool,
}

/// Six resolved header/footer roles for one section.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SectionHeaderFooters {
    pub section_index: usize,
    pub even_header: Option<HeaderFooterLink>,
    pub odd_header: Option<HeaderFooterLink>,
    pub even_footer: Option<HeaderFooterLink>,
    pub odd_footer: Option<HeaderFooterLink>,
    pub first_header: Option<HeaderFooterLink>,
    pub first_footer: Option<HeaderFooterLink>,
}

/// All source descriptors and resolved section links.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeaderFooterTable {
    stories: Vec<HeaderFooterStory>,
    sections: Vec<SectionHeaderFooters>,
}

impl HeaderFooterTable {
    /// Parses `PlcfHdd`, strips only guard paragraph marks from content ranges,
    /// and resolves the specified empty-story inheritance between sections.
    ///
    /// # Errors
    ///
    /// Returns a typed error for invalid counts, CPs, guards, or limits.
    pub fn parse(document: &WordBinaryDocument, limits: DocLimits) -> Result<Self> {
        let section_count = document.sections(limits)?.len();
        let Some(header_story) = document.story(StoryKind::Headers) else {
            return Ok(Self::default());
        };
        if header_story.cp_start == header_story.cp_end {
            return Ok(Self::default());
        }
        let location = document
            .fib()
            .locations
            .headers()
            .filter(|location| !location.is_empty())
            .ok_or_else(|| {
                DocError::InvalidHeaderFooter("header story exists but PlcfHdd is empty".into())
            })?;
        let plc = checked_slice(
            document.table_stream(),
            location.offset,
            location.length,
            "PlcfHdd",
        )?;
        Self::parse_plc(plc, header_story, section_count, limits)
    }

    fn parse_plc(
        plc: &[u8],
        header_story: &Story,
        section_count: usize,
        limits: DocLimits,
    ) -> Result<Self> {
        if !plc.len().is_multiple_of(4) || plc.len() < 8 {
            return Err(DocError::InvalidHeaderFooter(format!(
                "PlcfHdd length {} is not a non-empty CP array",
                plc.len()
            )));
        }
        // PlcfHdd has one boundary after the last story and then a final,
        // undefined CP which is ignored by MS-DOC.
        let story_count = plc.len() / 4 - 2;
        let expected = SEPARATOR_STORY_COUNT
            .checked_add(
                section_count
                    .checked_mul(STORIES_PER_SECTION)
                    .ok_or_else(|| {
                        DocError::InvalidHeaderFooter("header story count overflow".into())
                    })?,
            )
            .ok_or_else(|| DocError::InvalidHeaderFooter("header story count overflow".into()))?;
        if story_count != expected {
            return Err(DocError::InvalidHeaderFooter(format!(
                "PlcfHdd has {story_count} stories; {section_count} sections require {expected}"
            )));
        }
        if story_count > limits.max_header_footer_stories {
            return Err(DocError::ResourceLimit {
                resource: "header-footer-story",
                actual: u64::try_from(story_count).unwrap_or(u64::MAX),
                limit: u64::try_from(limits.max_header_footer_stories).unwrap_or(u64::MAX),
            });
        }
        let cps = plc[..plc.len() - 4]
            .chunks_exact(4)
            .map(|chunk| i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .map(|cp| {
                u32::try_from(cp).map_err(|_| {
                    DocError::InvalidHeaderFooter(format!("PlcfHdd contains negative CP {cp}"))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if cps.first().copied() != Some(0) || cps.windows(2).any(|pair| pair[0] > pair[1]) {
            return Err(DocError::InvalidHeaderFooter(
                "PlcfHdd CPs must begin at zero and be nondecreasing".into(),
            ));
        }
        let expected_terminal = header_story
            .cp_end
            .checked_sub(header_story.cp_start)
            .and_then(|length| length.checked_sub(1))
            .ok_or_else(|| {
                DocError::InvalidHeaderFooter("header story is too short for PlcfHdd".into())
            })?;
        if cps.last().copied() != Some(expected_terminal) {
            return Err(DocError::InvalidHeaderFooter(format!(
                "PlcfHdd terminal CP {:?} does not match ccpHdd-1 ({expected_terminal})",
                cps.last(),
            )));
        }

        let mut stories = Vec::with_capacity(story_count);
        for index in 0..story_count {
            let relative_start = cps[index];
            let relative_end = cps[index + 1];
            let cp_start = header_story
                .cp_start
                .checked_add(relative_start)
                .ok_or_else(|| DocError::InvalidHeaderFooter("story CP overflow".into()))?;
            let cp_end = header_story
                .cp_start
                .checked_add(relative_end)
                .ok_or_else(|| DocError::InvalidHeaderFooter("story CP overflow".into()))?;
            let (kind, section_index) = story_role(index);
            let cp_content_end =
                validate_guard(header_story, cp_start, cp_end, section_index.is_some())?;
            stories.push(HeaderFooterStory {
                index,
                kind,
                section_index,
                cp_start,
                cp_content_end,
                cp_end,
            });
        }
        let sections = resolve_section_links(&stories, section_count);
        Ok(Self { stories, sections })
    }

    #[must_use]
    pub fn stories(&self) -> &[HeaderFooterStory] {
        &self.stories
    }

    #[must_use]
    pub fn sections(&self) -> &[SectionHeaderFooters] {
        &self.sections
    }
}

impl WordBinaryDocument {
    /// Parses and resolves section header/footer stories.
    ///
    /// # Errors
    ///
    /// Returns a typed section or header/footer structure error.
    pub fn header_footers(&self, limits: DocLimits) -> Result<HeaderFooterTable> {
        HeaderFooterTable::parse(self, limits)
    }
}

fn validate_guard(
    story: &Story,
    cp_start: u32,
    cp_end: u32,
    whole_paragraph_story: bool,
) -> Result<u32> {
    if cp_start == cp_end {
        return Ok(cp_end);
    }
    let last_relative = cp_end
        .checked_sub(story.cp_start)
        .and_then(|length| length.checked_sub(1))
        .ok_or_else(|| {
            DocError::InvalidHeaderFooter(format!(
                "story [{cp_start}, {cp_end}) falls outside its header subdocument"
            ))
        })?;
    let last = usize::try_from(last_relative)
        .map_err(|_| DocError::InvalidHeaderFooter("guard offset overflow".into()))?;
    if story.content.utf16.get(last) != Some(&0x000D) {
        return Err(DocError::InvalidHeaderFooter(format!(
            "non-empty story [{cp_start}, {cp_end}) has no guard paragraph mark"
        )));
    }
    let content_end = cp_end - 1;
    if whole_paragraph_story {
        let content_last_relative = content_end
            .checked_sub(story.cp_start)
            .and_then(|length| length.checked_sub(1))
            .ok_or_else(|| {
                DocError::InvalidHeaderFooter(format!(
                    "header/footer story [{cp_start}, {cp_end}) has no complete content paragraph"
                ))
            })?;
        let content_last = usize::try_from(content_last_relative).map_err(|_| {
            DocError::InvalidHeaderFooter("header/footer content offset overflow".into())
        })?;
        if story.content.utf16.get(content_last) != Some(&0x000D) {
            return Err(DocError::InvalidHeaderFooter(format!(
                "header/footer story [{cp_start}, {cp_end}) does not contain a complete paragraph before its guard"
            )));
        }
    }
    Ok(content_end)
}

fn story_role(index: usize) -> (HeaderFooterKind, Option<usize>) {
    if index < SEPARATOR_STORY_COUNT {
        let kind = [
            HeaderFooterKind::FootnoteSeparator,
            HeaderFooterKind::FootnoteContinuationSeparator,
            HeaderFooterKind::FootnoteContinuationNotice,
            HeaderFooterKind::EndnoteSeparator,
            HeaderFooterKind::EndnoteContinuationSeparator,
            HeaderFooterKind::EndnoteContinuationNotice,
        ][index];
        return (kind, None);
    }
    let relative = index - SEPARATOR_STORY_COUNT;
    let kind = [
        HeaderFooterKind::EvenHeader,
        HeaderFooterKind::OddHeader,
        HeaderFooterKind::EvenFooter,
        HeaderFooterKind::OddFooter,
        HeaderFooterKind::FirstHeader,
        HeaderFooterKind::FirstFooter,
    ][relative % STORIES_PER_SECTION];
    (kind, Some(relative / STORIES_PER_SECTION))
}

fn resolve_section_links(
    stories: &[HeaderFooterStory],
    section_count: usize,
) -> Vec<SectionHeaderFooters> {
    let mut previous = [None; STORIES_PER_SECTION];
    let mut result = Vec::with_capacity(section_count);
    for section_index in 0..section_count {
        let start = SEPARATOR_STORY_COUNT + section_index * STORIES_PER_SECTION;
        let mut links = [None; STORIES_PER_SECTION];
        for role in 0..STORIES_PER_SECTION {
            let story = &stories[start + role];
            links[role] = if story.is_empty() {
                previous[role].map(|story_index| HeaderFooterLink {
                    story_index,
                    inherited: true,
                })
            } else {
                previous[role] = Some(story.index);
                Some(HeaderFooterLink {
                    story_index: story.index,
                    inherited: false,
                })
            };
        }
        result.push(SectionHeaderFooters {
            section_index,
            even_header: links[0],
            odd_header: links[1],
            even_footer: links[2],
            odd_footer: links[3],
            first_header: links[4],
            first_footer: links[5],
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DecodedText;

    #[test]
    fn strips_guards_and_inherits_empty_second_section_story() {
        let utf16 = vec![u16::from(b'H'), 0x000D, 0x000D, 0x000D];
        let story = Story {
            kind: StoryKind::Headers,
            cp_start: 100,
            cp_end: 104,
            content: DecodedText {
                cp_start: 100,
                cp_end: 104,
                text: String::from_utf16_lossy(&utf16),
                utf16,
            },
        };
        let mut cps = [0_u32; 20];
        cps[8..19].fill(3);
        let plc = cps
            .iter()
            .flat_map(|cp| cp.to_le_bytes())
            .collect::<Vec<_>>();
        let table = HeaderFooterTable::parse_plc(&plc, &story, 2, DocLimits::default()).unwrap();
        assert_eq!(table.stories()[7].cp_content_end, 102);
        assert_eq!(
            table.sections()[0].odd_header,
            Some(HeaderFooterLink {
                story_index: 7,
                inherited: false
            })
        );
        assert_eq!(
            table.sections()[1].odd_header,
            Some(HeaderFooterLink {
                story_index: 7,
                inherited: true
            })
        );
    }
}
