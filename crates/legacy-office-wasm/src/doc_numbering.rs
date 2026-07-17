//! OOXML numbering parts projected from Word Binary list tables.

use std::{
    collections::{BTreeSet, HashSet},
    fmt::Write as _,
    io::{Cursor, Read, Write as _},
};

use legacy_doc::{
    FontTable, ListCollection, ListFollow, ListLevel, ToggleValue, apply_character_sprms,
    apply_paragraph_sprms,
};
use office_oxide::core::{
    content_types::{ContentTypes, ContentTypesBuilder},
    opc::PartName,
    relationships::{Relationships, RelationshipsBuilder, TargetMode, rel_types},
};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::{CompressionMethod, ZipArchive};

const NUMBERING_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const DOCUMENT_XML: &str = "word/document.xml";
const DOCUMENT_RELS: &str = "word/_rels/document.xml.rels";
const CONTENT_TYPES: &str = "[Content_Types].xml";
const NUMBERING_XML: &str = "word/numbering.xml";
const NUMBERING_REL_ID: &str = "rIdDocsViewerWasmNumbering";
const MARKER_PREFIX: &str = "\u{F0000}DOCS_VIEWER_WASM_LIST_";
const MARKER_SUFFIX: char = '\u{F0001}';

pub(crate) fn add_numbering_to_docx(
    docx: Vec<u8>,
    lists: &ListCollection,
    fonts: &FontTable,
) -> Result<Vec<u8>, String> {
    if lists.overrides().is_empty() {
        return Ok(docx);
    }
    if !document_contains_list_markers(&docx)? {
        // Word commonly leaves unused list definitions in the table stream.
        // They must not make an otherwise list-free generated package grow a
        // numbering part or fail conversion.
        return Ok(docx);
    }
    patch_package(docx, lists, fonts)
}

fn document_contains_list_markers(docx: &[u8]) -> Result<bool, String> {
    let mut archive = ZipArchive::new(Cursor::new(docx)).map_err(|error| error.to_string())?;
    let mut document = archive
        .by_name(DOCUMENT_XML)
        .map_err(|_| "generated DOCX is missing word/document.xml".to_string())?;
    let mut xml = String::new();
    document
        .read_to_string(&mut xml)
        .map_err(|_| "generated word/document.xml is not UTF-8".to_string())?;
    Ok(!parse_markers(&xml)?.is_empty())
}

#[derive(Debug)]
struct PackageEntry {
    name: String,
    compression: CompressionMethod,
    data: Vec<u8>,
}

fn patch_package(
    docx: Vec<u8>,
    lists: &ListCollection,
    fonts: &FontTable,
) -> Result<Vec<u8>, String> {
    let mut archive = ZipArchive::new(Cursor::new(docx)).map_err(|error| error.to_string())?;
    let mut entries = Vec::with_capacity(archive.len() + 1);
    let mut found_document = false;
    let mut found_content_types = false;
    let mut found_document_rels = false;
    let mut used = BTreeSet::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name == NUMBERING_XML {
            return Err("generated DOCX unexpectedly already contains word/numbering.xml".into());
        }
        let compression = entry.compression();
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|error| error.to_string())?;
        data = match name.as_str() {
            DOCUMENT_XML => {
                found_document = true;
                let (patched, referenced) = patch_document_xml(&data, lists)?;
                used = referenced;
                patched
            }
            DOCUMENT_RELS => {
                found_document_rels = true;
                patch_document_relationships(&data)?
            }
            CONTENT_TYPES => {
                found_content_types = true;
                patch_content_types(&data)?
            }
            _ => data,
        };
        entries.push(PackageEntry {
            name,
            compression,
            data,
        });
    }
    if !found_document || !found_content_types {
        return Err("generated DOCX is missing document.xml or [Content_Types].xml".into());
    }
    if used.is_empty() {
        return Err(
            "DOC list tables were parsed but no projected list paragraphs were found".into(),
        );
    }
    if !found_document_rels {
        entries.push(PackageEntry {
            name: DOCUMENT_RELS.into(),
            compression: CompressionMethod::Deflated,
            data: new_document_relationships(),
        });
    }
    entries.push(PackageEntry {
        name: NUMBERING_XML.into(),
        compression: CompressionMethod::Deflated,
        data: numbering_xml(lists, fonts, &used)?.into_bytes(),
    });
    write_package(entries)
}

