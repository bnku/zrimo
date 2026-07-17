//! Bounded BTE PLCF and PAPX/CHPX FKP parsing.

use std::collections::HashMap;

use crate::{
    CharacterPropertyDelta, DocError, DocLimits, FcLcb, Fib, ParagraphPropertyDelta,
    PiecePropertyModifier, PieceTable, PropertyGroup, Result, Sprm, StyleKind, StyleSheet,
    TextPiece, apply_character_sprms, apply_paragraph_sprms, binary::checked_slice, decode_grpprl,
};

const FKP_SIZE: usize = 512;
const FKP_SIZE_U32: u32 = 512;
const PAGE_NUMBER_MASK: u32 = 0x003F_FFFF;
const MAX_CHPX_RUNS_PER_FKP: usize = 0x65;
const MAX_PAPX_RUNS_PER_FKP: usize = 0x1D;
const BX_PAP_SIZE: usize = 13;

/// One direct-formatting run backed by a physical `WordDocument` byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattingRun {
    /// Inclusive physical stream offset where this run begins.
    pub file_start: u32,
    /// Exclusive physical stream offset where this run ends.
    pub file_end: u32,
    /// Paragraph style index from `GrpPrlAndIstd`, for PAPX runs only.
    pub paragraph_style: Option<u16>,
    /// Raw, exactly bounded array of `Prl` structures.
    pub grpprl: Vec<u8>,
}

/// Direct character and paragraph formatting indexed by physical ranges.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormattingIndex {
    /// CHPX runs in BTE/FKP order.
    pub character_runs: Vec<FormattingRun>,
    /// PAPX runs in BTE/FKP order.
    pub paragraph_runs: Vec<FormattingRun>,
}

/// One formatting run converted from a physical FC range into a logical CP range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalFormattingRun {
    /// Inclusive global character position.
    pub cp_start: u32,
    /// Exclusive global character position.
    pub cp_end: u32,
    /// Physical byte start retained for diagnostics and structural oracles.
    pub file_start: u32,
    /// Physical byte end retained for diagnostics and structural oracles.
    pub file_end: u32,
    /// Paragraph style index for PAPX runs.
    pub paragraph_style: Option<u16>,
    /// Direct properties stored in the FKP run.
    pub grpprl: Vec<u8>,
    /// Piece-level PRM that is applied after FKP direct properties.
    pub piece_prm: u16,
}

/// Character and paragraph formatting in logical document order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogicalFormattingIndex {
    /// CHPX runs split at text-piece boundaries.
    pub character_runs: Vec<LogicalFormattingRun>,
    /// PAPX runs split at text-piece boundaries.
    pub paragraph_runs: Vec<LogicalFormattingRun>,
}

/// One logical character run with FKP and piece properties applied in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticCharacterRun {
    /// Source-backed CP/FC range and raw direct formatting.
    pub source: LogicalFormattingRun,
    /// Resolved PCD modifier, including unresolved compact values.
    pub piece_modifier: PiecePropertyModifier,
    /// FKP character SPRMs followed by applicable piece SPRMs.
    pub sprms: Vec<Sprm>,
    /// Known typed direct properties; style-relative toggles stay unresolved.
    pub properties: CharacterPropertyDelta,
}

/// One logical paragraph run with FKP and piece properties applied in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticParagraphRun {
    /// Source-backed CP/FC range, paragraph style index, and raw formatting.
    pub source: LogicalFormattingRun,
    /// Resolved PCD modifier, including unresolved compact values.
    pub piece_modifier: PiecePropertyModifier,
    /// FKP paragraph SPRMs followed by applicable piece SPRMs.
    pub sprms: Vec<Sprm>,
    /// Known typed direct paragraph properties.
    pub properties: ParagraphPropertyDelta,
}

/// Typed direct formatting in logical source order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemanticFormattingIndex {
    pub character_runs: Vec<SemanticCharacterRun>,
    pub paragraph_runs: Vec<SemanticParagraphRun>,
}

