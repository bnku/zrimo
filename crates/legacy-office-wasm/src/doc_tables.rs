//! Source-backed DOC table layout properties missing from the shared IR.

use std::io::{Cursor, Read, Write};

use legacy_doc::{DocLimits, StyledFormattingIndex, TableCollection, TableRow, WordBinaryDocument};
use zip::{
    CompressionMethod, ZipArchive,
    write::{SimpleFileOptions, ZipWriter},
};

const DOCUMENT_XML: &str = "word/document.xml";
const TABLE_PROPERTIES_START: &str = "<w:tblPr>";
const TABLE_PROPERTIES_END: &str = "</w:tblPr>";
const FIXED_LAYOUT: &str = "<w:tblLayout w:type=\"fixed\"/>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowHeightRule {
    AtLeast,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RowHeightPatch {
    cp_start: u32,
    height_twips: u16,
    rule: RowHeightRule,
}

pub(crate) fn add_table_layout_to_docx(
    docx: Vec<u8>,
    document: &WordBinaryDocument,
    tables: &TableCollection,
) -> Result<Vec<u8>, String> {
    if tables.is_empty() {
        return Ok(docx);
    }
    let fixed = tables
        .tables()
        .iter()
        .map(|table| {
            table
                .rows
                .first()
                .is_some_and(|row| !row.properties.autofit.unwrap_or(false))
        })
        .collect::<Vec<_>>();
    let styled = document
        .styled_formatting(DocLimits::default())
        .map_err(|error| format!("DOC table formatting failed: {error}"))?;
    let mut row_heights = tables
        .tables()
        .iter()
        .flat_map(|table| &table.rows)
        .filter_map(|row| {
            row.properties
                .row_height_twips
                .filter(|height| *height != 0)
                .map(|height| RowHeightPatch {
                    cp_start: row.cp_start,
                    height_twips: height.unsigned_abs(),
                    rule: if height < 0 || row_content_fits_minimum_height(document, &styled, row) {
                        RowHeightRule::Exact
                    } else {
                        RowHeightRule::AtLeast
                    },
                })
        })
        .collect::<Vec<_>>();
    row_heights.sort_by_key(|patch| patch.cp_start);
    patch_package(docx, &fixed, &row_heights)
}

/// Word's binary layout treats a positive row height as a minimum, but rows
/// whose content already fits that minimum are effectively fixed-height. Some
/// OOXML layout engines add rounding slack to every `atLeast` row; across a
/// long legacy table that can move otherwise fitting content to another page.
/// Emit `exact` only when a conservative source-backed width/height estimate
/// proves that every cell fits on one line. Rows that may wrap retain the
/// normative `atLeast` rule.
fn row_content_fits_minimum_height(
    document: &WordBinaryDocument,
    styled: &StyledFormattingIndex,
    row: &TableRow,
) -> bool {
    let Some(source_height) = row.properties.row_height_twips.filter(|height| *height > 0) else {
        return false;
    };
    let Some(definition) = &row.properties.definition else {
        return false;
    };
    row.cells.iter().all(|cell| {
        let index = usize::from(cell.index);
        let Some((left, right)) = definition
            .cell_edges_twips
            .get(index)
            .zip(definition.cell_edges_twips.get(index + 1))
        else {
            return false;
        };
        let width = i32::from(*right) - i32::from(*left);
        let margins = i32::from(row.properties.default_cell_margins.left)
            + i32::from(row.properties.default_cell_margins.right);
        let available = width - margins;
        if available <= 0 {
            return false;
        }
        let Ok(decoded) = document.decode_range(cell.cp_start, cell.cp_content_end) else {
            return false;
        };
        let text = decoded.text.trim_matches(['\r', '\u{7}', ' ']);
        if text.contains(['\r', '\n', '\u{b}', '\t', '\u{1}']) {
            return false;
        }
        let runs = styled
            .character_runs
            .iter()
            .filter(|run| run.cp_start < cell.cp_content_end && run.cp_end > cell.cp_start);
        let mut max_half_points = 22_u16;
        let mut max_spacing_twips = 0_i16;
        for run in runs {
            max_half_points =
                max_half_points.max(run.properties.font_size_half_points.unwrap_or(22));
            max_spacing_twips =
                max_spacing_twips.max(run.properties.character_spacing_twips.unwrap_or_default());
        }
        let em_twips = f64::from(max_half_points) * 10.0;
        let line_height = em_twips * 1.2;
        if line_height > f64::from(source_height) {
            return false;
        }
        let glyph_width = text.chars().map(relative_glyph_width).sum::<f64>() * em_twips;
        let gap_count = u32::try_from(text.chars().count().saturating_sub(1)).unwrap_or(u32::MAX);
        let spacing = f64::from(max_spacing_twips.max(0)) * f64::from(gap_count);
        // The generic glyph factors intentionally err high; allow only a small
        // tolerance for real font metrics and integer twip rounding.
        glyph_width + spacing <= f64::from(available) * 1.05
    })
}