#[derive(Debug, Clone)]
struct Marker {
    start: usize,
    end: usize,
    raw: String,
    num_id: u32,
    level: u8,
}

fn patch_document_xml(
    data: &[u8],
    lists: &ListCollection,
) -> Result<(Vec<u8>, BTreeSet<u32>), String> {
    let mut xml = String::from_utf8(data.to_vec())
        .map_err(|_| "generated word/document.xml is not UTF-8".to_string())?;
    let mut markers = parse_markers(&xml)?;
    if markers.is_empty() {
        return Ok((data.to_vec(), BTreeSet::new()));
    }
    let mut unique = HashSet::with_capacity(markers.len());
    let mut used = BTreeSet::new();
    for marker in &markers {
        if !unique.insert(marker.raw.clone()) {
            return Err(format!(
                "duplicate DOC list projection marker {}",
                marker.raw
            ));
        }
        let list = lists
            .overrides()
            .get(usize::try_from(marker.num_id - 1).unwrap_or(usize::MAX))
            .ok_or_else(|| format!("list marker references missing numId {}", marker.num_id))?;
        let definition = lists.definition(list.lsid).ok_or_else(|| {
            format!(
                "numId {} references missing lsid {}",
                marker.num_id, list.lsid
            )
        })?;
        if usize::from(marker.level) >= definition.levels.len() {
            return Err(format!(
                "list marker numId {} references missing level {}",
                marker.num_id, marker.level
            ));
        }
        used.insert(marker.num_id);
    }
    markers.sort_by_key(|marker| std::cmp::Reverse(marker.start));
    for marker in markers {
        patch_marker(&mut xml, &marker)?;
    }
    if xml.contains(MARKER_PREFIX) {
        return Err("a DOC list projection marker remained in document.xml".into());
    }
    Ok((xml.into_bytes(), used))
}

fn parse_markers(xml: &str) -> Result<Vec<Marker>, String> {
    let mut result = Vec::new();
    let mut cursor = 0_usize;
    while let Some(relative) = xml[cursor..].find(MARKER_PREFIX) {
        let start = cursor + relative;
        let suffix_relative = xml[start..]
            .find(MARKER_SUFFIX)
            .ok_or_else(|| "unterminated DOC list projection marker".to_string())?;
        let end = start + suffix_relative + MARKER_SUFFIX.len_utf8();
        let raw = &xml[start..end];
        let body = raw
            .strip_prefix(MARKER_PREFIX)
            .and_then(|value| value.strip_suffix(MARKER_SUFFIX))
            .ok_or_else(|| "malformed DOC list projection marker framing".to_string())?;
        let mut parts = body.split('_');
        let cp = parse_hex(parts.next(), "CP")?;
        let num_id = parse_hex(parts.next(), "numId")?;
        let level = parse_hex(parts.next(), "level")?;
        if parts.next().is_some() || num_id == 0 || level > 8 {
            return Err(format!("malformed DOC list projection marker {raw}"));
        }
        let _ = cp;
        result.push(Marker {
            start,
            end,
            raw: raw.to_string(),
            num_id,
            level: u8::try_from(level).map_err(|_| "list level does not fit u8")?,
        });
        cursor = end;
    }
    Ok(result)
}

fn parse_hex(value: Option<&str>, label: &str) -> Result<u32, String> {
    u32::from_str_radix(
        value.ok_or_else(|| format!("DOC list marker has no {label}"))?,
        16,
    )
    .map_err(|_| format!("DOC list marker has invalid {label}"))
}