/// One paragraph run after STSH inheritance and direct properties are ordered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledParagraphRun {
    /// Index into [`StyledFormattingIndex::direct`] paragraph runs.
    pub direct_run_index: usize,
    pub cp_start: u32,
    pub cp_end: u32,
    pub style_index: Option<u16>,
    /// Default/style modifiers followed by FKP and piece modifiers.
    pub sprms: Vec<Sprm>,
    pub properties: ParagraphPropertyDelta,
}

/// One character run split at paragraph boundaries and fully ordered by source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledCharacterRun {
    /// Index into [`StyledFormattingIndex::direct`] character runs.
    pub direct_run_index: usize,
    /// Overlapping styled paragraph run, if formatting tables left a gap.
    pub paragraph_run_index: Option<usize>,
    pub cp_start: u32,
    pub cp_end: u32,
    pub paragraph_style_index: Option<u16>,
    pub character_style_index: Option<u16>,
    /// STSH defaults, paragraph-style character properties, character style,
    /// and direct CHPX/PRM modifiers in normative application order.
    pub sprms: Vec<Sprm>,
    pub properties: CharacterPropertyDelta,
}

/// Direct and style-aware formatting retained together for source diagnostics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StyledFormattingIndex {
    pub direct: SemanticFormattingIndex,
    pub paragraph_runs: Vec<StyledParagraphRun>,
    pub character_runs: Vec<StyledCharacterRun>,
}

impl FormattingIndex {
    /// Parses the character and paragraph BTE/FKP structures referenced by FIB.
    ///
    /// # Errors
    ///
    /// Returns a typed [`DocError`] when a location, PLCF, page, run, or count
    /// is malformed or exceeds a configured budget.
    pub fn parse(
        fib: &Fib,
        word_document: &[u8],
        table_stream: &[u8],
        limits: DocLimits,
    ) -> Result<Self> {
        Ok(Self {
            character_runs: parse_kind(
                fib.locations.character_bte(),
                word_document,
                table_stream,
                limits,
                FkpKind::Character,
            )?,
            paragraph_runs: parse_kind(
                fib.locations.paragraph_bte(),
                word_document,
                table_stream,
                limits,
                FkpKind::Paragraph,
            )?,
        })
    }

    /// Converts physical FKP ranges to source-aligned logical CP ranges.
    ///
    /// Runs are split at piece boundaries so compressed and UTF-16 pieces never
    /// share one conversion formula. Gaps in physical storage are ignored.
    ///
    /// # Errors
    ///
    /// Returns [`DocError::InvalidFormatting`] when an FKP boundary cuts through
    /// the middle of a UTF-16 code unit or checked FC/CP arithmetic overflows.
    pub fn to_logical(&self, pieces: &PieceTable) -> Result<LogicalFormattingIndex> {
        Ok(LogicalFormattingIndex {
            character_runs: map_runs(&self.character_runs, pieces.pieces())?,
            paragraph_runs: map_runs(&self.paragraph_runs, pieces.pieces())?,
        })
    }
}

impl LogicalFormattingIndex {
    /// Applies each piece's `Prm` after its FKP modifiers and derives typed
    /// character/paragraph property deltas without resolving STSH inheritance.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed `grpprl`, invalid known operands, or
    /// a `Prm1` index outside the CLX `RgPrc` array.
    pub fn resolve_properties(&self, pieces: &PieceTable) -> Result<SemanticFormattingIndex> {
        let mut modifier_cache = HashMap::<u16, PiecePropertyModifier>::new();
        let mut character_runs = Vec::with_capacity(self.character_runs.len());
        for run in &self.character_runs {
            let modifier = resolve_modifier(pieces, run.piece_prm, &mut modifier_cache)?;
            let mut sprms = decode_formatting_run(run, "CHPX")?;
            sprms.extend(modifier.sprms_for(PropertyGroup::Character));
            let properties = apply_character_sprms(&sprms)?;
            character_runs.push(SemanticCharacterRun {
                source: run.clone(),
                piece_modifier: modifier,
                sprms,
                properties,
            });
        }

        let mut paragraph_runs = Vec::with_capacity(self.paragraph_runs.len());
        for run in &self.paragraph_runs {
            let modifier = resolve_modifier(pieces, run.piece_prm, &mut modifier_cache)?;
            let mut sprms = decode_formatting_run(run, "PAPX")?;
            sprms.extend(modifier.sprms_for(PropertyGroup::Paragraph));
            let properties = apply_paragraph_sprms(&sprms)?;
            paragraph_runs.push(SemanticParagraphRun {
                source: run.clone(),
                piece_modifier: modifier,
                sprms,
                properties,
            });
        }
        Ok(SemanticFormattingIndex {
            character_runs,
            paragraph_runs,
        })
    }
}

