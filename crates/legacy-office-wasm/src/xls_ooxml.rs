//! Projection of source-backed BIFF8 formatting into generated XLSX parts.

use std::fmt::Write as _;
use std::io::{Cursor, Read, Write};

use zip::{
    CompressionMethod, ZipArchive,
    write::{SimpleFileOptions, ZipWriter},
};

use crate::xls_biff::{
    CellRange, HyperlinkTarget, SheetFormatting, SourceCellValue, WorkbookFormatting, XlsBorder,
    XlsFill,
};

const STYLES_XML: &str = "xl/styles.xml";

#[derive(Debug)]
struct PackageEntry {
    name: String,
    compression: CompressionMethod,
    data: Vec<u8>,
}

pub(crate) fn add_xls_fidelity_to_xlsx(
    xlsx: Vec<u8>,
    workbook: &WorkbookFormatting,
) -> Result<Vec<u8>, String> {
    if workbook.sheets.is_empty() || workbook.xfs.is_empty() {
        return Ok(xlsx);
    }
    let mut archive = ZipArchive::new(Cursor::new(xlsx)).map_err(|error| error.to_string())?;
    let mut entries = Vec::with_capacity(archive.len() + workbook.sheets.len());
    let mut seen_styles = false;
    let mut seen_sheets = vec![false; workbook.sheets.len()];
    let mut seen_rels = vec![false; workbook.sheets.len()];

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let compression = entry.compression();
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|error| error.to_string())?;
        if name == STYLES_XML {
            data = styles_xml(workbook).into_bytes();
            seen_styles = true;
        } else if let Some(sheet_index) = worksheet_index(&name, ".xml") {
            if let Some(sheet) = workbook.sheets.get(sheet_index) {
                data = patch_worksheet(&data, sheet)?;
                seen_sheets[sheet_index] = true;
            }
        } else if let Some(sheet_index) = worksheet_index(&name, ".xml.rels")
            && let Some(sheet) = workbook.sheets.get(sheet_index)
        {
            data = patch_relationships(&data, sheet)?;
            seen_rels[sheet_index] = true;
        }
        entries.push(PackageEntry {
            name,
            compression,
            data,
        });
    }
    if !seen_styles || seen_sheets.iter().any(|seen| !seen) {
        return Err("generated XLSX is missing styles.xml or a worksheet part".into());
    }
    for (index, sheet) in workbook.sheets.iter().enumerate() {
        if !seen_rels[index] && has_external_hyperlinks(sheet) {
            entries.push(PackageEntry {
                name: format!("xl/worksheets/_rels/sheet{}.xml.rels", index + 1),
                compression: CompressionMethod::Deflated,
                data: new_relationships(sheet).into_bytes(),
            });
        }
    }
    write_package(entries)
}

fn worksheet_index(name: &str, suffix: &str) -> Option<usize> {
    let middle = name
        .strip_prefix(if suffix == ".xml" {
            "xl/worksheets/sheet"
        } else {
            "xl/worksheets/_rels/sheet"
        })?
        .strip_suffix(suffix)?;
    middle.parse::<usize>().ok()?.checked_sub(1)
}

