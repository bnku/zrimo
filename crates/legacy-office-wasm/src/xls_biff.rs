//! Bounded extraction of visual BIFF8 workbook metadata.
//!
//! `office_oxide` remains responsible for values and cached formula results. This
//! module deliberately reads only source-backed formatting, worksheet geometry,
//! merged ranges and hyperlink metadata needed by the XLSX postprocessor.

use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;

use office_oxide::cfb::CfbReader;

const BOF: u16 = 0x0809;
const EOF: u16 = 0x000A;
const BOUNDSHEET: u16 = 0x0085;
const FONT: u16 = 0x0031;
const FORMAT: u16 = 0x041E;
const XF: u16 = 0x00E0;
const PALETTE: u16 = 0x0092;
const DIMENSIONS: u16 = 0x0200;
const DEFCOLWIDTH: u16 = 0x0055;
const STANDARDWIDTH: u16 = 0x0099;
const DEFAULTROWHEIGHT: u16 = 0x0225;
const COLINFO: u16 = 0x007D;
const ROW: u16 = 0x0208;
const MERGEDCELLS: u16 = 0x00E5;
const HLINK: u16 = 0x01B8;
const LABELSST: u16 = 0x00FD;
const LABEL: u16 = 0x0204;
const RSTRING: u16 = 0x00D6;
const NUMBER: u16 = 0x0203;
const RK: u16 = 0x027E;
const MULRK: u16 = 0x00BD;
const BOOLERR: u16 = 0x0205;
const FORMULA: u16 = 0x0006;
const BLANK: u16 = 0x0201;
const MULBLANK: u16 = 0x00BE;
const FILEPASS: u16 = 0x002F;

#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)]
pub(crate) struct XlsLimits {
    pub max_stream_bytes: usize,
    pub max_records: usize,
    pub max_sheets: usize,
    pub max_fonts: usize,
    pub max_xfs: usize,
    pub max_formats: usize,
    pub max_cells: usize,
    pub max_ranges: usize,
    pub max_hyperlinks: usize,
}