fn decode_formatting_run(run: &LogicalFormattingRun, kind: &str) -> Result<Vec<Sprm>> {
    let decoded = decode_grpprl(&run.grpprl).or_else(|error| {
        // Extended PapxInFkp records are word-sized in storage. Producers can
        // leave one zero alignment byte after the final complete Prl; accept
        // only that exact case and only when the preceding bytes decode fully.
        if kind == "PAPX" && run.grpprl.last() == Some(&0) {
            decode_grpprl(&run.grpprl[..run.grpprl.len() - 1])
        } else {
            Err(error)
        }
    });
    decoded.map_err(|error| match error {
        DocError::InvalidFormatting(message) => {
            let preview_length = run.grpprl.len().min(32);
            DocError::InvalidFormatting(format!(
                "{kind} run FC [{}, {}) / CP [{}, {}) has malformed {}-byte grpprl ({:02X?}): {message}",
                run.file_start,
                run.file_end,
                run.cp_start,
                run.cp_end,
                run.grpprl.len(),
                &run.grpprl[..preview_length],
            ))
        }
        other => other,
    })
}

impl SemanticFormattingIndex {
    /// Applies stylesheet inheritance around direct formatting and splits CHPX
    /// runs at paragraph boundaries before assigning paragraph style effects.
    ///
    /// # Errors
    ///
    /// Returns a typed error for absent/wrong-kind styles or invalid known
    /// properties after all source layers are placed in normative order.
    pub fn apply_styles(&self, styles: &StyleSheet) -> Result<StyledFormattingIndex> {
        let mut inherited_cache = HashMap::new();
        let mut paragraph_runs = Vec::with_capacity(self.paragraph_runs.len());
        for (direct_run_index, direct) in self.paragraph_runs.iter().enumerate() {
            let style_index = direct
                .properties
                .style_index
                .or(direct.source.paragraph_style);
            let mut sprms = Vec::new();
            if let Some(index) = style_index {
                require_style_kind(styles, index, StyleKind::Paragraph)?;
                let inherited = inherited_style(styles, index, &mut inherited_cache)?;
                sprms.extend(inherited.paragraph_sprms.iter().cloned());
            }
            sprms.extend(direct.sprms.iter().cloned());
            let properties = apply_paragraph_sprms(&sprms)?;
            paragraph_runs.push(StyledParagraphRun {
                direct_run_index,
                cp_start: direct.source.cp_start,
                cp_end: direct.source.cp_end,
                style_index,
                sprms,
                properties,
            });
        }

        let defaults = styles.default_character_sprms();
        let mut character_runs = Vec::new();
        let mut paragraph_cursor = 0_usize;
        for (direct_run_index, direct) in self.character_runs.iter().enumerate() {
            let mut cp = direct.source.cp_start;
            while cp < direct.source.cp_end {
                while paragraph_cursor < paragraph_runs.len()
                    && paragraph_runs[paragraph_cursor].cp_end <= cp
                {
                    paragraph_cursor += 1;
                }
                let paragraph_run_index = paragraph_runs
                    .get(paragraph_cursor)
                    .filter(|paragraph| paragraph.cp_start <= cp && cp < paragraph.cp_end)
                    .map(|_| paragraph_cursor);
                let cp_end = paragraph_run_index.map_or_else(
                    || {
                        paragraph_runs
                            .get(paragraph_cursor)
                            .map_or(direct.source.cp_end, |next| {
                                next.cp_start.min(direct.source.cp_end)
                            })
                    },
                    |index| paragraph_runs[index].cp_end.min(direct.source.cp_end),
                );
                if cp_end <= cp {
                    return Err(DocError::InvalidFormatting(format!(
                        "cannot advance styled character run at CP {cp}"
                    )));
                }
                let paragraph_style_index =
                    paragraph_run_index.and_then(|index| paragraph_runs[index].style_index);
                let character_style_index = direct.properties.style_index;
                let mut sprms = defaults.clone();
                if let Some(index) = paragraph_style_index {
                    let inherited = inherited_style(styles, index, &mut inherited_cache)?;
                    sprms.extend(inherited.character_sprms.iter().cloned());
                }
                if let Some(index) = character_style_index {
                    require_style_kind(styles, index, StyleKind::Character)?;
                    let inherited = inherited_style(styles, index, &mut inherited_cache)?;
                    sprms.extend(inherited.character_sprms.iter().cloned());
                }
                sprms.extend(direct.sprms.iter().cloned());
                let properties = apply_character_sprms(&sprms)?;
                character_runs.push(StyledCharacterRun {
                    direct_run_index,
                    paragraph_run_index,
                    cp_start: cp,
                    cp_end,
                    paragraph_style_index,
                    character_style_index,
                    sprms,
                    properties,
                });
                cp = cp_end;
            }
        }
        Ok(StyledFormattingIndex {
            direct: self.clone(),
            paragraph_runs,
            character_runs,
        })
    }
}