#[allow(clippy::too_many_lines)]
fn styles_xml(workbook: &WorkbookFormatting) -> String {
    let xfs = resolved_xfs(workbook);
    let fills = unique_fills(&xfs);
    let borders = unique_borders(&xfs);
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">",
    );
    if !workbook.custom_formats.is_empty() {
        write!(xml, "<numFmts count=\"{}\">", workbook.custom_formats.len()).unwrap();
        for (id, code) in &workbook.custom_formats {
            write!(
                xml,
                "<numFmt numFmtId=\"{id}\" formatCode=\"{}\"/>",
                escape_xml(code)
            )
            .unwrap();
        }
        xml.push_str("</numFmts>");
    }

    let font_count = ooxml_font_count(workbook);
    write!(xml, "<fonts count=\"{font_count}\">").unwrap();
    for index in 0..font_count {
        if index == 4 && workbook.fonts.len() > 4 {
            xml.push_str("<font><sz val=\"10\"/><name val=\"Arial\"/></font>");
            continue;
        }
        let source = if index < 4 { index } else { index - 1 };
        if let Some(font) = workbook.fonts.get(source) {
            xml.push_str("<font>");
            if font.weight >= 700 {
                xml.push_str("<b/>");
            }
            if font.italic {
                xml.push_str("<i/>");
            }
            if font.strike {
                xml.push_str("<strike/>");
            }
            match font.underline {
                1 => xml.push_str("<u/>"),
                2 => xml.push_str("<u val=\"double\"/>"),
                0x21 => xml.push_str("<u val=\"singleAccounting\"/>"),
                0x22 => xml.push_str("<u val=\"doubleAccounting\"/>"),
                _ => {}
            }
            write!(
                xml,
                "<sz val=\"{:.2}\"/>",
                f64::from(font.height_twips) / 20.0
            )
            .unwrap();
            if let Some(color) = color_hex(workbook, font.color_index) {
                write!(xml, "<color rgb=\"FF{color}\"/>").unwrap();
            }
            write!(xml, "<name val=\"{}\"/>", escape_xml(&font.name)).unwrap();
            if font.family != 0 {
                write!(xml, "<family val=\"{}\"/>", font.family).unwrap();
            }
            if font.charset != 0 {
                write!(xml, "<charset val=\"{}\"/>", font.charset).unwrap();
            }
            xml.push_str("</font>");
        } else {
            xml.push_str("<font><sz val=\"11\"/><name val=\"Calibri\"/><family val=\"2\"/></font>");
        }
    }
    xml.push_str("</fonts>");

    write!(xml, "<fills count=\"{}\">", fills.len()).unwrap();
    for fill in &fills {
        write_fill(&mut xml, workbook, *fill);
    }
    xml.push_str("</fills>");

    write!(xml, "<borders count=\"{}\">", borders.len()).unwrap();
    for border in &borders {
        write_border(&mut xml, workbook, *border);
    }
    xml.push_str("</borders>");
    xml.push_str("<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>");
    write!(xml, "<cellXfs count=\"{}\">", workbook.xfs.len()).unwrap();
    for xf in &xfs {
        let fill_id = fills.iter().position(|fill| fill == &xf.fill).unwrap_or(0);
        let border_id = borders
            .iter()
            .position(|border| border == &xf.border)
            .unwrap_or(0);
        write!(
            xml,
            "<xf numFmtId=\"{}\" fontId=\"{}\" fillId=\"{fill_id}\" borderId=\"{border_id}\" xfId=\"0\" applyNumberFormat=\"1\" applyFont=\"1\" applyFill=\"1\" applyBorder=\"1\" applyAlignment=\"1\"",
            xf.format_index,
            valid_font_id(workbook, xf.font_index)
        )
        .unwrap();
        if has_alignment(xf) {
            xml.push('>');
            xml.push_str("<alignment");
            if let Some(value) = horizontal_name(xf.horizontal) {
                write!(xml, " horizontal=\"{value}\"").unwrap();
            }
            if let Some(value) = vertical_name(xf.vertical) {
                write!(xml, " vertical=\"{value}\"").unwrap();
            }
            if xf.wrap {
                xml.push_str(" wrapText=\"1\"");
            }
            if xf.shrink_to_fit {
                xml.push_str(" shrinkToFit=\"1\"");
            }
            if xf.indent != 0 {
                write!(xml, " indent=\"{}\"", xf.indent).unwrap();
            }
            if xf.rotation != 0 {
                let rotation = if xf.rotation <= 90 {
                    i16::from(xf.rotation)
                } else if xf.rotation <= 180 {
                    90 - i16::from(xf.rotation)
                } else {
                    255
                };
                write!(xml, " textRotation=\"{rotation}\"").unwrap();
            }
            match xf.reading_order {
                1 => xml.push_str(" readingOrder=\"1\""),
                2 => xml.push_str(" readingOrder=\"2\""),
                _ => {}
            }
            xml.push_str("/></xf>");
        } else {
            xml.push_str("/>");
        }
    }
    xml.push_str("</cellXfs><cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles></styleSheet>");
    xml
}

fn unique_fills(xfs: &[crate::xls_biff::XlsXf]) -> Vec<XlsFill> {
    let mut fills = vec![
        XlsFill {
            pattern: 0,
            foreground: 0,
            background: 0,
        },
        XlsFill {
            pattern: 17,
            foreground: 0,
            background: 0,
        },
    ];
    for xf in xfs {
        if !fills.contains(&xf.fill) {
            fills.push(xf.fill);
        }
    }
    fills
}

fn unique_borders(xfs: &[crate::xls_biff::XlsXf]) -> Vec<XlsBorder> {
    let mut borders = vec![XlsBorder {
        left_style: 0,
        right_style: 0,
        top_style: 0,
        bottom_style: 0,
        left_color: 0,
        right_color: 0,
        top_color: 0,
        bottom_color: 0,
    }];
    for xf in xfs {
        if !borders.contains(&xf.border) {
            borders.push(xf.border);
        }
    }
    borders
}

