//! OOXML comment parts added after the shared writer serializes its IR.

use std::{
    fmt::Write as _,
    io::{Cursor, Read, Write as _},
};

use legacy_doc::{CommentCollection, WordBinaryDocument};
use office_oxide::core::{
    content_types::{ContentTypes, ContentTypesBuilder},
    opc::PartName,
    relationships::{Relationships, RelationshipsBuilder, TargetMode, rel_types},
};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::{CompressionMethod, ZipArchive};

const COMMENTS_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";
const DOCUMENT_XML: &str = "word/document.xml";
const DOCUMENT_RELS: &str = "word/_rels/document.xml.rels";
const CONTENT_TYPES: &str = "[Content_Types].xml";
const COMMENTS_XML: &str = "word/comments.xml";
const COMMENT_REL_ID: &str = "rIdZrimoComments";

pub(crate) fn add_comments_to_docx(
    docx: Vec<u8>,
    document: &WordBinaryDocument,
    comments: &CommentCollection,
) -> Result<Vec<u8>, String> {
    if comments.comments().is_empty() {
        return Ok(docx);
    }
    let projected = project_comments(document, comments)?;
    patch_package(docx, &projected)
}

#[derive(Debug)]
struct ProjectedComment {
    id: u32,
    author: String,
    initials: String,
    marker: String,
    range_markers: Option<(String, String)>,
    body: Vec<u16>,
}

fn project_comments(
    document: &WordBinaryDocument,
    comments: &CommentCollection,
) -> Result<Vec<ProjectedComment>, String> {
    comments
        .comments()
        .iter()
        .map(|comment| {
            let content_end = comment
                .cp_end
                .checked_sub(1)
                .ok_or_else(|| format!("comment {} body end underflows", comment.comment_id))?;
            let body = document
                .decode_range(comment.content_cp_start(), content_end)
                .map_err(|error| error.to_string())?
                .utf16;
            Ok(ProjectedComment {
                id: comment.comment_id,
                author: comment.author.clone(),
                initials: comment.initials.clone(),
                marker: comment.projection_marker(),
                range_markers: match (comment.anchor_cp_start, comment.anchor_cp_end) {
                    (Some(_), Some(_)) => Some((
                        comment.range_start_projection_marker(),
                        comment.range_end_projection_marker(),
                    )),
                    (None, None) => None,
                    _ => {
                        return Err(format!(
                            "comment {} has an incomplete source anchor range",
                            comment.comment_id
                        ));
                    }
                },
                body,
            })
        })
        .collect()
}

#[derive(Debug)]
struct PackageEntry {
    name: String,
    compression: CompressionMethod,
    data: Vec<u8>,
}