fn inherited_style<'a>(
    styles: &StyleSheet,
    index: u16,
    cache: &'a mut HashMap<u16, crate::InheritedStyleProperties>,
) -> Result<&'a crate::InheritedStyleProperties> {
    if let std::collections::hash_map::Entry::Vacant(entry) = cache.entry(index) {
        entry.insert(styles.inherited_properties(index)?);
    }
    cache
        .get(&index)
        .ok_or_else(|| DocError::InvalidStyle(format!("style cache lost index {index}")))
}

fn require_style_kind(styles: &StyleSheet, index: u16, expected: StyleKind) -> Result<()> {
    let style = styles
        .get(index)
        .ok_or_else(|| DocError::InvalidStyle(format!("referenced style {index} is empty")))?;
    if style.kind != expected {
        return Err(DocError::InvalidStyle(format!(
            "style {index} has kind {:?}; expected {expected:?}",
            style.kind
        )));
    }
    Ok(())
}

fn resolve_modifier(
    pieces: &PieceTable,
    raw: u16,
    cache: &mut HashMap<u16, PiecePropertyModifier>,
) -> Result<PiecePropertyModifier> {
    if let Some(modifier) = cache.get(&raw) {
        return Ok(modifier.clone());
    }
    let modifier = pieces.resolve_prm(raw)?;
    cache.insert(raw, modifier.clone());
    Ok(modifier)
}

fn map_runs(runs: &[FormattingRun], pieces: &[TextPiece]) -> Result<Vec<LogicalFormattingRun>> {
    let mut logical = Vec::new();
    for piece in pieces {
        let piece_end = piece.file_end()?;
        let mut run_index = runs.partition_point(|run| run.file_end <= piece.file_offset);
        while let Some(run) = runs.get(run_index) {
            if run.file_start >= piece_end {
                break;
            }
            let file_start = run.file_start.max(piece.file_offset);
            let file_end = run.file_end.min(piece_end);
            if file_start < file_end {
                logical.push(map_intersection(*piece, run, file_start, file_end)?);
            }
            run_index += 1;
        }
    }
    Ok(logical)
}

fn map_intersection(
    piece: TextPiece,
    run: &FormattingRun,
    file_start: u32,
    file_end: u32,
) -> Result<LogicalFormattingRun> {
    let bytes_per_cp = piece.bytes_per_cp();
    let relative_start = file_start - piece.file_offset;
    let relative_end = file_end - piece.file_offset;
    if !relative_start.is_multiple_of(bytes_per_cp) || !relative_end.is_multiple_of(bytes_per_cp) {
        return Err(DocError::InvalidFormatting(format!(
            "FKP range [{file_start}, {file_end}) cuts through a {:?} text piece",
            piece.encoding
        )));
    }
    let cp_start = piece
        .cp_start
        .checked_add(relative_start / bytes_per_cp)
        .ok_or_else(|| DocError::InvalidFormatting("logical run CP start overflow".into()))?;
    let cp_end = piece
        .cp_start
        .checked_add(relative_end / bytes_per_cp)
        .ok_or_else(|| DocError::InvalidFormatting("logical run CP end overflow".into()))?;
    Ok(LogicalFormattingRun {
        cp_start,
        cp_end,
        file_start,
        file_end,
        paragraph_style: run.paragraph_style,
        grpprl: run.grpprl.clone(),
        piece_prm: piece.prm,
    })
}