fn resolved_xfs(workbook: &WorkbookFormatting) -> Vec<crate::xls_biff::XlsXf> {
    workbook
        .xfs
        .iter()
        .copied()
        .map(|xf| {
            if xf.is_style {
                return xf;
            }
            let Some(parent) = workbook.xfs.get(usize::from(xf.parent_style)).copied() else {
                return xf;
            };
            let mut resolved = xf;
            if xf.used_attributes & 0x04 == 0 {
                resolved.format_index = parent.format_index;
            }
            if xf.used_attributes & 0x08 == 0 {
                resolved.font_index = parent.font_index;
            }
            if xf.used_attributes & 0x10 == 0 {
                resolved.horizontal = parent.horizontal;
                resolved.vertical = parent.vertical;
                resolved.wrap = parent.wrap;
                resolved.rotation = parent.rotation;
                resolved.indent = parent.indent;
                resolved.shrink_to_fit = parent.shrink_to_fit;
                resolved.reading_order = parent.reading_order;
            }
            if xf.used_attributes & 0x20 == 0 {
                resolved.border = parent.border;
            }
            if xf.used_attributes & 0x40 == 0 {
                resolved.fill = parent.fill;
            }
            resolved
        })
        .collect()
}

fn write_fill(xml: &mut String, workbook: &WorkbookFormatting, fill: XlsFill) {
    let pattern = pattern_name(fill.pattern);
    write!(xml, "<fill><patternFill patternType=\"{pattern}\">").unwrap();
    if fill.pattern != 0 {
        if let Some(color) = color_hex(workbook, u16::from(fill.foreground)) {
            write!(xml, "<fgColor rgb=\"FF{color}\"/>").unwrap();
        }
        if let Some(color) = color_hex(workbook, u16::from(fill.background)) {
            write!(xml, "<bgColor rgb=\"FF{color}\"/>").unwrap();
        }
    }
    xml.push_str("</patternFill></fill>");
}

fn write_border(xml: &mut String, workbook: &WorkbookFormatting, border: XlsBorder) {
    xml.push_str("<border>");
    write_border_side(xml, "left", border.left_style, border.left_color, workbook);
    write_border_side(
        xml,
        "right",
        border.right_style,
        border.right_color,
        workbook,
    );
    write_border_side(xml, "top", border.top_style, border.top_color, workbook);
    write_border_side(
        xml,
        "bottom",
        border.bottom_style,
        border.bottom_color,
        workbook,
    );
    xml.push_str("<diagonal/></border>");
}

fn write_border_side(
    xml: &mut String,
    name: &str,
    style: u8,
    color: u8,
    workbook: &WorkbookFormatting,
) {
    if style == 0 {
        write!(xml, "<{name}/>").unwrap();
        return;
    }
    write!(xml, "<{name} style=\"{}\">", border_name(style)).unwrap();
    if let Some(color) = color_hex(workbook, u16::from(color)) {
        write!(xml, "<color rgb=\"FF{color}\"/>").unwrap();
    } else {
        xml.push_str("<color auto=\"1\"/>");
    }
    write!(xml, "</{name}>").unwrap();
}

fn patch_worksheet(data: &[u8], sheet: &SheetFormatting) -> Result<Vec<u8>, String> {
    let mut xml = String::from_utf8(data.to_vec())
        .map_err(|_| "generated worksheet XML is not UTF-8".to_string())?;
    patch_source_values(&mut xml, sheet)?;
    ensure_styled_blank_cells(&mut xml, sheet)?;
    patch_cell_styles(&mut xml, sheet)?;
    patch_rows(&mut xml, sheet)?;

    let sheet_data = xml
        .find("<sheetData")
        .ok_or("worksheet is missing sheetData")?;
    let mut before = String::new();
    if sheet.default_col_width_chars.is_some() || sheet.default_row_height_twips.is_some() {
        before.push_str("<sheetFormatPr");
        if let Some(width) = sheet.default_col_width_chars {
            write!(before, " defaultColWidth=\"{width:.4}\"").unwrap();
        }
        if let Some(height) = sheet.default_row_height_twips {
            write!(
                before,
                " defaultRowHeight=\"{:.2}\"",
                f64::from(height) / 20.0
            )
            .unwrap();
        }
        before.push_str("/>");
    }
    if !sheet.columns.is_empty() {
        before.push_str("<cols>");
        for col in &sheet.columns {
            write!(
                before,
                "<col min=\"{}\" max=\"{}\" width=\"{:.4}\" customWidth=\"1\"",
                col.first + 1,
                col.last + 1,
                col.width_chars
            )
            .unwrap();
            if col.hidden {
                before.push_str(" hidden=\"1\"");
            }
            if usize::from(col.xf) < 4_096 {
                write!(before, " style=\"{}\"", col.xf).unwrap();
            }
            before.push_str("/>");
        }
        before.push_str("</cols>");
    }
    xml.insert_str(sheet_data, &before);

    let sheet_data_end = xml
        .find("</sheetData>")
        .ok_or("worksheet has unterminated sheetData")?
        + "</sheetData>".len();
    let mut after = String::new();
    if !sheet.merges.is_empty() {
        write!(after, "<mergeCells count=\"{}\">", sheet.merges.len()).unwrap();
        for range in &sheet.merges {
            write!(after, "<mergeCell ref=\"{}\"/>", range_ref(*range)).unwrap();
        }
        after.push_str("</mergeCells>");
    }
    if !sheet.hyperlinks.is_empty() {
        after.push_str("<hyperlinks>");
        let mut external_index = 0usize;
        for link in &sheet.hyperlinks {
            match &link.target {
                HyperlinkTarget::External(_) => {
                    external_index += 1;
                    write!(
                        after,
                        "<hyperlink ref=\"{}\" r:id=\"rIdZrimoXlsH{external_index}\"/>",
                        range_ref(link.range)
                    )
                    .unwrap();
                }
                HyperlinkTarget::Internal(location) => {
                    write!(
                        after,
                        "<hyperlink ref=\"{}\" location=\"{}\"/>",
                        range_ref(link.range),
                        escape_xml(location)
                    )
                    .unwrap();
                }
            }
        }
        after.push_str("</hyperlinks>");
    }
    xml.insert_str(sheet_data_end, &after);
    Ok(xml.into_bytes())
}