fn relative_glyph_width(character: char) -> f64 {
    if character.is_whitespace() {
        0.33
    } else if character.is_ascii_digit() {
        0.5
    } else if character.is_ascii_punctuation() {
        0.4
    } else if character.is_ascii_lowercase() {
        0.5
    } else if character.is_ascii_uppercase() {
        0.6
    } else if matches!(character as u32, 0x2E80..=0xD7AF | 0xF900..=0xFAFF) {
        1.0
    } else {
        0.6
    }
}

#[derive(Debug)]
struct PackageEntry {
    name: String,
    compression: CompressionMethod,
    data: Vec<u8>,
}

fn patch_package(
    docx: Vec<u8>,
    fixed: &[bool],
    row_heights: &[RowHeightPatch],
) -> Result<Vec<u8>, String> {
    let mut archive = ZipArchive::new(Cursor::new(docx)).map_err(|error| error.to_string())?;
    let mut entries = Vec::with_capacity(archive.len());
    let mut found_document = false;
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
        if name == DOCUMENT_XML {
            found_document = true;
            data = patch_document_xml(&data, fixed, row_heights)?;
        }
        entries.push(PackageEntry {
            name,
            compression,
            data,
        });
    }
    if !found_document {
        return Err("generated DOCX is missing word/document.xml".into());
    }
    write_package(entries)
}

fn patch_document_xml(
    data: &[u8],
    fixed: &[bool],
    row_heights: &[RowHeightPatch],
) -> Result<Vec<u8>, String> {
    let xml = String::from_utf8(data.to_vec())
        .map_err(|_| "generated word/document.xml is not UTF-8".to_string())?;
    let mut output = String::with_capacity(xml.len() + fixed.len() * FIXED_LAYOUT.len());
    let mut remaining = xml.as_str();
    for (table_index, fixed_layout) in fixed.iter().copied().enumerate() {
        let start = remaining.find(TABLE_PROPERTIES_START).ok_or_else(|| {
            format!(
                "generated document.xml has fewer table properties than source table {table_index}"
            )
        })?;
        let after_start = start + TABLE_PROPERTIES_START.len();
        let relative_end = remaining[after_start..]
            .find(TABLE_PROPERTIES_END)
            .ok_or_else(|| format!("generated table {table_index} has unclosed w:tblPr"))?;
        let end = after_start + relative_end;
        let table_properties = &remaining[after_start..end];
        if table_properties.contains("<w:tblLayout") {
            return Err(format!(
                "generated table {table_index} unexpectedly already contains w:tblLayout"
            ));
        }
        output.push_str(&remaining[..end]);
        if fixed_layout {
            output.push_str(FIXED_LAYOUT);
        }
        remaining = &remaining[end..];
    }
    if remaining.contains(TABLE_PROPERTIES_START) {
        return Err("generated document.xml has more tables than the source table model".into());
    }
    output.push_str(remaining);
    patch_row_heights(output, row_heights).map(String::into_bytes)
}