#[derive(Debug, Clone, Copy)]
enum FkpKind {
    Character,
    Paragraph,
}

#[derive(Debug, Clone, Copy)]
struct BinTableEntry {
    file_start: u32,
    file_end: u32,
    page_number: u32,
}

fn parse_kind(
    location: Option<FcLcb>,
    word_document: &[u8],
    table_stream: &[u8],
    limits: DocLimits,
    kind: FkpKind,
) -> Result<Vec<FormattingRun>> {
    let Some(location) = location.filter(|location| !location.is_empty()) else {
        return Ok(Vec::new());
    };
    let structure = match kind {
        FkpKind::Character => "PlcBteChpx",
        FkpKind::Paragraph => "PlcBtePapx",
    };
    let plc = checked_slice(table_stream, location.offset, location.length, structure)?;
    let entries = parse_bin_table(plc, limits, structure)?;
    let mut result = Vec::new();
    for entry in entries {
        let page_offset = entry.page_number.checked_mul(FKP_SIZE_U32).ok_or_else(|| {
            DocError::InvalidFormatting(format!("{structure} page offset overflow"))
        })?;
        let page = checked_slice(
            word_document,
            page_offset,
            FKP_SIZE_U32,
            match kind {
                FkpKind::Character => "ChpxFkp",
                FkpKind::Paragraph => "PapxFkp",
            },
        )?;
        let page_runs = match kind {
            FkpKind::Character => parse_chpx_fkp(page)?,
            FkpKind::Paragraph => parse_papx_fkp(page)?,
        };
        if let (Some(first), Some(last)) = (page_runs.first(), page_runs.last())
            && (first.file_start < entry.file_start || last.file_end > entry.file_end)
        {
            return Err(DocError::InvalidFormatting(format!(
                "FKP range [{}, {}) escapes {structure} range [{}, {})",
                first.file_start, last.file_end, entry.file_start, entry.file_end
            )));
        }
        let next_count = result
            .len()
            .checked_add(page_runs.len())
            .ok_or_else(|| DocError::InvalidFormatting("formatting run count overflow".into()))?;
        if next_count > limits.max_formatting_runs {
            return Err(DocError::ResourceLimit {
                resource: "formatting-run",
                actual: u64::try_from(next_count).unwrap_or(u64::MAX),
                limit: u64::try_from(limits.max_formatting_runs).unwrap_or(u64::MAX),
            });
        }
        result.extend(page_runs);
    }
    if result
        .windows(2)
        .any(|pair| pair[0].file_end > pair[1].file_start)
    {
        return Err(DocError::InvalidFormatting(format!(
            "{structure} FKP ranges overlap"
        )));
    }
    Ok(result)
}

fn parse_bin_table(
    data: &[u8],
    limits: DocLimits,
    structure: &'static str,
) -> Result<Vec<BinTableEntry>> {
    if data.len() < 4 || !(data.len() - 4).is_multiple_of(8) {
        return Err(DocError::InvalidFormatting(format!(
            "{structure} length {} is not 4 + 8*n",
            data.len()
        )));
    }
    let count = (data.len() - 4) / 8;
    if count > limits.max_formatting_pages {
        return Err(DocError::ResourceLimit {
            resource: "formatting-page",
            actual: u64::try_from(count).unwrap_or(u64::MAX),
            limit: u64::try_from(limits.max_formatting_pages).unwrap_or(u64::MAX),
        });
    }
    let boundary_bytes = (count + 1) * 4;
    let mut boundaries = Vec::with_capacity(count + 1);
    for chunk in data[..boundary_bytes].chunks_exact(4) {
        boundaries.push(u32::from_le_bytes(chunk.try_into().unwrap()));
    }
    if boundaries.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DocError::InvalidFormatting(format!(
            "{structure} FC boundaries are not strictly increasing"
        )));
    }
    let mut entries = Vec::with_capacity(count);
    for (index, chunk) in data[boundary_bytes..].chunks_exact(4).enumerate() {
        let page = u32::from_le_bytes(chunk.try_into().unwrap()) & PAGE_NUMBER_MASK;
        entries.push(BinTableEntry {
            file_start: boundaries[index],
            file_end: boundaries[index + 1],
            page_number: page,
        });
    }
    Ok(entries)
}