fn patch_source_values(xml: &mut String, sheet: &SheetFormatting) -> Result<(), String> {
    let mut cursor = 0usize;
    while let Some(relative) = xml[cursor..].find("<c ") {
        let start = cursor + relative;
        let tag_end = xml[start..]
            .find('>')
            .map(|offset| start + offset)
            .ok_or("unterminated worksheet cell")?;
        let tag = &xml[start..=tag_end];
        let source = attribute(tag, "r")
            .and_then(parse_cell_ref)
            .and_then(|position| sheet.source_values.get(&position));
        let Some(source) = source else {
            cursor = tag_end + 1;
            continue;
        };
        let end = xml[tag_end + 1..]
            .find("</c>")
            .map(|offset| tag_end + 1 + offset + "</c>".len())
            .ok_or("source value cell is not closed")?;
        let reference = attribute(tag, "r").ok_or("source value cell has no reference")?;
        let replacement = match source {
            SourceCellValue::Number(value) if value.is_finite() => {
                format!("<c r=\"{reference}\"><v>{value}</v></c>")
            }
            SourceCellValue::Boolean(value) => format!(
                "<c r=\"{reference}\" t=\"b\"><v>{}</v></c>",
                u8::from(*value)
            ),
            SourceCellValue::Error(code) => format!(
                "<c r=\"{reference}\" t=\"e\"><v>{}</v></c>",
                error_name(*code)
            ),
            SourceCellValue::Number(_) => {
                cursor = end;
                continue;
            }
        };
        xml.replace_range(start..end, &replacement);
        cursor = start + replacement.len();
    }
    Ok(())
}

fn error_name(code: u8) -> &'static str {
    match code {
        0x00 => "#NULL!",
        0x07 => "#DIV/0!",
        0x17 => "#REF!",
        0x1D => "#NAME?",
        0x24 => "#NUM!",
        0x2A => "#N/A",
        _ => "#VALUE!",
    }
}

fn ensure_styled_blank_cells(xml: &mut String, sheet: &SheetFormatting) -> Result<(), String> {
    let Some(dimensions) = sheet.dimensions else {
        return Ok(());
    };
    let start_tag = xml
        .find("<sheetData")
        .ok_or("worksheet is missing sheetData")?;
    let content_start = xml[start_tag..]
        .find('>')
        .map(|offset| start_tag + offset + 1)
        .ok_or("unterminated sheetData")?;
    let content_end = xml[content_start..]
        .find("</sheetData>")
        .map(|offset| content_start + offset)
        .ok_or("worksheet has unterminated sheetData")?;
    let mut rows = parse_row_fragments(&xml[content_start..content_end])?;

    for row in sheet
        .rows
        .keys()
        .copied()
        .filter(|row| *row >= dimensions.first_row && *row <= dimensions.last_row)
    {
        let row_number = u32::from(row) + 1;
        rows.entry(row_number)
            .or_insert_with(|| format!("<row r=\"{row_number}\"></row>"));
    }

    let mut blank_cells = sheet
        .cell_xfs
        .iter()
        .filter(|((row, col), _)| {
            *row >= dimensions.first_row
                && *row <= dimensions.last_row
                && *col >= dimensions.first_col
                && *col <= dimensions.last_col
        })
        .map(|(position, xf)| (*position, *xf))
        .collect::<Vec<_>>();
    blank_cells.sort_unstable_by_key(|((row, col), _)| (*row, *col));
    for ((row, col), xf) in blank_cells {
        let row_number = u32::from(row) + 1;
        let fragment = rows
            .entry(row_number)
            .or_insert_with(|| format!("<row r=\"{row_number}\"></row>"));
        insert_styled_blank_cell(fragment, row, col, xf)?;
    }

    let replacement = rows.into_values().collect::<String>();
    xml.replace_range(content_start..content_end, &replacement);
    Ok(())
}

