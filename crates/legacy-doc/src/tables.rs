//! Source-proven table row definitions and nested grid reconstruction.

use std::collections::BTreeMap;

use crate::{
    DocError, DocLimits, ParagraphPropertyDelta, PropertyGroup, Result, SemanticFormattingIndex,
    Sprm, Story, StoryKind, WordBinaryDocument,
};

const SPRM_T_DEF_TABLE: u16 = 0xD608;
const TC80_SIZE: usize = 20;

/// Horizontal merge role from `TCGRF.horzMerge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalMerge {
    None,
    Continuation,
    Start,
}

/// Vertical merge role from `TCGRF.vertMerge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalMerge {
    None,
    Continuation,
    Start,
    StartAndEnd,
}

/// Source formatting for one row cell definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellFormat {
    pub horizontal_merge: HorizontalMerge,
    pub vertical_merge: VerticalMerge,
    pub text_flow: u8,
    pub vertical_alignment: u8,
    pub width_unit: u8,
    pub preferred_width: u16,
    pub fit_text: bool,
    pub no_wrap: bool,
    pub hide_mark: bool,
    pub borders: [[u8; 4]; 4],
}

impl Default for CellFormat {
    fn default() -> Self {
        Self {
            horizontal_merge: HorizontalMerge::None,
            vertical_merge: VerticalMerge::None,
            text_flow: 0,
            vertical_alignment: 0,
            width_unit: 0,
            preferred_width: 0,
            fit_text: false,
            no_wrap: false,
            hide_mark: false,
            borders: [[0; 4]; 4],
        }
    }
}

/// Initial cell grid stored by one row's `sprmTDefTable`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRowDefinition {
    pub column_count: u8,
    /// Logical cell edges in twips, including the left and right outer edges.
    pub cell_edges_twips: Vec<i16>,
    pub cells: Vec<CellFormat>,
}

/// Default cell margins for a table row, in twips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellMargins {
    pub top: u16,
    pub left: u16,
    pub bottom: u16,
    pub right: u16,
}

impl Default for CellMargins {
    fn default() -> Self {
        Self {
            top: 0,
            left: 108,
            bottom: 0,
            right: 108,
        }
    }
}

/// Known row-level table properties. Unknown opcodes remain explicit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TablePropertyDelta {
    pub definition: Option<TableRowDefinition>,
    pub justification: Option<u16>,
    /// Half of the inter-cell gap in twips (`sprmTDxaGapHalf`).
    pub gap_half_twips: Option<u16>,
    /// Outer and inner table borders in top/left/bottom/right/insideH/insideV order.
    pub borders80: Option<[[u8; 4]; 6]>,
    pub default_cell_margins: CellMargins,
    /// Whether Word may resize columns to fit content (`sprmTFAutofit`).
    /// Absence means the MS-DOC default of fixed source column widths.
    pub autofit: Option<bool>,
    pub cant_split: Option<bool>,
    pub header: Option<bool>,
    /// Signed row height: positive means minimum, negative means exact.
    pub row_height_twips: Option<i16>,
    pub unsupported_opcodes: Vec<u16>,
}

/// One reconstructed cell, including its source cell mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCell {
    pub index: u8,
    pub cp_start: u32,
    /// Exclusive content end; excludes the cell mark.
    pub cp_content_end: u32,
    /// Exclusive source range; includes the cell mark.
    pub cp_end: u32,
    pub format: CellFormat,
}

/// One reconstructed table row ending in an explicit TTP mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow {
    pub cp_start: u32,
    pub cp_end: u32,
    pub depth: u32,
    pub properties: TablePropertyDelta,
    pub cells: Vec<TableCell>,
}

/// Consecutive rows at one table depth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub cp_start: u32,
    pub cp_end: u32,
    pub depth: u32,
    pub rows: Vec<TableRow>,
}

/// Reconstructed tables in main-story source order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableCollection {
    tables: Vec<Table>,
}