fn parse_chpx_fkp(page: &[u8]) -> Result<Vec<FormattingRun>> {
    ensure_fkp_size(page, "ChpxFkp")?;
    let count = usize::from(page[FKP_SIZE - 1]);
    if !(1..=MAX_CHPX_RUNS_PER_FKP).contains(&count) {
        return Err(DocError::InvalidFormatting(format!(
            "ChpxFkp run count {count} is outside 1..={MAX_CHPX_RUNS_PER_FKP}"
        )));
    }
    let boundaries = parse_fkp_boundaries(page, count, "ChpxFkp")?;
    let offsets_start = (count + 1) * 4;
    let properties_start = offsets_start + count;
    let mut runs = Vec::with_capacity(count);
    for index in 0..count {
        let offset = usize::from(page[offsets_start + index]) * 2;
        let grpprl = if offset == 0 {
            Vec::new()
        } else {
            if !(properties_start..FKP_SIZE - 1).contains(&offset) {
                return Err(DocError::InvalidFormatting(format!(
                    "ChpxFkp run {index} property offset {offset} is outside property area"
                )));
            }
            let length = usize::from(page[offset]);
            let start = offset + 1;
            let end = start.checked_add(length).ok_or_else(|| {
                DocError::InvalidFormatting("ChpxFkp property length overflow".into())
            })?;
            if end > FKP_SIZE - 1 {
                return Err(DocError::InvalidFormatting(format!(
                    "ChpxFkp run {index} properties end at {end}"
                )));
            }
            page[start..end].to_vec()
        };
        runs.push(FormattingRun {
            file_start: boundaries[index],
            file_end: boundaries[index + 1],
            paragraph_style: None,
            grpprl,
        });
    }
    Ok(runs)
}

fn parse_papx_fkp(page: &[u8]) -> Result<Vec<FormattingRun>> {
    ensure_fkp_size(page, "PapxFkp")?;
    let count = usize::from(page[FKP_SIZE - 1]);
    if !(1..=MAX_PAPX_RUNS_PER_FKP).contains(&count) {
        return Err(DocError::InvalidFormatting(format!(
            "PapxFkp run count {count} is outside 1..={MAX_PAPX_RUNS_PER_FKP}"
        )));
    }
    let boundaries = parse_fkp_boundaries(page, count, "PapxFkp")?;
    let bx_start = (count + 1) * 4;
    let properties_start = bx_start + count * BX_PAP_SIZE;
    let mut runs = Vec::with_capacity(count);
    for index in 0..count {
        let offset = usize::from(page[bx_start + index * BX_PAP_SIZE]) * 2;
        let (paragraph_style, grpprl) = if offset == 0 {
            (None, Vec::new())
        } else {
            if !(properties_start..FKP_SIZE - 1).contains(&offset) {
                return Err(DocError::InvalidFormatting(format!(
                    "PapxFkp run {index} property offset {offset} is outside property area"
                )));
            }
            parse_papx_in_fkp(page, offset, index)?
        };
        runs.push(FormattingRun {
            file_start: boundaries[index],
            file_end: boundaries[index + 1],
            paragraph_style,
            grpprl,
        });
    }
    Ok(runs)
}