fn parse_row_fragments(
    sheet_data: &str,
) -> Result<std::collections::BTreeMap<u32, String>, String> {
    let mut rows = std::collections::BTreeMap::new();
    let mut cursor = 0usize;
    while let Some(relative) = sheet_data[cursor..].find("<row ") {
        let start = cursor + relative;
        let tag_end = sheet_data[start..]
            .find('>')
            .map(|offset| start + offset)
            .ok_or("unterminated worksheet row")?;
        let tag = &sheet_data[start..=tag_end];
        let number = attribute(tag, "r")
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| (1..=65_536).contains(value))
            .ok_or("worksheet row has no valid r attribute")?;
        let end = sheet_data[tag_end + 1..]
            .find("</row>")
            .map(|offset| tag_end + 1 + offset + "</row>".len())
            .ok_or("worksheet row is not closed")?;
        if rows
            .insert(number, sheet_data[start..end].to_string())
            .is_some()
        {
            return Err(format!("worksheet contains duplicate row {number}"));
        }
        cursor = end;
    }
    Ok(rows)
}

fn insert_styled_blank_cell(
    row_xml: &mut String,
    row: u16,
    col: u16,
    xf: u16,
) -> Result<(), String> {
    let reference = cell_ref(row, col);
    let mut cursor = 0usize;
    let mut insertion = None;
    while let Some(relative) = row_xml[cursor..].find("<c ") {
        let start = cursor + relative;
        let end = row_xml[start..]
            .find('>')
            .map(|offset| start + offset)
            .ok_or("unterminated worksheet cell")?;
        let tag = &row_xml[start..=end];
        if let Some(existing) = attribute(tag, "r") {
            if existing == reference {
                return Ok(());
            }
            if parse_cell_ref(existing).is_some_and(|(_, existing_col)| existing_col > col) {
                insertion = Some(start);
                break;
            }
        }
        cursor = end + 1;
    }
    let insertion = insertion
        .or_else(|| row_xml.rfind("</row>"))
        .ok_or("worksheet row is not closed")?;
    row_xml.insert_str(insertion, &format!("<c r=\"{reference}\" s=\"{xf}\"/>"));
    Ok(())
}

fn patch_cell_styles(xml: &mut String, sheet: &SheetFormatting) -> Result<(), String> {
    let mut cursor = 0usize;
    while let Some(relative) = xml[cursor..].find("<c ") {
        let start = cursor + relative;
        let end = xml[start..]
            .find('>')
            .map(|n| start + n)
            .ok_or("unterminated worksheet cell")?;
        let tag = &xml[start..=end];
        if let Some(reference) = attribute(tag, "r")
            && let Some((row, col)) = parse_cell_ref(reference)
            && let Some(xf) = sheet.cell_xfs.get(&(row, col))
        {
            let replacement = set_attribute(tag, "s", &xf.to_string());
            xml.replace_range(start..=end, &replacement);
            cursor = start + replacement.len();
            continue;
        }
        cursor = end + 1;
    }
    Ok(())
}

fn patch_rows(xml: &mut String, sheet: &SheetFormatting) -> Result<(), String> {
    let mut cursor = 0usize;
    while let Some(relative) = xml[cursor..].find("<row ") {
        let start = cursor + relative;
        let end = xml[start..]
            .find('>')
            .map(|n| start + n)
            .ok_or("unterminated worksheet row")?;
        let tag = &xml[start..=end];
        let Some(row_number) = attribute(tag, "r")
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| (1..=65_536).contains(value))
        else {
            cursor = end + 1;
            continue;
        };
        let source_row = u16::try_from(row_number - 1).map_err(|_| "worksheet row overflow")?;
        let Some(layout) = sheet.rows.get(&source_row) else {
            cursor = end + 1;
            continue;
        };
        let mut replacement = tag.to_string();
        if layout.custom_height || layout.height_twips != 0 {
            replacement = set_attribute(
                &replacement,
                "ht",
                &format!("{:.2}", f64::from(layout.height_twips) / 20.0),
            );
            replacement = set_attribute(&replacement, "customHeight", "1");
        }
        if layout.hidden {
            replacement = set_attribute(&replacement, "hidden", "1");
        }
        if let Some(xf) = layout.xf {
            replacement = set_attribute(&replacement, "s", &xf.to_string());
            replacement = set_attribute(&replacement, "customFormat", "1");
        }
        xml.replace_range(start..=end, &replacement);
        cursor = start + replacement.len();
    }
    Ok(())
}