fn patch_marker(xml: &mut String, marker: &Marker) -> Result<(), String> {
    if xml.get(marker.start..marker.end) != Some(marker.raw.as_str()) {
        return Err(format!("DOC list marker {} moved unexpectedly", marker.raw));
    }
    let text_start = xml[..marker.start]
        .rfind("<w:t")
        .ok_or_else(|| format!("list marker {} is outside a w:t element", marker.raw))?;
    if xml[text_start..marker.start].contains("</w:t>") || !xml[marker.end..].contains("</w:t>") {
        return Err(format!(
            "list marker {} is not contained by one w:t element",
            marker.raw
        ));
    }
    let paragraph_start = xml[..marker.start]
        .rmatch_indices("<w:p")
        .find_map(|(offset, _)| {
            matches!(xml.as_bytes().get(offset + 4), Some(b'>' | b' ')).then_some(offset)
        })
        .ok_or_else(|| format!("list marker {} is outside a w:p element", marker.raw))?;
    if xml[paragraph_start..marker.start].contains("</w:p>") {
        return Err(format!(
            "list marker {} follows its paragraph end",
            marker.raw
        ));
    }
    let paragraph_open_end = xml[paragraph_start..]
        .find('>')
        .map(|offset| paragraph_start + offset + 1)
        .ok_or_else(|| "generated paragraph start tag is unterminated".to_string())?;
    let paragraph_end = xml[marker.end..]
        .find("</w:p>")
        .map(|offset| marker.end + offset)
        .ok_or_else(|| "generated list paragraph has no closing tag".to_string())?;
    if xml[paragraph_start..paragraph_end].contains("<w:numPr") {
        return Err(format!(
            "generated list paragraph for numId {} already contains numPr",
            marker.num_id
        ));
    }
    let num_pr = format!(
        "<w:numPr><w:ilvl w:val=\"{}\"/><w:numId w:val=\"{}\"/></w:numPr>",
        marker.level, marker.num_id
    );
    xml.replace_range(marker.start..marker.end, "");
    let ppr_close = xml[paragraph_open_end..marker.start]
        .rfind("</w:pPr>")
        .map(|offset| paragraph_open_end + offset);
    if let Some(offset) = ppr_close {
        xml.insert_str(offset, &num_pr);
    } else {
        xml.insert_str(paragraph_open_end, &format!("<w:pPr>{num_pr}</w:pPr>"));
    }
    Ok(())
}

fn numbering_xml(
    lists: &ListCollection,
    fonts: &FontTable,
    used: &BTreeSet<u32>,
) -> Result<String, String> {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:numbering xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">",
    );
    for num_id in used {
        let list = lists
            .overrides()
            .get(usize::try_from(*num_id - 1).unwrap_or(usize::MAX))
            .ok_or_else(|| format!("missing list override {num_id}"))?;
        let definition = lists
            .definition(list.lsid)
            .ok_or_else(|| format!("missing list definition {}", list.lsid))?;
        let abstract_id = num_id - 1;
        write!(
            xml,
            "<w:abstractNum w:abstractNumId=\"{abstract_id}\"><w:multiLevelType w:val=\"{}\"/>",
            if definition.simple {
                "singleLevel"
            } else if definition.hybrid {
                "hybridMultilevel"
            } else {
                "multilevel"
            }
        )
        .expect("writing XML into String cannot fail");
        for (level_index, base) in definition.levels.iter().enumerate() {
            let level_index = u8::try_from(level_index).unwrap_or(8);
            let replacement = list
                .levels
                .iter()
                .find(|item| item.level == level_index)
                .and_then(|item| item.formatting.as_ref());
            write_level(&mut xml, level_index, replacement.unwrap_or(base), fonts)?;
        }
        xml.push_str("</w:abstractNum>");
    }
    for num_id in used {
        let list = &lists.overrides()[usize::try_from(*num_id - 1).unwrap_or(usize::MAX)];
        write!(
            xml,
            "<w:num w:numId=\"{num_id}\"><w:abstractNumId w:val=\"{}\"/>",
            num_id - 1
        )
        .expect("writing XML into String cannot fail");
        for level in &list.levels {
            if let Some(start) = level.start_at {
                write!(
                    xml,
                    "<w:lvlOverride w:ilvl=\"{}\"><w:startOverride w:val=\"{start}\"/></w:lvlOverride>",
                    level.level
                )
                .expect("writing XML into String cannot fail");
            }
        }
        xml.push_str("</w:num>");
    }
    xml.push_str("</w:numbering>");
    Ok(xml)
}