impl TableCollection {
    /// Reconstructs nested tables from direct paragraph depth/TTP properties,
    /// explicit cell marks, and each row's `sprmTDefTable`.
    ///
    /// # Errors
    ///
    /// Returns a typed error if markers and row definitions disagree, a known
    /// table operand is malformed, or a configured resource limit is exceeded.
    pub fn parse(
        story: &Story,
        formatting: &SemanticFormattingIndex,
        limits: DocLimits,
    ) -> Result<Self> {
        if story.kind != StoryKind::Main {
            return Err(DocError::InvalidTable(
                "table reconstruction currently requires the main story".into(),
            ));
        }
        let mut pending_starts = BTreeMap::<u32, u32>::new();
        let mut rows = Vec::new();
        let mut cell_count = 0_usize;
        for paragraph in &formatting.paragraph_runs {
            let start = paragraph.source.cp_start.max(story.cp_start);
            let end = paragraph.source.cp_end.min(story.cp_end);
            if start >= end {
                continue;
            }
            let depth = paragraph_depth(&paragraph.properties);
            if depth == 0 {
                if let Some((_, pending_start)) = pending_starts.first_key_value() {
                    return Err(DocError::InvalidTable(format!(
                        "table row beginning at CP {pending_start} reaches non-table paragraph at CP {start}"
                    )));
                }
                continue;
            }
            if let Some((&unfinished_depth, &unfinished_start)) =
                pending_starts.range((depth + 1)..).next()
            {
                return Err(DocError::InvalidTable(format!(
                    "nested row at depth {unfinished_depth}, CP {unfinished_start}, reaches depth {depth} before its TTP"
                )));
            }
            pending_starts.entry(depth).or_insert(start);
            let is_ttp = if depth == 1 {
                paragraph.properties.table_terminating_paragraph == Some(true)
            } else {
                paragraph.properties.inner_table_terminating_paragraph == Some(true)
            };
            if is_ttp {
                if rows.len() >= limits.max_table_rows {
                    return resource_limit("table-row", rows.len() + 1, limits.max_table_rows);
                }
                let row_start = pending_starts.remove(&depth).ok_or_else(|| {
                    DocError::InvalidTable(format!("TTP at CP {start} has no row start"))
                })?;
                let row = parse_row(story, formatting, paragraph, row_start, end, depth, limits)?;
                cell_count = cell_count.checked_add(row.cells.len()).ok_or_else(|| {
                    DocError::InvalidTable("total table cell count overflow".into())
                })?;
                if cell_count > limits.max_table_cells {
                    return resource_limit("table-cell", cell_count, limits.max_table_cells);
                }
                rows.push(row);
            }
        }
        if let Some((depth, start)) = pending_starts.first_key_value() {
            return Err(DocError::InvalidTable(format!(
                "table row at depth {depth}, beginning at CP {start}, has no TTP"
            )));
        }

        rows.sort_by_key(|row| (row.depth, row.cp_start));
        let mut tables: Vec<Table> = Vec::new();
        for row in rows {
            if let Some(table) = tables.last_mut()
                && table.depth == row.depth
                && table.cp_end == row.cp_start
            {
                table.cp_end = row.cp_end;
                table.rows.push(row);
                continue;
            }
            if tables.len() >= limits.max_tables {
                return resource_limit("table", tables.len() + 1, limits.max_tables);
            }
            tables.push(Table {
                cp_start: row.cp_start,
                cp_end: row.cp_end,
                depth: row.depth,
                rows: vec![row],
            });
        }
        tables.sort_by_key(|table| (table.cp_start, table.depth));
        Ok(Self { tables })
    }

    #[must_use]
    pub fn tables(&self) -> &[Table] {
        &self.tables
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.tables.len()
    }
}

impl WordBinaryDocument {
    /// Reconstructs source-proven nested main-story tables.
    ///
    /// # Errors
    ///
    /// Returns a typed formatting or table-structure error.
    pub fn tables(&self, limits: DocLimits) -> Result<TableCollection> {
        let story = self
            .story(StoryKind::Main)
            .ok_or_else(|| DocError::InvalidTable("main story is absent".into()))?;
        TableCollection::parse(story, &self.semantic_formatting(limits)?, limits)
    }
}

/// Applies table SPRMs in source order. Later scalar modifiers win.
///
/// # Errors
///
/// Returns a typed error for malformed known table operands.
pub fn apply_table_sprms(sprms: &[Sprm]) -> Result<TablePropertyDelta> {
    let mut result = TablePropertyDelta::default();
    for sprm in sprms {
        if sprm.group != PropertyGroup::Table {
            continue;
        }
        match sprm.opcode {
            0x5400 => result.justification = Some(table_word(sprm)?),
            0x9602 => result.gap_half_twips = Some(table_word(sprm)?),
            0x3403 => result.cant_split = Some(table_bool(sprm)?),
            0x3404 => result.header = Some(table_bool(sprm)?),
            0x3615 => result.autofit = Some(table_bool(sprm)?),
            0x9407 => result.row_height_twips = Some(table_signed_word(sprm)?),
            0xD605 => result.borders80 = Some(table_borders80(sprm)?),
            0xD634 => apply_default_cell_margins(sprm, &mut result.default_cell_margins)?,
            SPRM_T_DEF_TABLE => result.definition = Some(parse_definition(sprm)?),
            _ => result.unsupported_opcodes.push(sprm.opcode),
        }
    }
    Ok(result)
}