fn patch_relationships(data: &[u8], sheet: &SheetFormatting) -> Result<Vec<u8>, String> {
    if !has_external_hyperlinks(sheet) {
        return Ok(data.to_vec());
    }
    let mut xml = String::from_utf8(data.to_vec())
        .map_err(|_| "worksheet relationships are not UTF-8".to_string())?;
    let insertion = xml
        .rfind("</Relationships>")
        .ok_or("invalid worksheet relationships XML")?;
    xml.insert_str(insertion, &external_relationship_elements(sheet));
    Ok(xml.into_bytes())
}

fn new_relationships(sheet: &SheetFormatting) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{}</Relationships>",
        external_relationship_elements(sheet)
    )
}

fn external_relationship_elements(sheet: &SheetFormatting) -> String {
    let mut xml = String::new();
    let mut index = 0usize;
    for link in &sheet.hyperlinks {
        if let HyperlinkTarget::External(target) = &link.target {
            index += 1;
            write!(xml, "<Relationship Id=\"rIdZrimoXlsH{index}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"{}\" TargetMode=\"External\"/>", escape_xml(target)).unwrap();
        }
    }
    xml
}

fn has_external_hyperlinks(sheet: &SheetFormatting) -> bool {
    sheet
        .hyperlinks
        .iter()
        .any(|link| matches!(link.target, HyperlinkTarget::External(_)))
}

fn write_package(entries: Vec<PackageEntry>) -> Result<Vec<u8>, String> {
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut output);
        for entry in entries {
            writer
                .start_file(
                    &entry.name,
                    SimpleFileOptions::default().compression_method(entry.compression),
                )
                .map_err(|error| error.to_string())?;
            writer
                .write_all(&entry.data)
                .map_err(|error| error.to_string())?;
        }
        writer.finish().map_err(|error| error.to_string())?;
    }
    Ok(output.into_inner())
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

fn set_attribute(tag: &str, name: &str, value: &str) -> String {
    let needle = format!(" {name}=\"");
    if let Some(start) = tag.find(&needle) {
        let value_start = start + needle.len();
        if let Some(relative_end) = tag[value_start..].find('"') {
            let mut result = tag.to_string();
            result.replace_range(value_start..value_start + relative_end, value);
            return result;
        }
    }
    let insertion = if tag.ends_with("/>") {
        tag.len() - 2
    } else {
        tag.rfind('>').unwrap_or(tag.len())
    };
    let mut result = tag.to_string();
    result.insert_str(insertion, &format!(" {name}=\"{value}\""));
    result
}

fn parse_cell_ref(value: &str) -> Option<(u16, u16)> {
    let split = value.find(|ch: char| ch.is_ascii_digit())?;
    let (letters, digits) = value.split_at(split);
    if letters.is_empty() || digits.is_empty() {
        return None;
    }
    let mut col = 0u32;
    for byte in letters.bytes() {
        if !byte.is_ascii_alphabetic() {
            return None;
        }
        col = col
            .checked_mul(26)?
            .checked_add(u32::from(byte.to_ascii_uppercase() - b'A' + 1))?;
    }
    let row = digits.parse::<u32>().ok()?;
    if col == 0 || col > 256 || row == 0 || row > 65_536 {
        return None;
    }
    Some((u16::try_from(row - 1).ok()?, u16::try_from(col - 1).ok()?))
}

fn range_ref(range: CellRange) -> String {
    let first = cell_ref(range.first_row, range.first_col);
    let last = cell_ref(range.last_row, range.last_col);
    if first == last {
        first
    } else {
        format!("{first}:{last}")
    }
}

fn cell_ref(row: u16, col: u16) -> String {
    format!("{}{}", column_name(col), u32::from(row) + 1)
}

fn column_name(mut col: u16) -> String {
    let mut name = String::new();
    loop {
        name.insert(0, char::from(b'A' + (col % 26) as u8));
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    name
}

fn ooxml_font_count(workbook: &WorkbookFormatting) -> usize {
    if workbook.fonts.is_empty() {
        1
    } else if workbook.fonts.len() > 4 {
        workbook.fonts.len() + 1
    } else {
        workbook.fonts.len()
    }
}

fn valid_font_id(workbook: &WorkbookFormatting, source: u16) -> u16 {
    if usize::from(source) < ooxml_font_count(workbook) {
        source
    } else {
        0
    }
}

fn has_alignment(xf: &crate::xls_biff::XlsXf) -> bool {
    xf.horizontal != 0
        || xf.vertical != 2
        || xf.wrap
        || xf.rotation != 0
        || xf.indent != 0
        || xf.shrink_to_fit
        || xf.reading_order != 0
}

fn horizontal_name(value: u8) -> Option<&'static str> {
    [
        None,
        Some("left"),
        Some("center"),
        Some("right"),
        Some("fill"),
        Some("justify"),
        Some("centerContinuous"),
        Some("distributed"),
    ]
    .get(usize::from(value))
    .copied()
    .flatten()
}