impl Default for XlsLimits {
    fn default() -> Self {
        Self {
            max_stream_bytes: 64 * 1024 * 1024,
            max_records: 500_000,
            max_sheets: 256,
            max_fonts: 1_024,
            max_xfs: 4_096,
            max_formats: 1_024,
            max_cells: 2_000_000,
            max_ranges: 65_536,
            max_hyperlinks: 65_536,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum XlsWarning {
    UnsupportedHyperlink {
        sheet: usize,
        row: u16,
        col: u16,
    },
    InvalidStyleReference {
        sheet: usize,
        row: u16,
        col: u16,
        xf: u16,
    },
    InvalidColumnStyleReference {
        sheet: usize,
        first_col: u16,
        last_col: u16,
        xf: u16,
    },
    InvalidRowStyleReference {
        sheet: usize,
        row: u16,
        xf: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct XlsFont {
    pub name: String,
    pub height_twips: u16,
    pub color_index: u16,
    pub weight: u16,
    pub italic: bool,
    pub strike: bool,
    pub underline: u8,
    pub family: u8,
    pub charset: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct XlsBorder {
    pub left_style: u8,
    pub right_style: u8,
    pub top_style: u8,
    pub bottom_style: u8,
    pub left_color: u8,
    pub right_color: u8,
    pub top_color: u8,
    pub bottom_color: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct XlsFill {
    pub pattern: u8,
    pub foreground: u8,
    pub background: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct XlsXf {
    pub font_index: u16,
    pub format_index: u16,
    pub parent_style: u16,
    pub is_style: bool,
    pub used_attributes: u8,
    pub horizontal: u8,
    pub vertical: u8,
    pub wrap: bool,
    pub rotation: u8,
    pub indent: u8,
    pub shrink_to_fit: bool,
    pub reading_order: u8,
    pub border: XlsBorder,
    pub fill: XlsFill,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ColumnBand {
    pub first: u16,
    pub last: u16,
    pub width_chars: f64,
    pub hidden: bool,
    pub xf: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RowLayout {
    pub height_twips: u16,
    pub hidden: bool,
    pub custom_height: bool,
    pub xf: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CellRange {
    pub first_row: u16,
    pub last_row: u16,
    pub first_col: u16,
    pub last_col: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HyperlinkTarget {
    External(String),
    Internal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct XlsHyperlink {
    pub range: CellRange,
    pub target: HyperlinkTarget,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SourceCellValue {
    Number(f64),
    Boolean(bool),
    Error(u8),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SheetFormatting {
    #[allow(dead_code)]
    pub name: String,
    pub dimensions: Option<CellRange>,
    pub default_col_width_chars: Option<f64>,
    pub default_row_height_twips: Option<u16>,
    pub columns: Vec<ColumnBand>,
    pub rows: BTreeMap<u16, RowLayout>,
    pub cell_xfs: HashMap<(u16, u16), u16>,
    pub source_values: HashMap<(u16, u16), SourceCellValue>,
    pub merges: Vec<CellRange>,
    pub hyperlinks: Vec<XlsHyperlink>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WorkbookFormatting {
    pub fonts: Vec<XlsFont>,
    pub xfs: Vec<XlsXf>,
    pub custom_formats: BTreeMap<u16, String>,
    pub palette: Vec<[u8; 3]>,
    pub sheets: Vec<SheetFormatting>,
    pub warnings: Vec<XlsWarning>,
}

impl WorkbookFormatting {
    pub(crate) fn from_xls(data: &[u8], limits: XlsLimits) -> Result<Self, String> {
        let mut cfb = CfbReader::new(Cursor::new(data))
            .map_err(|error| format!("XLS container parse failed: {error}"))?;
        let stream = if cfb.has_stream("Workbook") {
            cfb.open_stream("Workbook")
        } else if cfb.has_stream("Book") {
            cfb.open_stream("Book")
        } else {
            return Err("XLS is missing the Workbook/Book stream".into());
        }
        .map_err(|error| format!("XLS workbook stream read failed: {error}"))?;
        if stream.len() > limits.max_stream_bytes {
            return Err(format!(
                "XLS Workbook stream exceeds limit: {} > {} bytes",
                stream.len(),
                limits.max_stream_bytes
            ));
        }
        Self::parse_stream(&stream, limits)
    }

    fn parse_stream(data: &[u8], limits: XlsLimits) -> Result<Self, String> {
        let records = records(data, limits.max_records)?;
        let first = records.first().ok_or("XLS Workbook stream is empty")?;
        if first.kind != BOF || first.data.len() < 4 || le_u16(first.data, 0)? != 0x0600 {
            return Err("only BIFF8 XLS workbooks are supported for formatting fidelity".into());
        }

        let mut workbook = Self::default();
        let mut sheet_infos = Vec::new();
        let mut index = 0;
        while index < records.len() {
            let record = records[index];
            index += 1;
            match record.kind {
                EOF => break,
                FILEPASS => return Err("encrypted XLS formatting is not supported".into()),
                BOUNDSHEET => {
                    if sheet_infos.len() >= limits.max_sheets {
                        return Err("XLS sheet limit exceeded".into());
                    }
                    sheet_infos.push(parse_boundsheet(record.data)?);
                }
                FONT => {
                    enforce_push(workbook.fonts.len(), limits.max_fonts, "font")?;
                    workbook.fonts.push(parse_font(record.data)?);
                }
                FORMAT => {
                    enforce_push(workbook.custom_formats.len(), limits.max_formats, "format")?;
                    let (format_id, code) = parse_format(record.data)?;
                    workbook.custom_formats.insert(format_id, code);
                }
                XF => {
                    enforce_push(workbook.xfs.len(), limits.max_xfs, "XF")?;
                    workbook.xfs.push(parse_xf(record.data)?);
                }
                PALETTE => workbook.palette = parse_palette(record.data)?,
                _ => {}
            }
        }

        for (sheet_index, info) in sheet_infos
            .into_iter()
            .filter(|info| info.is_worksheet)
            .enumerate()
        {
            let start = records
                .iter()
                .position(|record| record.offset == info.offset as usize)
                .ok_or_else(|| format!("worksheet '{}' BOF offset is invalid", info.name))?;
            let sheet = parse_sheet(
                &records[start..],
                info.name,
                &workbook.xfs,
                limits,
                sheet_index,
                &mut workbook.warnings,
            )?;
            workbook.sheets.push(sheet);
        }
        Ok(workbook)
    }
}

#[derive(Clone, Copy)]
struct Record<'a> {
    kind: u16,
    data: &'a [u8],
    offset: usize,
}

fn records(data: &[u8], limit: usize) -> Result<Vec<Record<'_>>, String> {
    let mut result = Vec::new();
    let mut offset = 0usize;
    while offset < data.len() {
        if data.len() - offset < 4 && data[offset..].iter().all(|byte| *byte == 0) {
            break;
        }
        if result.len() >= limit {
            return Err("XLS record limit exceeded".into());
        }
        let header_end = offset.checked_add(4).ok_or("XLS record header overflow")?;
        let header = data
            .get(offset..header_end)
            .ok_or("truncated XLS record header")?;
        let kind = u16::from_le_bytes([header[0], header[1]]);
        let length = usize::from(u16::from_le_bytes([header[2], header[3]]));
        let end = header_end
            .checked_add(length)
            .ok_or("XLS record length overflow")?;
        let payload = data
            .get(header_end..end)
            .ok_or_else(|| format!("truncated XLS record 0x{kind:04X} at byte {offset}"))?;
        result.push(Record {
            kind,
            data: payload,
            offset,
        });
        offset = end;
    }
    Ok(result)
}

#[derive(Debug)]
struct SheetInfo {
    offset: u32,
    name: String,
    is_worksheet: bool,
}

fn parse_boundsheet(data: &[u8]) -> Result<SheetInfo, String> {
    if data.len() < 8 {
        return Err("truncated BOUNDSHEET record".into());
    }
    let offset = le_u32(data, 0)?;
    let name_len = usize::from(data[6]);
    let options = data[7];
    let name = parse_xl_chars(data, 8, name_len, options & 1 != 0)?;
    Ok(SheetInfo {
        offset,
        name,
        is_worksheet: data[5] == 0,
    })
}

fn parse_font(data: &[u8]) -> Result<XlsFont, String> {
    if data.len() < 16 {
        return Err("truncated FONT record".into());
    }
    let options = le_u16(data, 2)?;
    let name_len = usize::from(data[14]);
    let unicode = data[15] & 1 != 0;
    Ok(XlsFont {
        name: parse_xl_chars(data, 16, name_len, unicode)?,
        height_twips: le_u16(data, 0)?,
        color_index: le_u16(data, 4)?,
        weight: le_u16(data, 6)?,
        italic: options & 0x0002 != 0,
        strike: options & 0x0008 != 0,
        underline: data[10],
        family: data[11],
        charset: data[12],
    })
}

fn parse_format(data: &[u8]) -> Result<(u16, String), String> {
    if data.len() < 5 {
        return Err("truncated FORMAT record".into());
    }
    let format_id = le_u16(data, 0)?;
    let char_count = usize::from(le_u16(data, 2)?);
    let options = data[4];
    let rich_bytes = if options & 0x08 != 0 { 2 } else { 0 };
    let extension_bytes = if options & 0x04 != 0 { 4 } else { 0 };
    let chars_offset = 5_usize
        .checked_add(rich_bytes)
        .and_then(|offset| offset.checked_add(extension_bytes))
        .ok_or("FORMAT string offset overflow")?;
    let value = parse_xl_chars(data, chars_offset, char_count, options & 1 != 0)?;
    Ok((format_id, value))
}

fn parse_palette(data: &[u8]) -> Result<Vec<[u8; 3]>, String> {
    let count = usize::from(le_u16(data, 0)?);
    if count > 56 {
        return Err(format!("invalid XLS palette size {count}"));
    }
    let bytes = count
        .checked_mul(4)
        .and_then(|n| n.checked_add(2))
        .ok_or("palette size overflow")?;
    if data.len() < bytes {
        return Err("truncated PALETTE record".into());
    }
    Ok((0..count)
        .map(|index| {
            let offset = 2 + index * 4;
            [data[offset], data[offset + 1], data[offset + 2]]
        })
        .collect())
}

fn parse_xf(data: &[u8]) -> Result<XlsXf, String> {
    if data.len() != 20 {
        return Err(format!(
            "invalid XF record length {}; expected 20",
            data.len()
        ));
    }
    let protection = le_u16(data, 4)?;
    let align = data[6];
    let misc = data[8];
    let border1 = le_u32(data, 10)?;
    let border2 = le_u32(data, 14)?;
    let colors = le_u16(data, 18)?;
    Ok(XlsXf {
        font_index: le_u16(data, 0)?,
        format_index: le_u16(data, 2)?,
        parent_style: protection >> 4,
        is_style: protection & 0x0004 != 0,
        used_attributes: data[9],
        horizontal: align & 0x07,
        wrap: align & 0x08 != 0,
        vertical: (align >> 4) & 0x07,
        rotation: data[7],
        indent: misc & 0x0F,
        shrink_to_fit: misc & 0x10 != 0,
        reading_order: (misc >> 6) & 0x03,
        border: XlsBorder {
            left_style: (border1 & 0x0F) as u8,
            right_style: ((border1 >> 4) & 0x0F) as u8,
            top_style: ((border1 >> 8) & 0x0F) as u8,
            bottom_style: ((border1 >> 12) & 0x0F) as u8,
            left_color: ((border1 >> 16) & 0x7F) as u8,
            right_color: ((border1 >> 23) & 0x7F) as u8,
            top_color: (border2 & 0x7F) as u8,
            bottom_color: ((border2 >> 7) & 0x7F) as u8,
        },
        fill: XlsFill {
            pattern: ((border2 >> 26) & 0x3F) as u8,
            foreground: (colors & 0x7F) as u8,
            background: ((colors >> 7) & 0x7F) as u8,
        },
    })
}

fn parse_sheet(
    records: &[Record<'_>],
    name: String,
    xfs: &[XlsXf],
    limits: XlsLimits,
    sheet_index: usize,
    warnings: &mut Vec<XlsWarning>,
) -> Result<SheetFormatting, String> {
    let mut sheet = SheetFormatting {
        name,
        ..SheetFormatting::default()
    };
    let mut started = false;
    for record in records {
        if record.kind == BOF {
            if started {
                return Err("nested worksheet BOF".into());
            }
            started = true;
            continue;
        }
        if !started {
            continue;
        }
        if record.kind == EOF {
            break;
        }
        match record.kind {
            DIMENSIONS => sheet.dimensions = parse_dimensions(record.data)?,
            DEFCOLWIDTH => {
                sheet.default_col_width_chars = Some(f64::from(le_u16(record.data, 0)?));
            }
            STANDARDWIDTH => {
                sheet.default_col_width_chars = Some(f64::from(le_u16(record.data, 0)?) / 256.0);
            }
            DEFAULTROWHEIGHT => {
                if record.data.len() < 4 {
                    return Err("truncated DEFAULTROWHEIGHT record".into());
                }
                sheet.default_row_height_twips = Some(le_u16(record.data, 2)? & 0x7FFF);
            }
            COLINFO => {
                add_column_layout(record.data, &mut sheet, xfs, limits, sheet_index, warnings)?;
            }
            ROW => {
                add_row_layout(record.data, &mut sheet, xfs, sheet_index, warnings)?;
            }
            MERGEDCELLS => parse_merges(record.data, &mut sheet.merges, limits.max_ranges)?,
            HLINK => {
                if sheet.hyperlinks.len() >= limits.max_hyperlinks {
                    return Err("XLS hyperlink limit exceeded".into());
                }
                if let Ok(link) = parse_hlink(record.data) {
                    sheet.hyperlinks.push(link);
                } else {
                    let range = parse_range(record.data).unwrap_or(CellRange {
                        first_row: 0,
                        last_row: 0,
                        first_col: 0,
                        last_col: 0,
                    });
                    warnings.push(XlsWarning::UnsupportedHyperlink {
                        sheet: sheet_index,
                        row: range.first_row,
                        col: range.first_col,
                    });
                }
            }
            LABELSST | LABEL | RSTRING | BLANK => {
                add_cell_xf(record.data, &mut sheet, xfs, limits, sheet_index, warnings)?;
            }
            NUMBER | RK | BOOLERR | FORMULA => {
                add_cell_xf(record.data, &mut sheet, xfs, limits, sheet_index, warnings)?;
                if let Some(value) = parse_source_value(record.kind, record.data)? {
                    sheet
                        .source_values
                        .insert((le_u16(record.data, 0)?, le_u16(record.data, 2)?), value);
                }
            }
            MULRK => add_mulrk_xfs(record.data, &mut sheet, xfs, limits, sheet_index, warnings)?,
            MULBLANK => {
                add_mulblank_xfs(record.data, &mut sheet, xfs, limits, sheet_index, warnings)?;
            }
            _ => {}
        }
    }
    if let Some(dimensions) = sheet.dimensions {
        sheet.columns.retain_mut(|column| {
            if column.first > dimensions.last_col || column.last < dimensions.first_col {
                return false;
            }
            column.first = column.first.max(dimensions.first_col);
            column.last = column.last.min(dimensions.last_col);
            true
        });
    }
    Ok(sheet)
}

fn add_column_layout(
    data: &[u8],
    sheet: &mut SheetFormatting,
    xfs: &[XlsXf],
    limits: XlsLimits,
    sheet_index: usize,
    warnings: &mut Vec<XlsWarning>,
) -> Result<(), String> {
    enforce_push(sheet.columns.len(), limits.max_ranges, "column band")?;
    let Some(mut column) = parse_colinfo(data)? else {
        return Ok(());
    };
    if usize::from(column.xf) >= xfs.len() {
        warnings.push(XlsWarning::InvalidColumnStyleReference {
            sheet: sheet_index,
            first_col: column.first,
            last_col: column.last,
            xf: column.xf,
        });
        column.xf = 0;
    }
    sheet.columns.push(column);
    Ok(())
}

fn add_row_layout(
    data: &[u8],
    sheet: &mut SheetFormatting,
    xfs: &[XlsXf],
    sheet_index: usize,
    warnings: &mut Vec<XlsWarning>,
) -> Result<(), String> {
    let (row, mut layout) = parse_row(data)?;
    if let Some(xf) = layout.xf
        && usize::from(xf) >= xfs.len()
    {
        warnings.push(XlsWarning::InvalidRowStyleReference {
            sheet: sheet_index,
            row,
            xf,
        });
        layout.xf = None;
    }
    sheet.rows.insert(row, layout);
    Ok(())
}

fn parse_dimensions(data: &[u8]) -> Result<Option<CellRange>, String> {
    if data.len() < 14 {
        return Err("truncated DIMENSIONS record".into());
    }
    let first_row = le_u32(data, 0)?;
    let row_after = le_u32(data, 4)?;
    let first_col = le_u16(data, 8)?;
    let col_after = le_u16(data, 10)?;
    if row_after <= first_row || col_after <= first_col {
        return Ok(None);
    }
    if row_after > 65_536 || col_after > 256 {
        return Err("DIMENSIONS exceeds BIFF8 grid".into());
    }
    Ok(Some(CellRange {
        first_row: u16::try_from(first_row).map_err(|_| "DIMENSIONS first row overflow")?,
        last_row: u16::try_from(row_after - 1).map_err(|_| "DIMENSIONS last row overflow")?,
        first_col,
        last_col: col_after - 1,
    }))
}

fn parse_colinfo(data: &[u8]) -> Result<Option<ColumnBand>, String> {
    if data.len() < 12 {
        return Err("truncated COLINFO record".into());
    }
    let first = le_u16(data, 0)?;
    let last = le_u16(data, 2)?;
    if first > last {
        return Err("invalid COLINFO range".into());
    }
    // Some producers use 0x0100 as an exclusive/sentinel upper bound. It
    // must never materialise an OOXML column 257 or expand the used range.
    if first >= 256 {
        return Ok(None);
    }
    let options = le_u16(data, 8)?;
    Ok(Some(ColumnBand {
        first,
        last: last.min(255),
        width_chars: f64::from(le_u16(data, 4)?) / 256.0,
        hidden: options & 1 != 0,
        xf: le_u16(data, 6)?,
    }))
}

fn parse_row(data: &[u8]) -> Result<(u16, RowLayout), String> {
    if data.len() < 16 {
        return Err("truncated ROW record".into());
    }
    let row = le_u16(data, 0)?;
    let raw_height = le_u16(data, 6)?;
    let options = le_u32(data, 12)?;
    let formatted = options & 0x0080 != 0;
    Ok((
        row,
        RowLayout {
            height_twips: raw_height & 0x7FFF,
            hidden: options & 0x0020 != 0,
            custom_height: raw_height & 0x8000 != 0 || options & 0x0040 != 0,
            xf: formatted.then_some(((options >> 16) & 0x0FFF) as u16),
        },
    ))
}

fn parse_merges(data: &[u8], target: &mut Vec<CellRange>, limit: usize) -> Result<(), String> {
    let count = usize::from(le_u16(data, 0)?);
    if target
        .len()
        .checked_add(count)
        .ok_or("merge count overflow")?
        > limit
    {
        return Err("XLS merged range limit exceeded".into());
    }
    let expected = count
        .checked_mul(8)
        .and_then(|n| n.checked_add(2))
        .ok_or("merge size overflow")?;
    if data.len() < expected {
        return Err("truncated MERGEDCELLS record".into());
    }
    for index in 0..count {
        let range =
            parse_range(&data[2 + index * 8..2 + index * 8 + 8]).ok_or("invalid merged range")?;
        validate_range(range)?;
        target.push(range);
    }
    Ok(())
}

fn add_cell_xf(
    data: &[u8],
    sheet: &mut SheetFormatting,
    xfs: &[XlsXf],
    limits: XlsLimits,
    sheet_index: usize,
    warnings: &mut Vec<XlsWarning>,
) -> Result<(), String> {
    if data.len() < 6 {
        return Err("truncated XLS cell record".into());
    }
    add_xf(
        le_u16(data, 0)?,
        le_u16(data, 2)?,
        le_u16(data, 4)?,
        sheet,
        xfs,
        limits,
        sheet_index,
        warnings,
    )
}

fn add_mulrk_xfs(
    data: &[u8],
    sheet: &mut SheetFormatting,
    xfs: &[XlsXf],
    limits: XlsLimits,
    sheet_index: usize,
    warnings: &mut Vec<XlsWarning>,
) -> Result<(), String> {
    if data.len() < 12 || !(data.len() - 6).is_multiple_of(6) {
        return Err("invalid MULRK record".into());
    }
    let row = le_u16(data, 0)?;
    let first_col = le_u16(data, 2)?;
    let count = (data.len() - 6) / 6;
    let last_col = le_u16(data, data.len() - 2)?;
    if usize::from(last_col)
        .checked_sub(usize::from(first_col))
        .and_then(|n| n.checked_add(1))
        != Some(count)
    {
        return Err("MULRK column count mismatch".into());
    }
    for index in 0..count {
        let col = first_col
            .checked_add(u16::try_from(index).map_err(|_| "MULRK column overflow")?)
            .ok_or("MULRK column overflow")?;
        add_xf(
            row,
            col,
            le_u16(data, 4 + index * 6)?,
            sheet,
            xfs,
            limits,
            sheet_index,
            warnings,
        )?;
        sheet.source_values.insert(
            (row, col),
            SourceCellValue::Number(decode_rk(le_u32(data, 6 + index * 6)?)),
        );
    }
    Ok(())
}

fn parse_source_value(kind: u16, data: &[u8]) -> Result<Option<SourceCellValue>, String> {
    if data.len() < 8 {
        return Err("truncated numeric XLS cell record".into());
    }
    match kind {
        NUMBER => {
            if data.len() < 14 {
                return Err("truncated NUMBER record".into());
            }
            let bytes: [u8; 8] = data[6..14].try_into().map_err(|_| "invalid NUMBER")?;
            Ok(Some(SourceCellValue::Number(f64::from_le_bytes(bytes))))
        }
        RK => {
            if data.len() < 10 {
                return Err("truncated RK record".into());
            }
            Ok(Some(SourceCellValue::Number(decode_rk(le_u32(data, 6)?))))
        }
        BOOLERR => Ok(Some(if data[7] == 0 {
            SourceCellValue::Boolean(data[6] != 0)
        } else {
            SourceCellValue::Error(data[6])
        })),
        FORMULA => {
            if data.len() < 14 {
                return Err("truncated FORMULA record".into());
            }
            let result = &data[6..14];
            if result[6] == 0xFF && result[7] == 0xFF {
                Ok(match result[0] {
                    1 => Some(SourceCellValue::Boolean(result[2] != 0)),
                    2 => Some(SourceCellValue::Error(result[2])),
                    _ => None,
                })
            } else {
                let bytes: [u8; 8] = result.try_into().map_err(|_| "invalid FORMULA result")?;
                Ok(Some(SourceCellValue::Number(f64::from_le_bytes(bytes))))
            }
        }
        _ => Ok(None),
    }
}

fn decode_rk(raw: u32) -> f64 {
    let mut value = if raw & 0x02 != 0 {
        f64::from(raw.cast_signed() >> 2)
    } else {
        f64::from_bits(u64::from(raw & 0xFFFF_FFFC) << 32)
    };
    if raw & 0x01 != 0 {
        value /= 100.0;
    }
    value
}

fn add_mulblank_xfs(
    data: &[u8],
    sheet: &mut SheetFormatting,
    xfs: &[XlsXf],
    limits: XlsLimits,
    sheet_index: usize,
    warnings: &mut Vec<XlsWarning>,
) -> Result<(), String> {
    if data.len() < 8 || !(data.len() - 6).is_multiple_of(2) {
        return Err("invalid MULBLANK record".into());
    }
    let row = le_u16(data, 0)?;
    let first_col = le_u16(data, 2)?;
    let count = (data.len() - 6) / 2;
    let last_col = le_u16(data, data.len() - 2)?;
    if usize::from(last_col)
        .checked_sub(usize::from(first_col))
        .and_then(|n| n.checked_add(1))
        != Some(count)
    {
        return Err("MULBLANK column count mismatch".into());
    }
    for index in 0..count {
        let col = first_col
            .checked_add(u16::try_from(index).map_err(|_| "MULBLANK column overflow")?)
            .ok_or("MULBLANK column overflow")?;
        add_xf(
            row,
            col,
            le_u16(data, 4 + index * 2)?,
            sheet,
            xfs,
            limits,
            sheet_index,
            warnings,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn add_xf(
    row: u16,
    col: u16,
    xf: u16,
    sheet: &mut SheetFormatting,
    xfs: &[XlsXf],
    limits: XlsLimits,
    sheet_index: usize,
    warnings: &mut Vec<XlsWarning>,
) -> Result<(), String> {
    if col >= 256 {
        return Err("cell column exceeds BIFF8 grid".into());
    }
    if sheet.cell_xfs.len() >= limits.max_cells {
        return Err("XLS styled-cell limit exceeded".into());
    }
    if usize::from(xf) >= xfs.len() {
        warnings.push(XlsWarning::InvalidStyleReference {
            sheet: sheet_index,
            row,
            col,
            xf,
        });
        return Ok(());
    }
    sheet.cell_xfs.insert((row, col), xf);
    Ok(())
}

fn parse_hlink(data: &[u8]) -> Result<XlsHyperlink, ()> {
    let range = parse_range(data).ok_or(())?;
    validate_range(range).map_err(|_| ())?;
    // BIFF8 HLINK begins with Ref8, a fixed hyperlink GUID and stream version.
    // Common files then store flags followed by optional UTF-16 display/location
    // strings and a URL moniker. We intentionally accept only targets that can be
    // extracted unambiguously and leave all other monikers as typed degradation.
    if data.len() < 36 {
        return Err(());
    }
    let utf16 = &data[8..];
    let decoded: Vec<u16> = utf16
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let text = String::from_utf16_lossy(&decoded);
    let lower = text.to_ascii_lowercase();
    for scheme in ["https://", "http://", "mailto:", "tel:"] {
        if let Some(start) = lower.find(scheme) {
            let target = text[start..]
                .split('\0')
                .next()
                .unwrap_or_default()
                .trim_matches(|ch: char| ch.is_control());
            if !target.is_empty() {
                return Ok(XlsHyperlink {
                    range,
                    target: HyperlinkTarget::External(target.to_string()),
                });
            }
        }
    }
    for candidate in text.split('\0').filter(|part| !part.is_empty()) {
        let trimmed = candidate.trim_matches(|ch: char| ch.is_control());
        if let Some(location) = trimmed.strip_prefix('#') {
            return Ok(XlsHyperlink {
                range,
                target: HyperlinkTarget::Internal(location.to_string()),
            });
        }
    }
    Err(())
}

fn parse_range(data: &[u8]) -> Option<CellRange> {
    if data.len() < 8 {
        return None;
    }
    Some(CellRange {
        first_row: u16::from_le_bytes([data[0], data[1]]),
        last_row: u16::from_le_bytes([data[2], data[3]]),
        first_col: u16::from_le_bytes([data[4], data[5]]),
        last_col: u16::from_le_bytes([data[6], data[7]]),
    })
}

fn validate_range(range: CellRange) -> Result<(), String> {
    if range.first_row > range.last_row || range.first_col > range.last_col || range.last_col >= 256
    {
        return Err("range exceeds BIFF8 grid".into());
    }
    Ok(())
}

fn parse_xl_chars(
    data: &[u8],
    offset: usize,
    count: usize,
    unicode: bool,
) -> Result<String, String> {
    let width = if unicode { 2 } else { 1 };
    let byte_count = count
        .checked_mul(width)
        .ok_or("XLS string length overflow")?;
    let end = offset
        .checked_add(byte_count)
        .ok_or("XLS string end overflow")?;
    let bytes = data.get(offset..end).ok_or("truncated XLS string")?;
    if unicode {
        let units = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&units).map_err(|_| "invalid UTF-16 in XLS string".into())
    } else {
        // BIFF compressed Unicode stores the low byte of each UTF-16 code unit,
        // not an arbitrary workbook-codepage byte sequence.
        Ok(bytes.iter().map(|byte| char::from(*byte)).collect())
    }
}

fn le_u16(data: &[u8], offset: usize) -> Result<u16, String> {
    let bytes = data
        .get(offset..offset.checked_add(2).ok_or("u16 offset overflow")?)
        .ok_or("truncated u16")?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn le_u32(data: &[u8], offset: usize) -> Result<u32, String> {
    let bytes = data
        .get(offset..offset.checked_add(4).ok_or("u32 offset overflow")?)
        .ok_or("truncated u32")?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn enforce_push(current: usize, limit: usize, resource: &str) -> Result<(), String> {
    if current >= limit {
        Err(format!("XLS {resource} limit exceeded"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(kind: u16, payload: &[u8]) -> Vec<u8> {
        let mut result = Vec::with_capacity(payload.len() + 4);
        result.extend_from_slice(&kind.to_le_bytes());
        result.extend_from_slice(
            &u16::try_from(payload.len())
                .expect("test record payload fits u16")
                .to_le_bytes(),
        );
        result.extend_from_slice(payload);
        result
    }

    #[test]
    fn record_iterator_rejects_truncation_and_limits() {
        assert!(records(&[0x09, 0x08, 0x04, 0x00, 0x00], 10).is_err());
        let bytes = [record(BOF, &[0, 6, 5, 0]), record(EOF, &[])].concat();
        assert!(records(&bytes, 1).is_err());
        assert_eq!(records(&bytes, 2).expect("records").len(), 2);
    }

    #[test]
    fn parses_a_complete_synthetic_biff8_stream_with_bounded_layout() {
        let bof = record(BOF, &[0x00, 0x06, 0x05, 0x00]);
        let xf = record(XF, &[0; 20]);
        let mut boundsheet_payload = vec![0; 8];
        boundsheet_payload[4] = 1; // Hidden worksheets still keep their index and formatting.
        boundsheet_payload[6] = 1;
        boundsheet_payload.extend_from_slice(b"S");
        let boundsheet = record(BOUNDSHEET, &boundsheet_payload);
        let globals_len = bof.len() + xf.len() + boundsheet.len() + record(EOF, &[]).len();
        boundsheet_payload[0..4].copy_from_slice(
            &u32::try_from(globals_len)
                .expect("synthetic stream offset fits u32")
                .to_le_bytes(),
        );

        let mut dimensions = [0_u8; 14];
        dimensions[4..8].copy_from_slice(&1_u32.to_le_bytes());
        dimensions[10..12].copy_from_slice(&1_u16.to_le_bytes());
        let mut colinfo = [0_u8; 12];
        colinfo[4..6].copy_from_slice(&(20_u16 * 256).to_le_bytes());
        let mut row = [0_u8; 16];
        row[6..8].copy_from_slice(&400_u16.to_le_bytes());
        let mut number = [0_u8; 14];
        number[6..14].copy_from_slice(&12.5_f64.to_le_bytes());
        let merge = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        let bytes = [
            bof.clone(),
            xf,
            record(BOUNDSHEET, &boundsheet_payload),
            record(EOF, &[]),
            bof,
            record(DIMENSIONS, &dimensions),
            record(COLINFO, &colinfo),
            record(ROW, &row),
            record(NUMBER, &number),
            record(MERGEDCELLS, &merge),
            record(EOF, &[]),
        ]
        .concat();

        let parsed = WorkbookFormatting::parse_stream(&bytes, XlsLimits::default())
            .expect("synthetic workbook");
        assert_eq!(parsed.sheets.len(), 1);
        let sheet = &parsed.sheets[0];
        assert_eq!(sheet.name, "S");
        assert!((sheet.columns[0].width_chars - 20.0).abs() < f64::EPSILON);
        assert_eq!(sheet.rows[&0].height_twips, 400);
        assert_eq!(sheet.cell_xfs[&(0, 0)], 0);
        assert_eq!(sheet.source_values[&(0, 0)], SourceCellValue::Number(12.5));
        assert_eq!(sheet.merges.len(), 1);
    }

    #[test]
    fn distinguishes_worksheets_from_chart_sheets_without_dropping_hidden_sheets() {
        let mut worksheet = vec![0_u8; 8];
        worksheet[4] = 1;
        worksheet[6] = 1;
        worksheet.extend_from_slice(b"S");
        assert!(
            parse_boundsheet(&worksheet)
                .expect("worksheet")
                .is_worksheet
        );

        let mut chart = worksheet;
        chart[5] = 2;
        assert!(!parse_boundsheet(&chart).expect("chart").is_worksheet);
    }

    #[test]
    fn parses_font_xf_palette_and_custom_format() {
        let mut font = vec![0; 16];
        font[0..2].copy_from_slice(&240_u16.to_le_bytes());
        font[2..4].copy_from_slice(&0x000A_u16.to_le_bytes());
        font[4..6].copy_from_slice(&10_u16.to_le_bytes());
        font[6..8].copy_from_slice(&700_u16.to_le_bytes());
        font[10] = 1;
        font[11] = 2;
        font[12] = 204;
        font[14] = 4;
        font[15] = 1;
        font.extend("Тест".encode_utf16().flat_map(u16::to_le_bytes));
        let parsed = parse_font(&font).expect("font");
        assert_eq!(parsed.name, "Тест");
        assert!(parsed.italic && parsed.strike);
        assert_eq!(parsed.weight, 700);

        let mut xf = [0_u8; 20];
        xf[0..2].copy_from_slice(&2_u16.to_le_bytes());
        xf[2..4].copy_from_slice(&164_u16.to_le_bytes());
        xf[6] = 0x2A; // center, wrap, vertical center
        xf[7] = 45;
        xf[8] = 0x91; // indent, shrink, RTL
        xf[10..14].copy_from_slice(&0x0504_4321_u32.to_le_bytes());
        let border2 = 9_u32 | (10_u32 << 7) | (1_u32 << 26);
        xf[14..18].copy_from_slice(&border2.to_le_bytes());
        xf[18..20].copy_from_slice(&(0x000C_u16 | (0x000D_u16 << 7)).to_le_bytes());
        let parsed = parse_xf(&xf).expect("xf");
        assert_eq!(parsed.horizontal, 2);
        assert!(parsed.wrap && parsed.shrink_to_fit);
        assert_eq!(parsed.border.left_style, 1);
        assert_eq!(parsed.fill.pattern, 1);

        let palette = [2, 0, 1, 2, 3, 0, 4, 5, 6, 0];
        assert_eq!(
            parse_palette(&palette).expect("palette"),
            vec![[1, 2, 3], [4, 5, 6]]
        );

        let mut format = vec![164, 0, 4, 0, 0];
        format.extend_from_slice(b"0.00");
        assert_eq!(parse_format(&format).expect("format"), (164, "0.00".into()));
    }

    #[test]
    fn parses_geometry_cell_styles_and_merges() {
        let col = [1, 0, 3, 0, 0, 8, 2, 0, 1, 0, 0, 0];
        let band = parse_colinfo(&col).expect("colinfo").expect("band");
        assert_eq!(
            (band.first, band.last, band.width_chars, band.hidden),
            (1, 3, 8.0, true)
        );

        let mut row = [0_u8; 16];
        row[0..2].copy_from_slice(&7_u16.to_le_bytes());
        row[6..8].copy_from_slice(&(0x0258_u16 | 0x8000).to_le_bytes());
        row[12..16].copy_from_slice(&(0x20_u32 | 0x80 | (2_u32 << 16)).to_le_bytes());
        let (_, layout) = parse_row(&row).expect("row");
        assert_eq!(layout.height_twips, 600);
        assert!(layout.hidden && layout.custom_height);
        assert_eq!(layout.xf, Some(2));

        let mut target = Vec::new();
        parse_merges(&[1, 0, 0, 0, 1, 0, 2, 0, 4, 0], &mut target, 10).expect("merge");
        assert_eq!(
            target[0],
            CellRange {
                first_row: 0,
                last_row: 1,
                first_col: 2,
                last_col: 4
            }
        );
    }

    #[test]
    fn decodes_rk_and_formula_cached_results() {
        assert!((decode_rk((123_i32 << 2).cast_unsigned() | 2) - 123.0).abs() < f64::EPSILON);
        assert!((decode_rk((123_i32 << 2).cast_unsigned() | 3) - 1.23).abs() < f64::EPSILON);
        let mut formula = vec![0_u8; 14];
        formula[6..14].copy_from_slice(&42.5_f64.to_le_bytes());
        assert_eq!(
            parse_source_value(FORMULA, &formula).expect("formula"),
            Some(SourceCellValue::Number(42.5))
        );
        formula[6..14].copy_from_slice(&[1, 0, 1, 0, 0, 0, 0xFF, 0xFF]);
        assert_eq!(
            parse_source_value(FORMULA, &formula).expect("boolean"),
            Some(SourceCellValue::Boolean(true))
        );
    }

    #[test]
    fn extracts_common_external_and_internal_hyperlinks_without_guessing() {
        let mut external = vec![0_u8; 36];
        external.extend(
            "https://example.test/a"
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        );
        external.extend_from_slice(&[0, 0]);
        assert!(matches!(
            parse_hlink(&external).expect("external").target,
            HyperlinkTarget::External(_)
        ));

        let mut internal = vec![0_u8; 36];
        internal.extend("#Sheet2!A1".encode_utf16().flat_map(u16::to_le_bytes));
        internal.extend_from_slice(&[0, 0]);
        assert_eq!(
            parse_hlink(&internal).expect("internal").target,
            HyperlinkTarget::Internal("Sheet2!A1".into())
        );

        let mut unsupported = vec![0_u8; 36];
        unsupported.extend(
            "file:///etc/passwd"
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        );
        assert!(parse_hlink(&unsupported).is_err());
    }
}