fn apply_default_cell_margins(sprm: &Sprm, margins: &mut CellMargins) -> Result<()> {
    if sprm.operand.len() != 7 || sprm.operand[0] != 6 {
        return Err(DocError::InvalidTable(format!(
            "sprmTCellPaddingDefault must contain a 6-byte CSSA, got {:?}",
            sprm.operand
        )));
    }
    if sprm.operand[1] != 0 || sprm.operand[2] != 1 {
        return Err(DocError::InvalidTable(format!(
            "sprmTCellPaddingDefault must target the whole row, got cells {}..{}",
            sprm.operand[1], sprm.operand[2]
        )));
    }
    let width = match sprm.operand[4] {
        0 => 0,
        3 => u16::from_le_bytes([sprm.operand[5], sprm.operand[6]]),
        value => {
            return Err(DocError::InvalidTable(format!(
                "sprmTCellPaddingDefault has unsupported width type 0x{value:02X}"
            )));
        }
    };
    if width > 31_680 {
        return Err(DocError::InvalidTable(format!(
            "sprmTCellPaddingDefault width {width} exceeds 31680"
        )));
    }
    let sides = sprm.operand[3];
    if sides & 0x01 != 0 {
        margins.top = width;
    }
    if sides & 0x02 != 0 {
        margins.left = width;
    }
    if sides & 0x04 != 0 {
        margins.bottom = width;
    }
    if sides & 0x08 != 0 {
        margins.right = width;
    }
    Ok(())
}

fn table_borders80(sprm: &Sprm) -> Result<[[u8; 4]; 6]> {
    if sprm.operand.len() != 25 || sprm.operand[0] != 24 {
        return Err(DocError::InvalidTable(format!(
            "sprmTTableBorders80 must contain a 24-byte payload, got {:?}",
            sprm.operand
        )));
    }
    let mut borders = [[0_u8; 4]; 6];
    for (border, raw) in borders.iter_mut().zip(sprm.operand[1..].chunks_exact(4)) {
        border.copy_from_slice(raw);
    }
    Ok(borders)
}

fn parse_row(
    story: &Story,
    formatting: &SemanticFormattingIndex,
    paragraph: &crate::SemanticParagraphRun,
    cp_start: u32,
    cp_end: u32,
    depth: u32,
    limits: DocLimits,
) -> Result<TableRow> {
    let properties = apply_table_sprms(&paragraph.sprms)?;
    let definition = properties.definition.as_ref().ok_or_else(|| {
        DocError::InvalidTable(format!("row [{cp_start}, {cp_end}) has no sprmTDefTable"))
    })?;
    let start = usize::try_from(cp_start - story.cp_start)
        .map_err(|_| DocError::InvalidTable("row start does not fit usize".into()))?;
    let end = usize::try_from(cp_end - story.cp_start)
        .map_err(|_| DocError::InvalidTable("row end does not fit usize".into()))?;
    let units = story.content.utf16.get(start..end).ok_or_else(|| {
        DocError::InvalidTable(format!("row [{cp_start}, {cp_end}) exceeds main story"))
    })?;
    let marks = row_marks(story, formatting, units, cp_start, cp_end, depth)?;
    let expected_marks = usize::from(definition.column_count) + 1;
    if marks.len() != expected_marks {
        return Err(DocError::InvalidTable(format!(
            "row [{cp_start}, {cp_end}) has {} cell/TTP marks; definition requires {expected_marks}",
            marks.len()
        )));
    }
    if marks.last().copied() != Some(units.len() - 1) {
        return Err(DocError::InvalidTable(format!(
            "row [{cp_start}, {cp_end}) TTP is not its final CP"
        )));
    }
    if usize::from(definition.column_count) > limits.max_table_cells {
        return resource_limit(
            "table-cell",
            usize::from(definition.column_count),
            limits.max_table_cells,
        );
    }
    let mut cells = Vec::with_capacity(usize::from(definition.column_count));
    let mut cell_start = cp_start;
    for (index, mark_offset) in marks
        .iter()
        .take(usize::from(definition.column_count))
        .enumerate()
    {
        let mark_cp =
            cp_start
                .checked_add(u32::try_from(*mark_offset).map_err(|_| {
                    DocError::InvalidTable("cell mark offset does not fit u32".into())
                })?)
                .ok_or_else(|| DocError::InvalidTable("cell mark CP overflow".into()))?;
        cells.push(TableCell {
            index: u8::try_from(index)
                .map_err(|_| DocError::InvalidTable("cell index exceeds u8".into()))?,
            cp_start: cell_start,
            cp_content_end: mark_cp,
            cp_end: mark_cp + 1,
            format: definition.cells[index].clone(),
        });
        cell_start = mark_cp + 1;
    }
    Ok(TableRow {
        cp_start,
        cp_end,
        depth,
        properties,
        cells,
    })
}