fn vertical_name(value: u8) -> Option<&'static str> {
    [
        Some("top"),
        Some("center"),
        Some("bottom"),
        Some("justify"),
        Some("distributed"),
    ]
    .get(usize::from(value))
    .copied()
    .flatten()
}

fn pattern_name(value: u8) -> &'static str {
    [
        "none",
        "solid",
        "mediumGray",
        "darkGray",
        "lightGray",
        "darkHorizontal",
        "darkVertical",
        "darkDown",
        "darkUp",
        "darkGrid",
        "darkTrellis",
        "lightHorizontal",
        "lightVertical",
        "lightDown",
        "lightUp",
        "lightGrid",
        "lightTrellis",
        "gray125",
        "gray0625",
    ]
    .get(usize::from(value))
    .copied()
    .unwrap_or("none")
}

fn border_name(value: u8) -> &'static str {
    [
        "none",
        "thin",
        "medium",
        "dashed",
        "dotted",
        "thick",
        "double",
        "hair",
        "mediumDashed",
        "dashDot",
        "mediumDashDot",
        "dashDotDot",
        "mediumDashDotDot",
        "slantDashDot",
    ]
    .get(usize::from(value))
    .copied()
    .unwrap_or("thin")
}

fn color_hex(workbook: &WorkbookFormatting, index: u16) -> Option<String> {
    if (8..=63).contains(&index) {
        let palette_index = usize::from(index - 8);
        let rgb = workbook
            .palette
            .get(palette_index)
            .copied()
            .unwrap_or(DEFAULT_PALETTE[palette_index]);
        return Some(format!("{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]));
    }
    let rgb = match index {
        0 | 64 => [0, 0, 0],
        1 | 65 => [255, 255, 255],
        2 => [255, 0, 0],
        3 => [0, 255, 0],
        4 => [0, 0, 255],
        5 => [255, 255, 0],
        6 => [255, 0, 255],
        7 => [0, 255, 255],
        _ => return None,
    };
    Some(format!("{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]))
}

const DEFAULT_PALETTE: [[u8; 3]; 56] = [
    [0, 0, 0],
    [255, 255, 255],
    [255, 0, 0],
    [0, 255, 0],
    [0, 0, 255],
    [255, 255, 0],
    [255, 0, 255],
    [0, 255, 255],
    [128, 0, 0],
    [0, 128, 0],
    [0, 0, 128],
    [128, 128, 0],
    [128, 0, 128],
    [0, 128, 128],
    [192, 192, 192],
    [128, 128, 128],
    [153, 153, 255],
    [153, 51, 102],
    [255, 255, 204],
    [204, 255, 255],
    [102, 0, 102],
    [255, 128, 128],
    [0, 102, 204],
    [204, 204, 255],
    [0, 0, 128],
    [255, 0, 255],
    [255, 255, 0],
    [0, 255, 255],
    [128, 0, 128],
    [128, 0, 0],
    [0, 128, 128],
    [0, 0, 255],
    [0, 204, 255],
    [204, 255, 255],
    [204, 255, 204],
    [255, 255, 153],
    [153, 204, 255],
    [255, 153, 204],
    [204, 153, 255],
    [255, 204, 153],
    [51, 102, 255],
    [51, 204, 204],
    [153, 204, 0],
    [255, 204, 0],
    [255, 153, 0],
    [255, 102, 0],
    [102, 102, 153],
    [150, 150, 150],
    [0, 51, 102],
    [51, 153, 102],
    [0, 51, 0],
    [51, 51, 0],
    [153, 51, 0],
    [153, 51, 102],
    [51, 51, 153],
    [51, 51, 51],
];

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xls_biff::{ColumnBand, RowLayout, XlsFont, XlsXf};

    fn sample_workbook() -> WorkbookFormatting {
        let border = XlsBorder {
            left_style: 1,
            right_style: 1,
            top_style: 1,
            bottom_style: 1,
            left_color: 8,
            right_color: 8,
            top_color: 8,
            bottom_color: 8,
        };
        let fill = XlsFill {
            pattern: 1,
            foreground: 10,
            background: 9,
        };
        WorkbookFormatting {
            fonts: vec![XlsFont {
                name: "Arial".into(),
                height_twips: 240,
                color_index: 8,
                weight: 700,
                italic: false,
                strike: false,
                underline: 0,
                family: 2,
                charset: 204,
            }],
            xfs: vec![XlsXf {
                font_index: 0,
                format_index: 164,
                parent_style: 0,
                is_style: false,
                used_attributes: 0x7C,
                horizontal: 2,
                vertical: 1,
                wrap: true,
                rotation: 0,
                indent: 0,
                shrink_to_fit: false,
                reading_order: 0,
                border,
                fill,
            }],
            custom_formats: [(164, "0.00%".into())].into(),
            palette: Vec::new(),
            sheets: Vec::new(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn serializes_source_styles_without_the_generated_header_heuristic() {
        let xml = styles_xml(&sample_workbook());
        assert!(xml.contains("<numFmt numFmtId=\"164\" formatCode=\"0.00%\"/>"));
        assert!(xml.contains("<b/><sz val=\"12.00\"/>"));
        assert!(xml.contains("patternType=\"solid\""));
        assert!(xml.contains("horizontal=\"center\" vertical=\"center\" wrapText=\"1\""));
    }

    #[test]
    fn patches_cells_rows_columns_merges_and_links() {
        let mut sheet = SheetFormatting {
            name: "S".into(),
            dimensions: Some(CellRange {
                first_row: 0,
                last_row: 0,
                first_col: 0,
                last_col: 1,
            }),
            default_col_width_chars: Some(8.0),
            default_row_height_twips: Some(300),
            ..SheetFormatting::default()
        };
        sheet.columns.push(ColumnBand {
            first: 0,
            last: 0,
            width_chars: 20.0,
            hidden: false,
            xf: 0,
        });
        sheet.rows.insert(
            0,
            RowLayout {
                height_twips: 600,
                hidden: false,
                custom_height: true,
                xf: None,
            },
        );
        sheet.cell_xfs.insert((0, 0), 7);
        sheet.cell_xfs.insert((0, 1), 8);
        sheet.merges.push(CellRange {
            first_row: 0,
            last_row: 0,
            first_col: 0,
            last_col: 1,
        });
        sheet.hyperlinks.push(crate::xls_biff::XlsHyperlink {
            range: CellRange {
                first_row: 0,
                last_row: 0,
                first_col: 0,
                last_col: 0,
            },
            target: HyperlinkTarget::External("https://example.test".into()),
        });
        let input = br#"<?xml version="1.0"?><worksheet xmlns:r="r"><sheetData><row r="1"><c r="A1" s="1" t="inlineStr"><is><t>x</t></is></c></row></sheetData></worksheet>"#;
        let output = String::from_utf8(patch_worksheet(input, &sheet).expect("patch")).unwrap();
        assert!(
            output
                .contains("<sheetFormatPr defaultColWidth=\"8.0000\" defaultRowHeight=\"15.00\"/>")
        );
        assert!(output.contains("<col min=\"1\" max=\"1\" width=\"20.0000\""));
        assert!(output.contains("<row r=\"1\" ht=\"30.00\" customHeight=\"1\">"));
        assert!(output.contains("<c r=\"A1\" s=\"7\""));
        assert!(output.contains("<c r=\"B1\" s=\"8\"/>"));
        assert!(output.contains("<mergeCell ref=\"A1:B1\"/>"));
        assert!(output.contains("r:id=\"rIdZrimoXlsH1\""));
    }

    #[test]
    fn cell_reference_round_trip_covers_last_biff8_column() {
        for (row, col, expected) in [
            (0, 0, "A1"),
            (9, 25, "Z10"),
            (0, 26, "AA1"),
            (65_535, 255, "IV65536"),
        ] {
            assert_eq!(cell_ref(row, col), expected);
            assert_eq!(parse_cell_ref(expected), Some((row, col)));
        }
    }

    #[test]
    fn patches_styles_and_geometry_on_the_last_biff8_row() {
        let mut sheet = SheetFormatting {
            dimensions: Some(CellRange {
                first_row: 65_535,
                last_row: 65_535,
                first_col: 255,
                last_col: 255,
            }),
            ..SheetFormatting::default()
        };
        sheet.cell_xfs.insert((65_535, 255), 0);
        sheet.rows.insert(
            65_535,
            RowLayout {
                height_twips: 400,
                hidden: false,
                custom_height: true,
                xf: None,
            },
        );
        let input = br#"<worksheet><sheetData><row r="65536"><c r="IV65536"/></row></sheetData></worksheet>"#;
        let output = String::from_utf8(patch_worksheet(input, &sheet).expect("patch")).unwrap();
        assert!(output.contains("<row r=\"65536\" ht=\"20.00\" customHeight=\"1\">"));
        assert!(output.contains("<c r=\"IV65536\" s=\"0\"/>"));
    }

    #[test]
    fn inserts_attributes_before_a_self_closing_slash() {
        assert_eq!(
            set_attribute("<c r=\"A1\"/>", "s", "2"),
            "<c r=\"A1\" s=\"2\"/>"
        );
    }
}