fn write_level(
    xml: &mut String,
    level_index: u8,
    level: &ListLevel,
    fonts: &FontTable,
) -> Result<(), String> {
    let format = number_format(level.number_format)?;
    let start_at = if matches!(level.number_format, 23 | 0xFF) {
        1
    } else {
        level.start_at
    };
    write!(
        xml,
        "<w:lvl w:ilvl=\"{level_index}\"><w:start w:val=\"{start_at}\"/><w:numFmt w:val=\"{format}\"/>"
    )
    .expect("writing XML into String cannot fail");
    if level.no_restart && !matches!(level.number_format, 23 | 0xFF) {
        write!(xml, "<w:lvlRestart w:val=\"{}\"/>", level.restart_limit)
            .expect("writing XML into String cannot fail");
    }
    if level.legal_numbering {
        xml.push_str("<w:isLgl/>");
    }
    write!(
        xml,
        "<w:suff w:val=\"{}\"/><w:lvlText w:val=\"{}\"/><w:lvlJc w:val=\"{}\"/>",
        match level.follow {
            ListFollow::Tab => "tab",
            ListFollow::Space => "space",
            ListFollow::Nothing => "nothing",
        },
        escape_xml(&level.ooxml_level_text()),
        match level.justification {
            0 => "left",
            1 => "center",
            2 => "right",
            _ =>
                return Err(format!(
                    "invalid list justification {}",
                    level.justification
                )),
        },
    )
    .expect("writing XML into String cannot fail");
    append_level_paragraph_properties(xml, level)?;
    append_level_character_properties(xml, level, fonts)?;
    xml.push_str("</w:lvl>");
    Ok(())
}

fn append_level_paragraph_properties(xml: &mut String, level: &ListLevel) -> Result<(), String> {
    let properties = apply_paragraph_sprms(&level.paragraph_sprms).map_err(|e| e.to_string())?;
    let has_indent = properties.indent_left_twips.is_some()
        || properties.indent_right_twips.is_some()
        || properties.first_line_indent_twips.is_some();
    if !has_indent && properties.tab_changes.is_empty() {
        return Ok(());
    }
    xml.push_str("<w:pPr>");
    if !properties.tab_changes.is_empty() {
        xml.push_str("<w:tabs>");
        for change in &properties.tab_changes {
            for tab in &change.additions {
                write!(
                    xml,
                    "<w:tab w:val=\"{}\" w:leader=\"{}\" w:pos=\"{}\"/>",
                    match tab.alignment {
                        1 => "center",
                        2 => "right",
                        3 => "decimal",
                        4 => "bar",
                        _ => "left",
                    },
                    match tab.leader {
                        1 => "dot",
                        2 => "hyphen",
                        3 => "underscore",
                        4 => "heavy",
                        5 => "middleDot",
                        _ => "none",
                    },
                    tab.position_twips
                )
                .expect("writing XML into String cannot fail");
            }
        }
        xml.push_str("</w:tabs>");
    }
    if has_indent {
        xml.push_str("<w:ind");
        if let Some(value) = properties.indent_left_twips {
            write!(xml, " w:left=\"{value}\"").expect("writing XML cannot fail");
        }
        if let Some(value) = properties.indent_right_twips {
            write!(xml, " w:right=\"{value}\"").expect("writing XML cannot fail");
        }
        if let Some(value) = properties.first_line_indent_twips {
            if value < 0 {
                write!(xml, " w:hanging=\"{}\"", value.unsigned_abs())
                    .expect("writing XML cannot fail");
            } else {
                write!(xml, " w:firstLine=\"{value}\"").expect("writing XML cannot fail");
            }
        }
        xml.push_str("/>");
    }
    xml.push_str("</w:pPr>");
    Ok(())
}