fn row_marks(
    story: &Story,
    formatting: &SemanticFormattingIndex,
    units: &[u16],
    cp_start: u32,
    cp_end: u32,
    depth: u32,
) -> Result<Vec<usize>> {
    if depth == 1 {
        return Ok(units
            .iter()
            .enumerate()
            .filter_map(|(offset, unit)| (*unit == 0x0007).then_some(offset))
            .collect());
    }
    let mut marks = Vec::new();
    for paragraph in &formatting.paragraph_runs {
        if paragraph.source.cp_start < cp_start || paragraph.source.cp_end > cp_end {
            continue;
        }
        if paragraph_depth(&paragraph.properties) != depth {
            continue;
        }
        let is_cell = paragraph.properties.inner_table_cell == Some(true);
        let is_ttp = paragraph.properties.inner_table_terminating_paragraph == Some(true);
        if !is_cell && !is_ttp {
            continue;
        }
        let mark_cp = paragraph.source.cp_end.checked_sub(1).ok_or_else(|| {
            DocError::InvalidTable("nested cell paragraph has an empty CP range".into())
        })?;
        let story_offset = usize::try_from(mark_cp - story.cp_start)
            .map_err(|_| DocError::InvalidTable("nested cell mark offset overflow".into()))?;
        if story.content.utf16.get(story_offset) != Some(&0x000D) {
            return Err(DocError::InvalidTable(format!(
                "nested cell/TTP at CP {mark_cp} is not a paragraph mark"
            )));
        }
        marks.push(
            usize::try_from(mark_cp - cp_start)
                .map_err(|_| DocError::InvalidTable("row-relative mark offset overflow".into()))?,
        );
    }
    marks.sort_unstable();
    Ok(marks)
}

fn parse_definition(sprm: &Sprm) -> Result<TableRowDefinition> {
    let operand = &sprm.operand;
    if operand.len() < 5 {
        return table_error("sprmTDefTable operand is shorter than 5 bytes");
    }
    let cb = usize::from(u16::from_le_bytes([operand[0], operand[1]]));
    if cb == 0 || operand.len() != cb + 1 {
        return table_error(&format!(
            "sprmTDefTable cb {cb} does not match operand length {}",
            operand.len()
        ));
    }
    let column_count = operand[2];
    if column_count > 63 {
        return table_error(&format!(
            "sprmTDefTable column count {column_count} exceeds 63"
        ));
    }
    let edge_count = usize::from(column_count) + 1;
    let edge_bytes = edge_count
        .checked_mul(2)
        .ok_or_else(|| DocError::InvalidTable("cell edge byte length overflow".into()))?;
    let formats_offset = 3_usize
        .checked_add(edge_bytes)
        .ok_or_else(|| DocError::InvalidTable("cell format offset overflow".into()))?;
    let edges = operand.get(3..formats_offset).ok_or_else(|| {
        DocError::InvalidTable("sprmTDefTable cell edge array is truncated".into())
    })?;
    let cell_edges_twips = edges
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    if cell_edges_twips.windows(2).any(|pair| pair[0] > pair[1]) {
        return table_error("sprmTDefTable cell edges are decreasing");
    }
    let formats = &operand[formats_offset..];
    if !formats.len().is_multiple_of(TC80_SIZE) {
        return table_error(&format!(
            "sprmTDefTable has {} trailing bytes not divisible by TC80 size",
            formats.len()
        ));
    }
    let mut cells = formats
        .chunks_exact(TC80_SIZE)
        .take(usize::from(column_count))
        .map(parse_tc80)
        .collect::<Result<Vec<_>>>()?;
    cells.resize(usize::from(column_count), CellFormat::default());
    Ok(TableRowDefinition {
        column_count,
        cell_edges_twips,
        cells,
    })
}