fn parse_papx_in_fkp(page: &[u8], offset: usize, index: usize) -> Result<(Option<u16>, Vec<u8>)> {
    let cb = usize::from(page[offset]);
    let (start, length) = if cb == 0 {
        let cb_prime_offset = offset + 1;
        if cb_prime_offset >= FKP_SIZE - 1 {
            return Err(DocError::InvalidFormatting(format!(
                "PapxFkp run {index} has truncated extended length"
            )));
        }
        let cb_prime = usize::from(page[cb_prime_offset]);
        if cb_prime == 0 {
            return Err(DocError::InvalidFormatting(format!(
                "PapxFkp run {index} extended length is zero"
            )));
        }
        (offset + 2, cb_prime * 2)
    } else {
        (offset + 1, cb * 2 - 1)
    };
    let end = start
        .checked_add(length)
        .ok_or_else(|| DocError::InvalidFormatting("PapxInFkp property length overflow".into()))?;
    if length < 2 || end > FKP_SIZE - 1 {
        return Err(DocError::InvalidFormatting(format!(
            "PapxFkp run {index} GrpPrlAndIstd range [{start}, {end}) is invalid"
        )));
    }
    let style = u16::from_le_bytes([page[start], page[start + 1]]);
    Ok((Some(style), page[start + 2..end].to_vec()))
}

fn parse_fkp_boundaries(page: &[u8], count: usize, structure: &'static str) -> Result<Vec<u32>> {
    let byte_length = (count + 1) * 4;
    let mut boundaries = Vec::with_capacity(count + 1);
    for chunk in page[..byte_length].chunks_exact(4) {
        boundaries.push(u32::from_le_bytes(chunk.try_into().unwrap()));
    }
    if boundaries.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DocError::InvalidFormatting(format!(
            "{structure} FC boundaries are not strictly increasing"
        )));
    }
    Ok(boundaries)
}