fn append_level_character_properties(
    xml: &mut String,
    level: &ListLevel,
    fonts: &FontTable,
) -> Result<(), String> {
    let properties = apply_character_sprms(&level.character_sprms).map_err(|e| e.to_string())?;
    let font = properties
        .font_ascii
        .or(properties.font_other)
        .and_then(|index| fonts.get(index));
    let bold = explicit_toggle(properties.bold);
    let italic = explicit_toggle(properties.italic);
    if font.is_none()
        && bold.is_none()
        && italic.is_none()
        && properties.font_size_half_points.is_none()
    {
        return Ok(());
    }
    xml.push_str("<w:rPr>");
    if let Some(font) = font {
        let name = escape_xml(&font.name);
        write!(
            xml,
            "<w:rFonts w:ascii=\"{name}\" w:hAnsi=\"{name}\" w:cs=\"{name}\"/>"
        )
        .expect("writing XML cannot fail");
    }
    if let Some(value) = bold {
        xml.push_str(if value {
            "<w:b/>"
        } else {
            "<w:b w:val=\"0\"/>"
        });
    }
    if let Some(value) = italic {
        xml.push_str(if value {
            "<w:i/>"
        } else {
            "<w:i w:val=\"0\"/>"
        });
    }
    if let Some(size) = properties.font_size_half_points {
        write!(xml, "<w:sz w:val=\"{size}\"/>").expect("writing XML cannot fail");
    }
    xml.push_str("</w:rPr>");
    Ok(())
}

const fn explicit_toggle(value: Option<ToggleValue>) -> Option<bool> {
    match value {
        Some(ToggleValue::On) => Some(true),
        Some(ToggleValue::Off) => Some(false),
        Some(ToggleValue::SameAsStyle | ToggleValue::OppositeStyle) | None => None,
    }
}

fn number_format(value: u8) -> Result<&'static str, String> {
    Ok(match value {
        0 => "decimal",
        1 => "upperRoman",
        2 => "lowerRoman",
        3 => "upperLetter",
        4 => "lowerLetter",
        5 => "ordinal",
        6 => "cardinalText",
        7 => "ordinalText",
        10 => "aiueo",
        11 => "iroha",
        12 => "decimalFullWidth",
        13 => "decimalHalfWidth",
        14 => "japaneseCounting",
        16 => "decimalEnclosedCircle",
        17 => "decimalFullWidth2",
        18 => "aiueoFullWidth",
        20 => "irohaFullWidth",
        21 => "decimalZero",
        23 => "bullet",
        24 => "ganada",
        25 => "chosung",
        26 => "decimalEnclosedFullstop",
        27 => "decimalEnclosedParen",
        28 => "decimalEnclosedCircleChinese",
        29 => "ideographEnclosedCircle",
        30 => "ideographTraditional",
        31 => "ideographZodiac",
        32 => "ideographZodiacTraditional",
        33 => "taiwaneseCounting",
        34 => "ideographLegalTraditional",
        35 => "taiwaneseCountingThousand",
        36 => "taiwaneseDigital",
        37 => "chineseCounting",
        38 => "chineseLegalSimplified",
        39 => "chineseCountingThousand",
        41 => "koreanDigital",
        42 => "koreanCounting",
        43 => "koreanLegal",
        44 => "koreanDigital2",
        45 => "hebrew1",
        46 => "arabicAlpha",
        47 => "hebrew2",
        48 => "arabicAbjad",
        49 => "hindiVowels",
        50 => "hindiConsonants",
        51 => "hindiNumbers",
        52 => "hindiCounting",
        53 => "thaiLetters",
        54 => "thaiNumbers",
        55 => "thaiCounting",
        56 => "vietnameseCounting",
        57 => "numberInDash",
        58 => "russianLower",
        59 => "russianUpper",
        0xFF => "none",
        other => return Err(format!("unsupported Word list MSONFC {other:#04X}")),
    })
}