fn patch_row_heights(mut xml: String, row_heights: &[RowHeightPatch]) -> Result<String, String> {
    let mut search_start = 0_usize;
    for (row_index, patch) in row_heights.iter().enumerate() {
        let relative_start = xml[search_start..].find("<w:trHeight ").ok_or_else(|| {
            format!("generated document.xml has fewer row heights than source row {row_index}")
        })?;
        let start = search_start + relative_start;
        let relative_end = xml[start..]
            .find("/>")
            .ok_or_else(|| format!("generated row height {row_index} is unclosed"))?;
        let end = start + relative_end + 2;
        let height = patch.height_twips;
        let element = &xml[start..end];
        if !element.contains(&format!("w:val=\"{height}\"")) {
            return Err(format!(
                "generated row height {row_index} does not match source value {height}"
            ));
        }
        if element.contains("w:hRule=") {
            return Err(format!(
                "generated row height {row_index} unexpectedly already contains w:hRule"
            ));
        }
        let rule = match patch.rule {
            RowHeightRule::Exact => "exact",
            RowHeightRule::AtLeast => "atLeast",
        };
        let attribute = format!(" w:hRule=\"{rule}\"");
        xml.insert_str(end - 2, &attribute);
        search_start = end + attribute.len();
    }
    if xml[search_start..].contains("<w:trHeight ") {
        return Err(
            "generated document.xml has more row heights than the source table model".into(),
        );
    }
    Ok(xml)
}

fn write_package(entries: Vec<PackageEntry>) -> Result<Vec<u8>, String> {
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut output);
        for entry in entries {
            let options = SimpleFileOptions::default().compression_method(entry.compression);
            writer
                .start_file(entry.name, options)
                .map_err(|error| error.to_string())?;
            writer
                .write_all(&entry.data)
                .map_err(|error| error.to_string())?;
        }
        writer.finish().map_err(|error| error.to_string())?;
    }
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_fixed_layout_only_to_source_fixed_tables() {
        let document = concat!(
            "<w:document><w:body>",
            "<w:tbl><w:tblPr><w:tblW/></w:tblPr></w:tbl>",
            "<w:tbl><w:tblPr><w:tblW/></w:tblPr></w:tbl>",
            "</w:body></w:document>"
        );
        let patched = String::from_utf8(
            patch_document_xml(document.as_bytes(), &[true, false], &[]).unwrap(),
        )
        .unwrap();
        assert_eq!(patched.matches(FIXED_LAYOUT).count(), 1);
        assert!(patched.contains(&format!("<w:tblW/>{FIXED_LAYOUT}</w:tblPr>")));
    }

    #[test]
    fn rejects_table_count_mismatches_and_existing_layout() {
        let one = b"<w:document><w:tbl><w:tblPr></w:tblPr></w:tbl></w:document>";
        assert!(patch_document_xml(one, &[true, false], &[]).is_err());
        assert!(patch_document_xml(one, &[], &[]).is_err());
        let existing = b"<w:document><w:tbl><w:tblPr><w:tblLayout w:type=\"fixed\"/></w:tblPr></w:tbl></w:document>";
        assert!(patch_document_xml(existing, &[true], &[]).is_err());
    }

    #[test]
    fn adds_source_row_height_rules() {
        let document = concat!(
            "<w:document><w:body><w:tbl><w:tblPr></w:tblPr>",
            "<w:tr><w:trPr><w:trHeight w:val=\"303\"/></w:trPr></w:tr>",
            "<w:tr><w:trPr><w:trHeight w:val=\"504\"/></w:trPr></w:tr>",
            "</w:tbl></w:body></w:document>"
        );
        let patched = String::from_utf8(
            patch_document_xml(
                document.as_bytes(),
                &[true],
                &[
                    RowHeightPatch {
                        cp_start: 1,
                        height_twips: 303,
                        rule: RowHeightRule::AtLeast,
                    },
                    RowHeightPatch {
                        cp_start: 2,
                        height_twips: 504,
                        rule: RowHeightRule::Exact,
                    },
                ],
            )
            .unwrap(),
        )
        .unwrap();
        assert!(patched.contains("w:val=\"303\" w:hRule=\"atLeast\""));
        assert!(patched.contains("w:val=\"504\" w:hRule=\"exact\""));
    }

    #[test]
    fn estimates_common_legacy_glyph_widths_conservatively() {
        for (character, expected) in [('1', 0.5), ('W', 0.6), (' ', 0.33), ('界', 1.0)] {
            assert!((relative_glyph_width(character) - expected).abs() < f64::EPSILON);
        }
    }
}