fn patch_package(docx: Vec<u8>, comments: &[ProjectedComment]) -> Result<Vec<u8>, String> {
    let mut archive = ZipArchive::new(Cursor::new(docx)).map_err(|error| error.to_string())?;
    let mut entries = Vec::with_capacity(archive.len() + 1);
    let mut found_document = false;
    let mut found_content_types = false;
    let mut found_document_rels = false;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name == COMMENTS_XML {
            return Err("generated DOCX unexpectedly already contains word/comments.xml".into());
        }
        let compression = entry.compression();
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|error| error.to_string())?;
        data = match name.as_str() {
            DOCUMENT_XML => {
                found_document = true;
                patch_document_xml(&data, comments)?
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
    if !found_document_rels {
        entries.push(PackageEntry {
            name: DOCUMENT_RELS.into(),
            compression: CompressionMethod::Deflated,
            data: new_document_relationships(),
        });
    }
    entries.push(PackageEntry {
        name: COMMENTS_XML.into(),
        compression: CompressionMethod::Deflated,
        data: comments_xml(comments).into_bytes(),
    });
    write_package(entries)
}

fn patch_document_xml(data: &[u8], comments: &[ProjectedComment]) -> Result<Vec<u8>, String> {
    let mut xml = String::from_utf8(data.to_vec())
        .map_err(|_| "generated word/document.xml is not UTF-8".to_string())?;
    for comment in comments {
        if let Some((start, end)) = &comment.range_markers {
            replace_comment_marker(
                &mut xml,
                start,
                &format!("<w:commentRangeStart w:id=\"{}\"/>", comment.id),
                comment.id,
                "range start",
            )?;
            replace_comment_marker(
                &mut xml,
                end,
                &format!("<w:commentRangeEnd w:id=\"{}\"/>", comment.id),
                comment.id,
                "range end",
            )?;
        }
        replace_comment_marker(
            &mut xml,
            &comment.marker,
            &format!("<w:commentReference w:id=\"{}\"/>", comment.id),
            comment.id,
            "reference",
        )?;
    }
    if xml.contains("ZRIMO_COMMENT") {
        return Err("a DOC comment projection marker remained in document.xml".into());
    }
    Ok(xml.into_bytes())
}

fn replace_comment_marker(
    xml: &mut String,
    marker: &str,
    element: &str,
    comment_id: u32,
    kind: &str,
) -> Result<(), String> {
    let matches = xml.matches(marker).count();
    if matches != 1 {
        return Err(format!(
            "comment {comment_id} {kind} projection marker occurs {matches} times in document.xml"
        ));
    }
    let marker_offset = xml
        .find(marker)
        .ok_or_else(|| format!("comment {comment_id} {kind} marker disappeared"))?;
    let text_start = xml[..marker_offset]
        .rfind("<w:t")
        .ok_or_else(|| format!("comment {comment_id} {kind} marker is outside a w:t element"))?;
    if xml[text_start..marker_offset].contains("</w:t>")
        || !xml[marker_offset + marker.len()..].contains("</w:t>")
    {
        return Err(format!(
            "comment {comment_id} {kind} marker is not contained by one w:t element"
        ));
    }
    let replacement = format!("</w:t>{element}<w:t xml:space=\"preserve\">");
    xml.replace_range(marker_offset..marker_offset + marker.len(), &replacement);
    Ok(())
}

fn patch_content_types(data: &[u8]) -> Result<Vec<u8>, String> {
    let parsed = ContentTypes::parse(data).map_err(|error| error.to_string())?;
    let comments_part = PartName::new("/word/comments.xml").map_err(|error| error.to_string())?;
    if parsed.overrides().contains_key(&comments_part) {
        return Err("generated DOCX already declares a comments content type".into());
    }
    let mut builder = ContentTypesBuilder::new();
    for (extension, content_type) in parsed.defaults() {
        builder.add_default(extension, content_type);
    }
    for (part, content_type) in parsed.overrides() {
        builder.add_override(part.clone(), content_type);
    }
    builder.add_override(comments_part, COMMENTS_CONTENT_TYPE);
    Ok(builder.serialize())
}

fn patch_document_relationships(data: &[u8]) -> Result<Vec<u8>, String> {
    let parsed = Relationships::parse(data).map_err(|error| error.to_string())?;
    if parsed.get_by_id(COMMENT_REL_ID).is_some()
        || parsed.first_by_type(rel_types::COMMENTS).is_some()
    {
        return Err("generated DOCX already contains a comments relationship".into());
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
        COMMENT_REL_ID,
        rel_types::COMMENTS,
        "comments.xml",
        TargetMode::Internal,
    );
    Ok(builder.serialize())
}

fn new_document_relationships() -> Vec<u8> {
    let mut builder = RelationshipsBuilder::new();
    builder.add_with_id(
        COMMENT_REL_ID,
        rel_types::COMMENTS,
        "comments.xml",
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

fn comments_xml(comments: &[ProjectedComment]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:comments xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">",
    );
    for comment in comments {
        write!(
            xml,
            "<w:comment w:id=\"{}\" w:author=\"{}\" w:initials=\"{}\">",
            comment.id,
            escape_xml(&comment.author),
            escape_xml(&comment.initials),
        )
        .expect("writing XML into a String cannot fail");
        append_comment_body(&mut xml, &comment.body);
        xml.push_str("</w:comment>");
    }
    xml.push_str("</w:comments>");
    xml
}

fn append_comment_body(xml: &mut String, body: &[u16]) {
    for paragraph in body.split(|unit| *unit == 0x000D) {
        xml.push_str("<w:p><w:r>");
        let mut text = Vec::new();
        for unit in paragraph {
            match *unit {
                0x0009 => {
                    flush_comment_text(xml, &mut text);
                    xml.push_str("<w:tab/>");
                }
                0x000B => {
                    flush_comment_text(xml, &mut text);
                    xml.push_str("<w:br/>");
                }
                0x0020..=0xFFFF => text.push(*unit),
                _ => {}
            }
        }
        flush_comment_text(xml, &mut text);
        xml.push_str("</w:r></w:p>");
    }
}

fn flush_comment_text(xml: &mut String, text: &mut Vec<u16>) {
    if text.is_empty() {
        return;
    }
    let decoded = String::from_utf16_lossy(text);
    xml.push_str("<w:t xml:space=\"preserve\">");
    xml.push_str(&escape_xml(&decoded));
    xml.push_str("</w:t>");
    text.clear();
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
    fn serializes_comment_text_controls_and_escapes_metadata() {
        let comments = [ProjectedComment {
            id: 7,
            author: "A&B".into(),
            initials: "<AB>".into(),
            marker: "marker".into(),
            range_markers: None,
            body: "first\tline\u{000B}next\rsecond".encode_utf16().collect(),
        }];
        let xml = comments_xml(&comments);
        assert!(xml.contains("w:author=\"A&amp;B\""));
        assert!(xml.contains("w:initials=\"&lt;AB&gt;\""));
        assert!(xml.contains(
            "first</w:t><w:tab/><w:t xml:space=\"preserve\">line</w:t><w:br/><w:t xml:space=\"preserve\">next"
        ));
        assert!(xml.contains("</w:p><w:p>"));
    }

    #[test]
    fn replaces_ranged_comment_markers_with_ooxml_elements() {
        let start = "\u{F0000}ZRIMO_COMMENT_RANGE_START_00000007\u{F0001}";
        let end = "\u{F0000}ZRIMO_COMMENT_RANGE_END_00000007\u{F0001}";
        let reference = "\u{F0000}ZRIMO_COMMENT_00000007\u{F0001}";
        let document = format!(
            "<w:document><w:body><w:p><w:r><w:t>{start}text{end}{reference}</w:t></w:r></w:p></w:body></w:document>"
        );
        let projected = [ProjectedComment {
            id: 7,
            author: String::new(),
            initials: String::new(),
            marker: reference.into(),
            range_markers: Some((start.into(), end.into())),
            body: Vec::new(),
        }];
        let patched =
            String::from_utf8(patch_document_xml(document.as_bytes(), &projected).unwrap())
                .unwrap();
        assert!(patched.contains("<w:commentRangeStart w:id=\"7\"/>"));
        assert!(patched.contains("<w:commentRangeEnd w:id=\"7\"/>"));
        assert!(patched.contains("<w:commentReference w:id=\"7\"/>"));
        assert!(!patched.contains("ZRIMO_COMMENT"));
    }
}