fn patch_content_types(data: &[u8]) -> Result<Vec<u8>, String> {
    let parsed = ContentTypes::parse(data).map_err(|error| error.to_string())?;
    let part = PartName::new("/word/numbering.xml").map_err(|error| error.to_string())?;
    if parsed.overrides().contains_key(&part) {
        return Err("generated DOCX already declares a numbering content type".into());
    }
    let mut builder = ContentTypesBuilder::new();
    for (extension, content_type) in parsed.defaults() {
        builder.add_default(extension, content_type);
    }
    for (name, content_type) in parsed.overrides() {
        builder.add_override(name.clone(), content_type);
    }
    builder.add_override(part, NUMBERING_CONTENT_TYPE);
    Ok(builder.serialize())
}

fn patch_document_relationships(data: &[u8]) -> Result<Vec<u8>, String> {
    let parsed = Relationships::parse(data).map_err(|error| error.to_string())?;
    if parsed.get_by_id(NUMBERING_REL_ID).is_some()
        || parsed.first_by_type(rel_types::NUMBERING).is_some()
    {
        return Err("generated DOCX already contains a numbering relationship".into());
    }
    let mut builder = RelationshipsBuilder::new();
    for relationship in parsed.all() {
        builder.add_with_id(
            &relationship.id,
            &relationship.rel_type,
            &relationship.target,
            relationship.target_mode,
        );
    }
    builder.add_with_id(
        NUMBERING_REL_ID,
        rel_types::NUMBERING,
        "numbering.xml",
        TargetMode::Internal,
    );
    Ok(builder.serialize())
}

fn new_document_relationships() -> Vec<u8> {
    let mut builder = RelationshipsBuilder::new();
    builder.add_with_id(
        NUMBERING_REL_ID,
        rel_types::NUMBERING,
        "numbering.xml",
        TargetMode::Internal,
    );
    builder.serialize()
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

    #[test]
    fn parses_projection_markers_and_maps_common_number_formats() {
        let marker = "<w:p><w:r><w:t>\u{F0000}DOCS_VIEWER_WASM_LIST_0000000A_00000002_03\u{F0001}item</w:t></w:r></w:p>";
        let parsed = parse_markers(marker).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].num_id, 2);
        assert_eq!(parsed[0].level, 3);
        assert_eq!(number_format(0).unwrap(), "decimal");
        assert_eq!(number_format(23).unwrap(), "bullet");
        assert!(number_format(8).is_err());
    }

    #[test]
    fn serializes_word_restart_boundary_and_normalizes_bullet_start() {
        let mut level = ListLevel {
            start_at: 4,
            number_format: 0,
            justification: 0,
            legal_numbering: false,
            no_restart: true,
            restart_limit: 1,
            follow: ListFollow::Space,
            placeholder_offsets: [1, 0, 0, 0, 0, 0, 0, 0, 0],
            level_text: vec![0, u16::from(b'.')],
            paragraph_sprms: Vec::new(),
            character_sprms: Vec::new(),
        };
        let mut xml = String::new();
        write_level(&mut xml, 2, &level, &FontTable::default()).unwrap();
        assert!(xml.contains("<w:start w:val=\"4\"/>"));
        assert!(xml.contains("<w:lvlRestart w:val=\"1\"/>"));

        level.number_format = 23;
        level.level_text = vec![0xF0B7];
        let mut bullet = String::new();
        write_level(&mut bullet, 2, &level, &FontTable::default()).unwrap();
        assert!(bullet.contains("<w:start w:val=\"1\"/>"));
        assert!(!bullet.contains("<w:lvlRestart"));
    }
}