fn parse_tc80(data: &[u8]) -> Result<CellFormat> {
    let flags = u16::from_le_bytes([data[0], data[1]]);
    let horizontal_merge = match flags & 0x0003 {
        0 => HorizontalMerge::None,
        1 => HorizontalMerge::Continuation,
        2 | 3 => HorizontalMerge::Start,
        _ => unreachable!(),
    };
    let vertical_merge = match (flags >> 5) & 0x0003 {
        0 => VerticalMerge::None,
        1 => VerticalMerge::Continuation,
        2 => VerticalMerge::Start,
        3 => VerticalMerge::StartAndEnd,
        _ => unreachable!(),
    };
    Ok(CellFormat {
        horizontal_merge,
        vertical_merge,
        text_flow: u8::try_from((flags >> 2) & 0x0007)
            .map_err(|_| DocError::InvalidTable("text flow conversion failed".into()))?,
        vertical_alignment: u8::try_from((flags >> 7) & 0x0003)
            .map_err(|_| DocError::InvalidTable("vertical alignment conversion failed".into()))?,
        width_unit: u8::try_from((flags >> 9) & 0x0007)
            .map_err(|_| DocError::InvalidTable("width unit conversion failed".into()))?,
        preferred_width: u16::from_le_bytes([data[2], data[3]]),
        fit_text: flags & 0x1000 != 0,
        no_wrap: flags & 0x2000 != 0,
        hide_mark: flags & 0x4000 != 0,
        borders: [
            data[4..8].try_into().unwrap(),
            data[8..12].try_into().unwrap(),
            data[12..16].try_into().unwrap(),
            data[16..20].try_into().unwrap(),
        ],
    })
}

fn paragraph_depth(properties: &ParagraphPropertyDelta) -> u32 {
    properties.table_depth.map_or_else(
        || u32::from(properties.in_table == Some(true)),
        |depth| u32::try_from(depth).unwrap_or(0),
    )
}

fn table_word(sprm: &Sprm) -> Result<u16> {
    if sprm.operand.len() != 2 {
        return table_error(&format!(
            "SPRM 0x{:04X} has {} bytes; expected 2",
            sprm.opcode,
            sprm.operand.len()
        ));
    }
    Ok(u16::from_le_bytes([sprm.operand[0], sprm.operand[1]]))
}

fn table_signed_word(sprm: &Sprm) -> Result<i16> {
    Ok(i16::from_le_bytes(table_word(sprm)?.to_le_bytes()))
}

fn table_bool(sprm: &Sprm) -> Result<bool> {
    match sprm.operand.as_slice() {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => table_error(&format!("SPRM 0x{:04X} has invalid Bool8", sprm.opcode)),
    }
}

fn table_error<T>(message: &str) -> Result<T> {
    Err(DocError::InvalidTable(message.into()))
}