fn ensure_fkp_size(page: &[u8], structure: &'static str) -> Result<()> {
    if page.len() == FKP_SIZE {
        Ok(())
    } else {
        Err(DocError::InvalidFormatting(format!(
            "{structure} is {} bytes instead of {FKP_SIZE}",
            page.len()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_character_fkp_and_rejects_bad_offsets() {
        let mut page = [0_u8; FKP_SIZE];
        page[..4].copy_from_slice(&100_u32.to_le_bytes());
        page[4..8].copy_from_slice(&120_u32.to_le_bytes());
        page[8] = 250;
        page[500] = 3;
        page[501..504].copy_from_slice(&[1, 2, 3]);
        page[511] = 1;
        let runs = parse_chpx_fkp(&page).unwrap();
        assert_eq!(runs[0].file_start, 100);
        assert_eq!(runs[0].file_end, 120);
        assert_eq!(runs[0].grpprl, [1, 2, 3]);

        page[8] = 1;
        assert!(matches!(
            parse_chpx_fkp(&page),
            Err(DocError::InvalidFormatting(_))
        ));
    }

    #[test]
    fn parses_normal_and_extended_papx_records() {
        let mut page = [0_u8; FKP_SIZE];
        for (index, boundary) in [10_u32, 20, 30].into_iter().enumerate() {
            page[index * 4..index * 4 + 4].copy_from_slice(&boundary.to_le_bytes());
        }
        let bx_start = 12;
        page[bx_start] = 200;
        page[bx_start + BX_PAP_SIZE] = 220;
        page[400] = 3;
        page[401..406].copy_from_slice(&[7, 0, 1, 2, 3]);
        page[440] = 0;
        page[441] = 2;
        page[442..446].copy_from_slice(&[9, 0, 4, 5]);
        page[511] = 2;

        let runs = parse_papx_fkp(&page).unwrap();
        assert_eq!(runs[0].paragraph_style, Some(7));
        assert_eq!(runs[0].grpprl, [1, 2, 3]);
        assert_eq!(runs[1].paragraph_style, Some(9));
        assert_eq!(runs[1].grpprl, [4, 5]);
    }

    #[test]
    fn parses_bte_and_enforces_page_budget() {
        let mut plc = Vec::new();
        plc.extend_from_slice(&100_u32.to_le_bytes());
        plc.extend_from_slice(&200_u32.to_le_bytes());
        plc.extend_from_slice(&5_u32.to_le_bytes());
        let entries = parse_bin_table(&plc, DocLimits::default(), "PlcBteChpx").unwrap();
        assert_eq!(entries[0].page_number, 5);

        let limits = DocLimits {
            max_formatting_pages: 0,
            ..DocLimits::default()
        };
        assert!(matches!(
            parse_bin_table(&plc, limits, "PlcBteChpx"),
            Err(DocError::ResourceLimit {
                resource: "formatting-page",
                ..
            })
        ));
    }

    #[test]
    fn accepts_one_zero_papx_alignment_byte_without_weakening_other_grpprls() {
        let run = LogicalFormattingRun {
            cp_start: 10,
            cp_end: 11,
            file_start: 20,
            file_end: 21,
            paragraph_style: Some(0),
            grpprl: vec![0x07, 0x24, 1, 0],
            piece_prm: 0,
        };
        assert_eq!(decode_formatting_run(&run, "PAPX").unwrap().len(), 1);
        assert!(decode_formatting_run(&run, "CHPX").is_err());

        let malformed = LogicalFormattingRun {
            grpprl: vec![0x07, 0x24, 1, 0xFF],
            ..run
        };
        assert!(decode_formatting_run(&malformed, "PAPX").is_err());
    }

    #[test]
    fn maps_compressed_and_utf16_fc_ranges_to_logical_cp_ranges() {
        let pieces = PieceTable::parse(
            &make_clx(&[0, 4, 7], &[(0x4000_0000 | 0x00C8, 3), (300, 5)]),
            7,
            DocLimits::default(),
        )
        .unwrap();
        let index = FormattingIndex {
            character_runs: vec![
                FormattingRun {
                    file_start: 100,
                    file_end: 103,
                    paragraph_style: None,
                    grpprl: vec![1],
                },
                FormattingRun {
                    file_start: 103,
                    file_end: 104,
                    paragraph_style: None,
                    grpprl: vec![2],
                },
                FormattingRun {
                    file_start: 300,
                    file_end: 306,
                    paragraph_style: None,
                    grpprl: vec![3],
                },
            ],
            paragraph_runs: Vec::new(),
        };
        let logical = index.to_logical(&pieces).unwrap();
        assert_eq!(
            logical
                .character_runs
                .iter()
                .map(|run| (run.cp_start, run.cp_end, run.piece_prm))
                .collect::<Vec<_>>(),
            [(0, 3, 3), (3, 4, 3), (4, 7, 5)]
        );
    }

    #[test]
    fn rejects_fkp_boundary_inside_utf16_unit() {
        let pieces =
            PieceTable::parse(&make_clx(&[0, 2], &[(100, 0)]), 2, DocLimits::default()).unwrap();
        let index = FormattingIndex {
            character_runs: vec![FormattingRun {
                file_start: 100,
                file_end: 103,
                paragraph_style: None,
                grpprl: Vec::new(),
            }],
            paragraph_runs: Vec::new(),
        };
        assert!(matches!(
            index.to_logical(&pieces),
            Err(DocError::InvalidFormatting(_))
        ));
    }

    #[test]
    fn applies_piece_prm_after_fkp_direct_formatting() {
        let pieces = PieceTable::parse(
            &make_clx(&[0, 2], &[(0x4000_0000 | 0x00C8, 0x01AA)]),
            2,
            DocLimits::default(),
        )
        .unwrap();
        let logical = LogicalFormattingIndex {
            character_runs: vec![LogicalFormattingRun {
                cp_start: 0,
                cp_end: 2,
                file_start: 100,
                file_end: 102,
                paragraph_style: None,
                grpprl: vec![0x35, 0x08, 0], // FKP says bold off
                piece_prm: 0x01AA,           // PCD says bold on
            }],
            paragraph_runs: Vec::new(),
        };
        let semantic = logical.resolve_properties(&pieces).unwrap();
        assert_eq!(
            semantic.character_runs[0].properties.bold,
            Some(crate::ToggleValue::On)
        );
        assert_eq!(
            semantic.character_runs[0]
                .sprms
                .iter()
                .map(|sprm| sprm.opcode)
                .collect::<Vec<_>>(),
            [0x0835, 0x0835]
        );
    }

    fn make_clx(cps: &[u32], pieces: &[(u32, u16)]) -> Vec<u8> {
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
}