fn resource_limit<T>(resource: &'static str, actual: usize, limit: usize) -> Result<T> {
    Err(DocError::ResourceLimit {
        resource,
        actual: u64::try_from(actual).unwrap_or(u64::MAX),
        limit: u64::try_from(limit).unwrap_or(u64::MAX),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DecodedText, LogicalFormattingRun, ParagraphPropertyDelta, PiecePropertyModifier,
        SemanticParagraphRun, decode_grpprl,
    };

    #[test]
    fn decodes_tdef_table_grid_and_merge_flags() {
        let operand = tdef_operand(&[-120, 1000, 2200], &[(2, 1200), (1, 1200)]);
        let mut bytes = vec![0x08, 0xD6];
        bytes.extend_from_slice(&operand);
        let sprms = decode_grpprl(&bytes).unwrap();
        let properties = apply_table_sprms(&sprms).unwrap();
        let definition = properties.definition.unwrap();
        assert_eq!(definition.column_count, 2);
        assert_eq!(definition.cell_edges_twips, [-120, 1000, 2200]);
        assert_eq!(definition.cells[0].horizontal_merge, HorizontalMerge::Start);
        assert_eq!(
            definition.cells[1].horizontal_merge,
            HorizontalMerge::Continuation
        );
    }

    #[test]
    fn decodes_explicit_table_autofit_and_rejects_invalid_bool() {
        let enabled = decode_grpprl(&[0x15, 0x36, 1]).unwrap();
        assert_eq!(apply_table_sprms(&enabled).unwrap().autofit, Some(true));

        let invalid = decode_grpprl(&[0x15, 0x36, 2]).unwrap();
        assert!(matches!(
            apply_table_sprms(&invalid),
            Err(DocError::InvalidTable(_))
        ));
    }

    #[test]
    fn reconstructs_nested_rows_without_flattening_parent_cell_content() {
        let utf16 = vec![
            u16::from(b'X'),
            0x000D,
            u16::from(b'A'),
            0x000D,
            u16::from(b'B'),
            0x000D,
            0x000D,
            0x0007,
            0x0007,
        ];
        let story = Story {
            kind: StoryKind::Main,
            cp_start: 0,
            cp_end: 9,
            content: DecodedText {
                cp_start: 0,
                cp_end: 9,
                text: String::from_utf16_lossy(&utf16),
                utf16,
            },
        };
        let nested_definition = tdef_sprm(&[-100, 500, 1100], &[(0, 600), (0, 600)]);
        let outer_definition = tdef_sprm(&[-100, 1100], &[(0, 1200)]);
        let formatting = SemanticFormattingIndex {
            character_runs: Vec::new(),
            paragraph_runs: vec![
                paragraph(0, 2, 1, false, false, false, Vec::new()),
                paragraph(2, 4, 2, true, false, false, Vec::new()),
                paragraph(4, 6, 2, true, false, false, Vec::new()),
                paragraph(6, 7, 2, false, true, false, vec![nested_definition]),
                paragraph(7, 8, 1, false, false, false, Vec::new()),
                paragraph(8, 9, 1, false, false, true, vec![outer_definition]),
            ],
        };
        let tables = TableCollection::parse(&story, &formatting, DocLimits::default()).unwrap();
        assert_eq!(tables.len(), 2);
        assert_eq!(tables.tables()[0].depth, 1);
        assert_eq!(tables.tables()[0].rows[0].cells[0].cp_content_end, 7);
        assert_eq!(tables.tables()[1].depth, 2);
        assert_eq!(tables.tables()[1].rows[0].cells.len(), 2);
    }

    fn paragraph(
        cp_start: u32,
        cp_end: u32,
        depth: i32,
        inner_cell: bool,
        inner_ttp: bool,
        ttp: bool,
        sprms: Vec<Sprm>,
    ) -> SemanticParagraphRun {
        SemanticParagraphRun {
            source: LogicalFormattingRun {
                cp_start,
                cp_end,
                file_start: cp_start,
                file_end: cp_end,
                paragraph_style: Some(0),
                grpprl: Vec::new(),
                piece_prm: 0,
            },
            piece_modifier: PiecePropertyModifier::None,
            sprms,
            properties: ParagraphPropertyDelta {
                in_table: Some(true),
                table_depth: Some(depth),
                inner_table_cell: inner_cell.then_some(true),
                inner_table_terminating_paragraph: inner_ttp.then_some(true),
                table_terminating_paragraph: ttp.then_some(true),
                ..ParagraphPropertyDelta::default()
            },
        }
    }

    fn tdef_sprm(edges: &[i16], cells: &[(u16, u16)]) -> Sprm {
        let mut bytes = vec![0x08, 0xD6];
        bytes.extend_from_slice(&tdef_operand(edges, cells));
        decode_grpprl(&bytes).unwrap().remove(0)
    }

    fn tdef_operand(edges: &[i16], cells: &[(u16, u16)]) -> Vec<u8> {
        let mut remainder = vec![u8::try_from(cells.len()).unwrap()];
        for edge in edges {
            remainder.extend_from_slice(&edge.to_le_bytes());
        }
        for (flags, width) in cells {
            remainder.extend_from_slice(&flags.to_le_bytes());
            remainder.extend_from_slice(&width.to_le_bytes());
            remainder.extend_from_slice(&[0; 16]);
        }
        let cb = u16::try_from(remainder.len() + 1).unwrap();
        let mut operand = cb.to_le_bytes().to_vec();
        operand.extend_from_slice(&remainder);
        operand
    }
}
