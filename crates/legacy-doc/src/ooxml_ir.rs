//! Projection of source-backed DOC structures into the shared OOXML IR.

use office_oxide::{
    DocumentFormat,
    ir::{
        BorderLine, BorderStyle, CellPadding, CellVerticalAlign, ColumnLayout, DocumentIR, Element,
        FootnoteRef, HeaderFooter, Image, ImageFormat, ImagePositioning, InlineContent,
        LineSpacing as IrLineSpacing, Metadata, Note as IrNote, PageSetup, Paragraph,
        ParagraphAlignment, Section as IrSection, SectionBreakType, Table as IrTable,
        TableAlignment, TableBorder, TableCell as IrTableCell, TableRow as IrTableRow,
        TextDirection, TextSpan, UnderlineStyle, VerticalAlign,
    },
};

use crate::{
    BlipFormat, CharacterPropertyDelta, CommentCollection, DocError, DocLimits, Field,
    FieldCollection, FontTable, HeaderFooterLink, HeaderFooterTable, HorizontalMerge,
    InlinePicture, MediaCollection, NoteCollection, NoteKind, PageOrientation,
    ParagraphPropertyDelta, Result, SectionGeometry, SemanticFormattingIndex, SourceNote,
    StyledCharacterRun, StyledFormattingIndex, StyledParagraphRun, Table as SourceTable,
    TableCollection, TableRowDefinition, ToggleValue, VerticalAlignment, VerticalMerge,
    WordBinaryDocument, apply_character_sprms,
};

/// Build a DOCX-oriented IR from source-proven Word Binary structures.
///
/// # Errors
///
/// Returns a typed parser/projection error instead of silently dropping a
/// malformed structure or an image format that cannot be represented safely.
pub fn build_ooxml_ir(document: &WordBinaryDocument, limits: DocLimits) -> Result<DocumentIR> {
    let styled = document.styled_formatting(limits)?;
    let semantic = &styled.direct;
    let fonts = document.fonts(limits)?;
    let source_sections = document.sections(limits)?;
    let tables = document.tables(limits)?;
    let headers = document.header_footers(limits)?;
    let media = document.media(limits)?;
    let notes = document.notes(limits)?;
    let comments = document.comments(limits)?;
    let lists = document.lists(limits)?;
    let fields = document.fields(limits)?;
    let main = document
        .story(crate::StoryKind::Main)
        .ok_or_else(|| DocError::InvalidFormatting("main story is absent".into()))?;
    let context = ProjectionContext {
        document,
        styled: &styled,
        semantic,
        fonts: &fonts,
        tables: &tables,
        media: &media,
        notes: &notes,
        comments: &comments,
        lists: &lists,
        fields: &fields,
    };

    let mut sections = Vec::new();
    if source_sections.is_empty() {
        sections.push(project_section(
            &context,
            0,
            main.cp_start,
            main.cp_end,
            &SectionGeometry::default(),
            &headers,
        )?);
    } else {
        for (index, section) in source_sections.sections().iter().enumerate() {
            sections.push(project_section(
                &context,
                index,
                section.cp_start,
                section.cp_end,
                &section.geometry,
                &headers,
            )?);
        }
    }
    if let Some(section) = sections.last_mut() {
        section.elements.extend(project_notes(&context)?);
    }
    Ok(DocumentIR {
        metadata: Metadata {
            format: DocumentFormat::Doc,
            ..Default::default()
        },
        sections,
    })
}

impl WordBinaryDocument {
    /// Project this parsed DOC into the common OOXML writer IR.
    ///
    /// # Errors
    ///
    /// Returns a typed source parsing or projection error.
    pub fn to_ooxml_ir(&self, limits: DocLimits) -> Result<DocumentIR> {
        build_ooxml_ir(self, limits)
    }
}

struct ProjectionContext<'a> {
    document: &'a WordBinaryDocument,
    styled: &'a StyledFormattingIndex,
    semantic: &'a SemanticFormattingIndex,
    fonts: &'a FontTable,
    tables: &'a TableCollection,
    media: &'a MediaCollection,
    notes: &'a NoteCollection,
    comments: &'a CommentCollection,
    lists: &'a crate::ListCollection,
    fields: &'a FieldCollection,
}

fn project_notes(context: &ProjectionContext<'_>) -> Result<Vec<Element>> {
    context
        .notes
        .notes()
        .iter()
        .map(|note| project_note(context, note))
        .collect()
}

fn project_note(context: &ProjectionContext<'_>, source: &SourceNote) -> Result<Element> {
    let mut body = build_blocks(context, source.cp_start, source.cp_end, None)?;
    prepend_note_marker(&mut body, source.note_id);
    let note = IrNote {
        id: source.note_id,
        content: body,
        marker: None,
    };
    Ok(match source.kind {
        NoteKind::Footnote => Element::Footnote(note),
        NoteKind::Endnote => Element::Endnote(note),
    })
}

fn prepend_note_marker(content: &mut Vec<Element>, note_id: u32) {
    let marker = InlineContent::Text(TextSpan {
        text: note_id.to_string(),
        vertical_align: Some(VerticalAlign::Superscript),
        ..Default::default()
    });
    if let Some(Element::Paragraph(paragraph)) = content.first_mut() {
        paragraph.content.insert(0, marker);
    } else {
        content.insert(
            0,
            Element::Paragraph(Paragraph {
                content: vec![marker],
                ..Default::default()
            }),
        );
    }
}

fn project_section(
    context: &ProjectionContext<'_>,
    section_index: usize,
    cp_start: u32,
    cp_end: u32,
    geometry: &SectionGeometry,
    headers: &HeaderFooterTable,
) -> Result<IrSection> {
    let header_links = headers.sections().get(section_index);
    Ok(IrSection {
        title: None,
        elements: build_blocks(context, cp_start, cp_end, None)?,
        page_setup: Some(page_setup(geometry)),
        columns: column_layout(geometry),
        break_type: section_break(geometry.break_code),
        header: linked_header(
            context,
            headers,
            header_links.and_then(|value| value.odd_header),
        )?,
        footer: linked_header(
            context,
            headers,
            header_links.and_then(|value| value.odd_footer),
        )?,
        first_page_header: linked_header(
            context,
            headers,
            header_links.and_then(|value| value.first_header),
        )?,
        first_page_footer: linked_header(
            context,
            headers,
            header_links.and_then(|value| value.first_footer),
        )?,
        even_page_header: linked_header(
            context,
            headers,
            header_links.and_then(|value| value.even_header),
        )?,
        even_page_footer: linked_header(
            context,
            headers,
            header_links.and_then(|value| value.even_footer),
        )?,
        background_rgb: None,
    })
}

fn linked_header(
    context: &ProjectionContext<'_>,
    headers: &HeaderFooterTable,
    link: Option<HeaderFooterLink>,
) -> Result<Option<HeaderFooter>> {
    let Some(link) = link else {
        return Ok(None);
    };
    let story = headers.stories().get(link.story_index).ok_or_else(|| {
        DocError::InvalidHeaderFooter(format!(
            "resolved header/footer link {} is out of range",
            link.story_index
        ))
    })?;
    Ok(Some(HeaderFooter {
        content: build_blocks(context, story.cp_start, story.cp_content_end, Some(0))?,
    }))
}

fn build_blocks(
    context: &ProjectionContext<'_>,
    cp_start: u32,
    cp_end: u32,
    containing_table_depth: Option<u32>,
) -> Result<Vec<Element>> {
    let nested_depth = containing_table_depth.map_or(1, |depth| depth + 1);
    let mut source_tables = context
        .tables
        .tables()
        .iter()
        .filter(|table| {
            table.depth == nested_depth && table.cp_start >= cp_start && table.cp_end <= cp_end
        })
        .collect::<Vec<_>>();
    source_tables.sort_by_key(|table| table.cp_start);

    let mut elements = Vec::new();
    let mut table_index = 0_usize;
    for (paragraph_start, paragraph_end, paragraph) in
        logical_paragraphs(context, cp_start, cp_end)?
    {
        while let Some(table) = source_tables.get(table_index)
            && table.cp_end <= paragraph_start
        {
            elements.push(Element::Table(project_table(context, table)?));
            table_index += 1;
        }
        if let Some(table) = source_tables.get(table_index)
            && paragraph_start >= table.cp_start
            && paragraph_start < table.cp_end
        {
            continue;
        }
        if !paragraph_belongs_here(&paragraph.properties, containing_table_depth) {
            continue;
        }
        elements.extend(project_paragraph(
            context,
            paragraph,
            paragraph_start,
            paragraph_end,
        )?);
    }
    for table in source_tables.iter().skip(table_index) {
        elements.push(Element::Table(project_table(context, table)?));
    }
    if elements.is_empty() {
        elements.push(Element::Paragraph(Paragraph::default()));
    }
    Ok(elements)
}

fn logical_paragraphs<'a>(
    context: &'a ProjectionContext<'_>,
    cp_start: u32,
    cp_end: u32,
) -> Result<Vec<(u32, u32, &'a StyledParagraphRun)>> {
    let decoded = context.document.decode_range(cp_start, cp_end)?;
    let mut ranges = Vec::new();
    let mut start = cp_start;
    for (index, unit) in decoded.utf16.iter().enumerate() {
        if !matches!(*unit, 0x000D | 0x0007) {
            continue;
        }
        let end = cp_start
            .checked_add(u32::try_from(index + 1).map_err(|_| {
                DocError::InvalidFormatting("logical paragraph offset does not fit u32".into())
            })?)
            .ok_or_else(|| DocError::InvalidFormatting("logical paragraph CP overflow".into()))?;
        ranges.push(logical_paragraph(context, start, end)?);
        start = end;
    }
    if start < cp_end {
        ranges.push(logical_paragraph(context, start, cp_end)?);
    }
    Ok(ranges)
}

fn logical_paragraph<'a>(
    context: &'a ProjectionContext<'_>,
    cp_start: u32,
    cp_end: u32,
) -> Result<(u32, u32, &'a StyledParagraphRun)> {
    let property_cp = cp_end.saturating_sub(1);
    let source = context
        .styled
        .paragraph_runs
        .iter()
        .find(|run| run.cp_start <= property_cp && property_cp < run.cp_end)
        .or_else(|| {
            context
                .styled
                .paragraph_runs
                .iter()
                .rev()
                .find(|run| run.cp_start < cp_end && run.cp_end > cp_start)
        })
        .ok_or_else(|| {
            DocError::InvalidFormatting(format!(
                "logical paragraph [{cp_start}, {cp_end}) has no PAPX run"
            ))
        })?;
    Ok((cp_start, cp_end, source))
}

fn paragraph_belongs_here(
    properties: &ParagraphPropertyDelta,
    containing_table_depth: Option<u32>,
) -> bool {
    let mut depth = u32::try_from(properties.table_depth.unwrap_or_default()).unwrap_or_default();
    if depth == 0 && properties.in_table == Some(true) {
        depth = 1;
    }
    match containing_table_depth {
        None => depth == 0 && properties.in_table != Some(true),
        Some(expected) => depth == expected,
    }
}

fn project_paragraph(
    context: &ProjectionContext<'_>,
    paragraph: &StyledParagraphRun,
    cp_start: u32,
    mut cp_end: u32,
) -> Result<Vec<Element>> {
    if cp_start >= cp_end {
        return Ok(Vec::new());
    }
    let last = context.document.decode_range(cp_end - 1, cp_end)?;
    if matches!(last.utf16.first(), Some(0x000D | 0x0007)) {
        cp_end -= 1;
    }
    let pictures = context
        .media
        .pictures()
        .iter()
        .filter(|picture| picture.cp >= cp_start && picture.cp < cp_end)
        .collect::<Vec<_>>();
    let has_page_break = context
        .document
        .decode_range(cp_start, cp_end)?
        .utf16
        .contains(&0x000C);
    let mut elements = Vec::new();
    let mut segment_start = cp_start;
    for picture in pictures {
        let inlines = inline_content(context, segment_start, picture.cp)?;
        if !inlines.is_empty() || elements.is_empty() {
            elements.push(Element::Paragraph(paragraph_ir(paragraph, inlines)));
        }
        if let Some(image) = project_image(picture)? {
            elements.push(Element::Image(image));
        }
        segment_start = picture.cp + 1;
    }
    let inlines = inline_content(context, segment_start, cp_end)?;
    if !inlines.is_empty() || elements.is_empty() {
        elements.push(Element::Paragraph(paragraph_ir(paragraph, inlines)));
    }
    if has_page_break {
        elements.push(Element::PageBreak);
    }
    prepend_list_marker(context, paragraph, cp_start, &mut elements)?;
    Ok(elements)
}

fn prepend_list_marker(
    context: &ProjectionContext<'_>,
    paragraph: &StyledParagraphRun,
    cp: u32,
    elements: &mut [Element],
) -> Result<()> {
    let Some(ilfo) = paragraph.properties.list_id else {
        return Ok(());
    };
    let Some(list) = context.lists.resolve_paragraph(
        cp,
        ilfo,
        paragraph.properties.list_level.unwrap_or_default(),
    )?
    else {
        return Ok(());
    };
    let target = elements.iter_mut().find_map(|element| match element {
        Element::Paragraph(paragraph) => Some(paragraph),
        _ => None,
    });
    let Some(target) = target else {
        return Err(DocError::InvalidList(format!(
            "list paragraph at CP {cp} projected without a paragraph element"
        )));
    };
    target.content.insert(
        0,
        InlineContent::Text(TextSpan {
            text: list.projection_marker(),
            ..Default::default()
        }),
    );
    Ok(())
}

fn paragraph_ir(paragraph: &StyledParagraphRun, content: Vec<InlineContent>) -> Paragraph {
    let properties = &paragraph.properties;
    Paragraph {
        content,
        alignment: paragraph_alignment(properties.justification),
        indent_left_twips: properties.indent_left_twips.map(i32::from),
        indent_right_twips: properties.indent_right_twips.map(i32::from),
        first_line_indent_twips: properties.first_line_indent_twips.map(i32::from),
        // Word Binary defaults are zero paragraph spacing and single-line
        // leading. Emit them explicitly so DOCX consumer defaults cannot
        // substitute the newer 1.08/1.15-line Normal style.
        space_before_twips: Some(u32::from(properties.space_before_twips.unwrap_or(0))),
        space_after_twips: Some(u32::from(properties.space_after_twips.unwrap_or(0))),
        line_spacing: Some(properties.line_spacing.map_or(
            IrLineSpacing::Multiple(240),
            |spacing| {
                if spacing.multiple {
                    IrLineSpacing::Multiple(u32::from(spacing.value.unsigned_abs()))
                } else if spacing.value < 0 {
                    IrLineSpacing::Exact(u32::from(spacing.value.unsigned_abs()))
                } else {
                    IrLineSpacing::AtLeast(u32::from(spacing.value.unsigned_abs()))
                }
            },
        )),
        background_color: None,
        border: None,
        keep_with_next: properties.keep_with_next.unwrap_or(false),
        keep_together: properties.keep_together.unwrap_or(false),
        page_break_before: properties.page_break_before.unwrap_or(false),
        outline_level: properties.outline_level,
        frame_position: None,
    }
}

fn inline_content(
    context: &ProjectionContext<'_>,
    cp_start: u32,
    cp_end: u32,
) -> Result<Vec<InlineContent>> {
    let mut inlines = Vec::new();
    let mut cursor = cp_start;
    for field in context.fields.top_level_in(cp_start, cp_end) {
        if cursor < field.cp_begin {
            inlines.extend(inline_content_without_fields(
                context,
                cursor,
                field.cp_begin,
            )?);
        }
        append_field(context, field, &mut inlines)?;
        cursor = field.cp_end + 1;
    }
    if cursor < cp_end {
        inlines.extend(inline_content_without_fields(context, cursor, cp_end)?);
    }
    if context.comments.ranges_ending_at(cp_end).next().is_some()
        || context.comments.ranges_starting_at(cp_end).next().is_some()
    {
        let template = span_at(context, cp_end.saturating_sub(1))?;
        append_comment_range_events(context, cp_end, &template, &mut inlines);
    }
    Ok(inlines)
}

fn inline_content_without_fields(
    context: &ProjectionContext<'_>,
    cp_start: u32,
    cp_end: u32,
) -> Result<Vec<InlineContent>> {
    if cp_start >= cp_end {
        return Ok(Vec::new());
    }
    let mut inlines = Vec::new();
    let mut cursor = cp_start;
    for run in &context.styled.character_runs {
        let start = run.cp_start.max(cp_start);
        let end = run.cp_end.min(cp_end);
        if start >= end {
            continue;
        }
        if cursor < start {
            append_plain_text(context, cursor, start, &mut inlines)?;
        }
        append_styled_text(context, run, start, end, &mut inlines)?;
        cursor = end;
    }
    if cursor < cp_end {
        append_plain_text(context, cursor, cp_end, &mut inlines)?;
    }
    Ok(inlines)
}

fn append_field(
    context: &ProjectionContext<'_>,
    field: Field,
    output: &mut Vec<InlineContent>,
) -> Result<()> {
    let instruction = context
        .document
        .decode_range(field.instruction_start(), field.instruction_end())?;
    let keyword = instruction
        .text
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if matches!(keyword.as_str(), "PAGE" | "NUMPAGES") {
        let mut span = span_at(context, field.cp_begin)?;
        span.text = format!("{{{keyword}}}");
        output.push(InlineContent::Text(span));
    } else if let Some((result_start, result_end)) = field.result_range() {
        output.extend(inline_content_without_fields(
            context,
            result_start,
            result_end,
        )?);
    }
    Ok(())
}

fn span_at(context: &ProjectionContext<'_>, cp: u32) -> Result<TextSpan> {
    let Some(run) = context
        .styled
        .character_runs
        .iter()
        .find(|run| run.cp_start <= cp && cp < run.cp_end)
    else {
        return Ok(TextSpan::default());
    };
    let style = style_character_properties(context, run)?;
    let direct = &context.semantic.character_runs[run.direct_run_index].properties;
    Ok(text_span(context, run, &style, direct, FontSlot::Ascii))
}

fn append_plain_text(
    context: &ProjectionContext<'_>,
    cp_start: u32,
    cp_end: u32,
    output: &mut Vec<InlineContent>,
) -> Result<()> {
    let decoded = context.document.decode_range(cp_start, cp_end)?;
    append_units(
        context,
        cp_start,
        &decoded.utf16,
        &TextSpan::default(),
        output,
    );
    Ok(())
}

fn append_styled_text(
    context: &ProjectionContext<'_>,
    run: &StyledCharacterRun,
    cp_start: u32,
    cp_end: u32,
    output: &mut Vec<InlineContent>,
) -> Result<()> {
    let style_properties = style_character_properties(context, run)?;
    let direct = &context.semantic.character_runs[run.direct_run_index].properties;
    if resolve_toggle(
        direct.hidden,
        style_properties.hidden,
        run.properties.hidden,
    ) {
        return Ok(());
    }
    let decoded = context.document.decode_range(cp_start, cp_end)?;
    let mut start = 0_usize;
    while start < decoded.utf16.len() {
        let slot = font_slot(decoded.utf16[start]);
        let mut end = start + 1;
        while end < decoded.utf16.len() && font_slot(decoded.utf16[end]) == slot {
            end += 1;
        }
        let span = text_span(context, run, &style_properties, direct, slot);
        append_units(
            context,
            cp_start + u32::try_from(start).unwrap_or(u32::MAX),
            &decoded.utf16[start..end],
            &span,
            output,
        );
        start = end;
    }
    Ok(())
}

fn style_character_properties(
    context: &ProjectionContext<'_>,
    run: &StyledCharacterRun,
) -> Result<CharacterPropertyDelta> {
    let direct_length = context.semantic.character_runs[run.direct_run_index]
        .sprms
        .len();
    let style_length = run.sprms.len().checked_sub(direct_length).ok_or_else(|| {
        DocError::InvalidFormatting("styled character run is shorter than its direct SPRMs".into())
    })?;
    apply_character_sprms(&run.sprms[..style_length])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FontSlot {
    Ascii,
    EastAsian,
    Bidi,
}

fn font_slot(unit: u16) -> FontSlot {
    if matches!(unit, 0x0590..=0x08FF | 0xFB1D..=0xFEFC) {
        FontSlot::Bidi
    } else if matches!(
        unit,
        0x2E80..=0xD7AF | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF
    ) {
        FontSlot::EastAsian
    } else {
        FontSlot::Ascii
    }
}

fn text_span(
    context: &ProjectionContext<'_>,
    run: &StyledCharacterRun,
    style: &CharacterPropertyDelta,
    direct: &CharacterPropertyDelta,
    slot: FontSlot,
) -> TextSpan {
    let properties = &run.properties;
    let bold = if slot == FontSlot::Bidi {
        resolve_toggle(
            direct.bidi_bold.or(direct.bold),
            style.bidi_bold.or(style.bold),
            properties.bidi_bold.or(properties.bold),
        )
    } else {
        resolve_toggle(direct.bold, style.bold, properties.bold)
    };
    let italic = if slot == FontSlot::Bidi {
        resolve_toggle(
            direct.bidi_italic.or(direct.italic),
            style.bidi_italic.or(style.italic),
            properties.bidi_italic.or(properties.italic),
        )
    } else {
        resolve_toggle(direct.italic, style.italic, properties.italic)
    };
    let font_index = match slot {
        FontSlot::Ascii => properties.font_ascii.or(properties.font_other),
        FontSlot::EastAsian => properties.font_east_asian.or(properties.font_ascii),
        FontSlot::Bidi => properties.font_bidi.or(properties.font_ascii),
    };
    TextSpan {
        text: String::new(),
        bold,
        italic,
        strikethrough: resolve_toggle(direct.strike, style.strike, properties.strike),
        hyperlink: None,
        font_size_half_pt: properties.font_size_half_points.map(u32::from),
        color: properties
            .color_ref
            .map(color_ref)
            .or_else(|| properties.color_index.and_then(indexed_color)),
        underline: properties.underline.and_then(underline_style),
        font_name: font_index
            .and_then(|index| context.fonts.get(index))
            .map(|font| font.name.clone()),
        highlight: properties.highlight_index.and_then(indexed_color),
        vertical_align: properties
            .vertical_alignment
            .map(|alignment| match alignment {
                VerticalAlignment::Superscript => VerticalAlign::Superscript,
                VerticalAlignment::Subscript => VerticalAlign::Subscript,
                VerticalAlignment::Baseline => VerticalAlign::Baseline,
            }),
        all_caps: resolve_toggle(direct.caps, style.caps, properties.caps),
        small_caps: resolve_toggle(direct.small_caps, style.small_caps, properties.small_caps),
        // `office_oxide` currently forwards this value directly to OOXML
        // `w:spacing`, whose unit is one twentieth of a point (a twip).
        char_spacing_half_pt: properties.character_spacing_twips.map(i32::from),
    }
}

fn resolve_toggle(
    direct: Option<ToggleValue>,
    style: Option<ToggleValue>,
    combined: Option<ToggleValue>,
) -> bool {
    let style_value = evaluate_toggle(style, false);
    direct.map_or_else(
        || evaluate_toggle(combined, false),
        |value| evaluate_toggle(Some(value), style_value),
    )
}

fn evaluate_toggle(value: Option<ToggleValue>, style: bool) -> bool {
    match value {
        Some(ToggleValue::On) => true,
        Some(ToggleValue::Off) | None => false,
        Some(ToggleValue::SameAsStyle) => style,
        Some(ToggleValue::OppositeStyle) => !style,
    }
}

fn append_units(
    context: &ProjectionContext<'_>,
    cp_start: u32,
    units: &[u16],
    template: &TextSpan,
    output: &mut Vec<InlineContent>,
) {
    let mut segment_start = 0_usize;
    for (index, _) in units.iter().enumerate() {
        let cp = cp_start.saturating_add(u32::try_from(index).unwrap_or(u32::MAX));
        let note = context.notes.reference_at(cp);
        let comment = context.comments.reference_at(cp);
        let has_range_event = context.comments.ranges_ending_at(cp).next().is_some()
            || context.comments.ranges_starting_at(cp).next().is_some();
        if note.is_none() && comment.is_none() && !has_range_event {
            continue;
        }
        append_text_units(&units[segment_start..index], template, output);
        append_comment_range_events(context, cp, template, output);
        if let Some(comment) = comment {
            let mut marker = template.clone();
            marker.text = comment.projection_marker();
            output.push(InlineContent::Text(marker));
        } else if let Some(reference) = note {
            output.push(match reference.kind {
                NoteKind::Footnote => InlineContent::FootnoteRef(FootnoteRef {
                    note_id: reference.note_id,
                    marker: None,
                }),
                NoteKind::Endnote => InlineContent::EndnoteRef(FootnoteRef {
                    note_id: reference.note_id,
                    marker: None,
                }),
            });
        }
        segment_start = if comment.is_some() || note.is_some() {
            index + 1
        } else {
            index
        };
    }
    append_text_units(&units[segment_start..], template, output);
}

fn append_comment_range_events(
    context: &ProjectionContext<'_>,
    cp: u32,
    template: &TextSpan,
    output: &mut Vec<InlineContent>,
) {
    for comment in context.comments.ranges_ending_at(cp).rev() {
        let mut marker = template.clone();
        marker.text = comment.range_end_projection_marker();
        output.push(InlineContent::Text(marker));
    }
    for comment in context.comments.ranges_starting_at(cp) {
        let mut marker = template.clone();
        marker.text = comment.range_start_projection_marker();
        output.push(InlineContent::Text(marker));
    }
}

fn append_text_units(units: &[u16], template: &TextSpan, output: &mut Vec<InlineContent>) {
    let mut buffered = Vec::new();
    let flush = |buffered: &mut Vec<u16>, output: &mut Vec<InlineContent>| {
        if buffered.is_empty() {
            return;
        }
        let mut span = template.clone();
        span.text = String::from_utf16_lossy(buffered);
        output.push(InlineContent::Text(span));
        buffered.clear();
    };
    for unit in units {
        match *unit {
            0x0009 => buffered.push(0x0009),
            0x000B => {
                flush(&mut buffered, output);
                output.push(InlineContent::LineBreak);
            }
            0x0020..=0xFFFF => buffered.push(*unit),
            _ => {}
        }
    }
    flush(&mut buffered, output);
}

fn project_table(context: &ProjectionContext<'_>, source: &SourceTable) -> Result<IrTable> {
    let definition = source
        .rows
        .first()
        .and_then(|row| row.properties.definition.as_ref())
        .ok_or_else(|| DocError::InvalidTable("table has no first-row definition".into()))?;
    let grid_edges = table_grid_edges(source)?;
    let column_widths_twips = grid_edges
        .windows(2)
        .map(|pair| u32::from((pair[1] - pair[0]).unsigned_abs()))
        .collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(source.rows.len());
    for (row_index, row) in source.rows.iter().enumerate() {
        let mut cells = Vec::new();
        let mut cell_index = 0_usize;
        while cell_index < row.cells.len() {
            let cell = &row.cells[cell_index];
            if cell.format.horizontal_merge == HorizontalMerge::Continuation {
                cell_index += 1;
                continue;
            }
            let mut source_span = 1_usize;
            if cell.format.horizontal_merge == HorizontalMerge::Start {
                while row.cells.get(cell_index + source_span).is_some_and(|next| {
                    next.format.horizontal_merge == HorizontalMerge::Continuation
                }) {
                    source_span += 1;
                }
            }
            let row_span = vertical_span(source, row_index, cell_index);
            let (col_span, width_twips) =
                cell_grid_geometry(row, row_index, cell_index, source_span, &grid_edges)?;
            cells.push(IrTableCell {
                content: build_blocks(
                    context,
                    cell.cp_start,
                    cell.cp_content_end,
                    Some(source.depth),
                )?,
                col_span,
                row_span,
                background_color: None,
                border: cell_border(&cell.format.borders),
                vertical_align: match cell.format.vertical_alignment {
                    0 => Some(CellVerticalAlign::Top),
                    1 => Some(CellVerticalAlign::Center),
                    2 => Some(CellVerticalAlign::Bottom),
                    _ => None,
                },
                text_align: None,
                width_twips,
                padding: Some(cell_padding(row.properties.default_cell_margins)),
                text_direction: (cell.format.text_flow != 0).then_some(TextDirection::TbRl),
            });
            cell_index += source_span;
        }
        rows.push(IrTableRow {
            cells,
            is_header: row.properties.header.unwrap_or(false),
            height_twips: row
                .properties
                .row_height_twips
                .filter(|height| *height != 0)
                .map(|height| u32::from(height.unsigned_abs())),
            allow_break: !row.properties.cant_split.unwrap_or(false),
            repeat_as_header: row.properties.header.unwrap_or(false),
        });
    }
    let width_twips = column_widths_twips
        .iter()
        .try_fold(0_u32, |total, width| total.checked_add(*width));
    Ok(assemble_table(
        source,
        definition,
        rows,
        column_widths_twips,
        width_twips,
    ))
}

fn cell_padding(margins: crate::CellMargins) -> CellPadding {
    CellPadding {
        top_twips: Some(u32::from(margins.top)),
        left_twips: Some(u32::from(margins.left)),
        bottom_twips: Some(u32::from(margins.bottom)),
        right_twips: Some(u32::from(margins.right)),
    }
}

fn cell_grid_geometry(
    row: &crate::TableRow,
    row_index: usize,
    cell_index: usize,
    source_span: usize,
    grid_edges: &[i16],
) -> Result<(u32, Option<u32>)> {
    let cell_edges = row
        .properties
        .definition
        .as_ref()
        .and_then(|value| {
            value
                .cell_edges_twips
                .get(cell_index..=cell_index.checked_add(source_span)?)
        })
        .and_then(|edges| edges.first().zip(edges.last()))
        .map(|(start, end)| (*start, *end))
        .ok_or_else(|| {
            DocError::InvalidTable(format!(
                "row {row_index} cell {cell_index} has no matching grid edges"
            ))
        })?;
    let grid_start = grid_edges.binary_search(&cell_edges.0).map_err(|_| {
        DocError::InvalidTable(format!(
            "row {row_index} cell {cell_index} start {} is outside the unified grid",
            cell_edges.0
        ))
    })?;
    let grid_end = grid_edges.binary_search(&cell_edges.1).map_err(|_| {
        DocError::InvalidTable(format!(
            "row {row_index} cell {cell_index} end {} is outside the unified grid",
            cell_edges.1
        ))
    })?;
    let col_span = u32::try_from(grid_end - grid_start)
        .map_err(|_| DocError::InvalidTable("table grid span does not fit u32".into()))?;
    let width = u32::from((cell_edges.1 - cell_edges.0).unsigned_abs());
    Ok((col_span, Some(width)))
}

fn table_grid_edges(source: &SourceTable) -> Result<Vec<i16>> {
    let mut edges = source
        .rows
        .iter()
        .filter_map(|row| row.properties.definition.as_ref())
        .flat_map(|definition| definition.cell_edges_twips.iter().copied())
        .collect::<Vec<_>>();
    edges.sort_unstable();
    edges.dedup();
    if edges.len() < 2 {
        return Err(DocError::InvalidTable(
            "table has fewer than two distinct grid edges".into(),
        ));
    }
    Ok(edges)
}

fn assemble_table(
    source: &SourceTable,
    definition: &TableRowDefinition,
    rows: Vec<IrTableRow>,
    column_widths_twips: Vec<u32>,
    width_twips: Option<u32>,
) -> IrTable {
    IrTable {
        rows,
        column_widths_twips,
        border: source.rows[0]
            .properties
            .borders80
            .as_ref()
            .and_then(table_border),
        alignment: match source.rows[0].properties.justification {
            Some(1) => Some(TableAlignment::Center),
            Some(2) => Some(TableAlignment::Right),
            _ => Some(TableAlignment::Left),
        },
        cell_padding_twips: None,
        caption: None,
        width_twips,
        indent_left_twips: definition.cell_edges_twips.first().copied().map(|edge| {
            i32::from(edge) + i32::from(source.rows[0].properties.gap_half_twips.unwrap_or(0))
        }),
    }
}

fn vertical_span(table: &SourceTable, row_index: usize, cell_index: usize) -> u32 {
    let Some(cell) = table.rows[row_index].cells.get(cell_index) else {
        return 1;
    };
    if !matches!(
        cell.format.vertical_merge,
        VerticalMerge::Start | VerticalMerge::StartAndEnd
    ) {
        return 1;
    }
    if cell.format.vertical_merge == VerticalMerge::StartAndEnd {
        return 1;
    }
    let mut span = 1_u32;
    for row in table.rows.iter().skip(row_index + 1) {
        if row
            .cells
            .get(cell_index)
            .is_some_and(|next| next.format.vertical_merge == VerticalMerge::Continuation)
        {
            span += 1;
        } else {
            break;
        }
    }
    span
}

fn cell_border(raw: &[[u8; 4]; 4]) -> Option<TableBorder> {
    let mut borders = raw.iter().copied().map(brc80);
    let top = borders.next().flatten();
    let left = borders.next().flatten();
    let bottom = borders.next().flatten();
    let right = borders.next().flatten();
    (top.is_some() || left.is_some() || bottom.is_some() || right.is_some()).then_some(
        TableBorder {
            top,
            bottom,
            left,
            right,
            inside_h: None,
            inside_v: None,
        },
    )
}

fn table_border(raw: &[[u8; 4]; 6]) -> Option<TableBorder> {
    let borders = raw.iter().copied().map(brc80).collect::<Vec<_>>();
    borders.iter().any(Option::is_some).then(|| TableBorder {
        top: borders[0].clone(),
        left: borders[1].clone(),
        bottom: borders[2].clone(),
        right: borders[3].clone(),
        inside_h: borders[4].clone(),
        inside_v: borders[5].clone(),
    })
}

fn brc80(raw: [u8; 4]) -> Option<BorderLine> {
    if raw == [0xFF; 4] || raw[1] == 0 {
        return None;
    }
    let style = match raw[1] {
        3 | 10 => BorderStyle::Double,
        6..=9 => BorderStyle::Dotted,
        11..=19 => BorderStyle::Dashed,
        _ => BorderStyle::Single,
    };
    Some(BorderLine {
        style,
        color: indexed_color(raw[2]),
        size: Some(u32::from(raw[0].max(1))),
        space: None,
    })
}

fn project_image(picture: &InlinePicture) -> Result<Option<Image>> {
    let Some(image) = picture
        .images
        .iter()
        .find(|image| image.format != BlipFormat::Pict)
    else {
        return Ok(None);
    };
    if image.compressed {
        return Ok(None);
    }
    let (format, data) = match image.format {
        BlipFormat::Emf => (ImageFormat::Emf, image.data.clone()),
        BlipFormat::Wmf => (ImageFormat::Wmf, image.data.clone()),
        BlipFormat::Jpeg => (ImageFormat::Jpeg, image.data.clone()),
        BlipFormat::Png => (ImageFormat::Png, image.data.clone()),
        BlipFormat::Tiff => (ImageFormat::Tiff, image.data.clone()),
        BlipFormat::Dib => (ImageFormat::Bmp, dib_to_bmp(&image.data)?),
        BlipFormat::Pict => unreachable!("PICT was filtered above"),
    };
    let width_twips = scaled_twips(
        picture.geometry.width_twips,
        picture.geometry.scale_x_tenths_percent,
    )?;
    let height_twips = scaled_twips(
        picture.geometry.height_twips,
        picture.geometry.scale_y_tenths_percent,
    )?;
    Ok(Some(Image {
        alt_text: None,
        data: Some(data),
        format: Some(format),
        display_width_emu: Some(width_twips * 635),
        display_height_emu: Some(height_twips * 635),
        pixel_width: None,
        pixel_height: None,
        decorative: false,
        positioning: ImagePositioning::Inline,
    }))
}

fn scaled_twips(goal: i16, scale: u16) -> Result<u64> {
    let product = u64::from(goal.unsigned_abs())
        .checked_mul(u64::from(scale))
        .ok_or_else(|| DocError::InvalidMedia("picture extent overflow".into()))?;
    Ok(product / 1000)
}

fn dib_to_bmp(dib: &[u8]) -> Result<Vec<u8>> {
    if dib.len() < 12 {
        return Err(DocError::InvalidMedia(
            "DIB is shorter than its header".into(),
        ));
    }
    let header_size = usize::try_from(u32::from_le_bytes(dib[0..4].try_into().unwrap()))
        .map_err(|_| DocError::InvalidMedia("DIB header size overflow".into()))?;
    if header_size > dib.len() {
        return Err(DocError::InvalidMedia("DIB header exceeds payload".into()));
    }
    let (bits_per_pixel, colors_used, palette_entry_size) = if header_size == 12 {
        (
            u16::from_le_bytes(dib[10..12].try_into().unwrap()),
            0_u32,
            3_usize,
        )
    } else if header_size >= 40 {
        (
            u16::from_le_bytes(dib[14..16].try_into().unwrap()),
            u32::from_le_bytes(dib[32..36].try_into().unwrap()),
            4_usize,
        )
    } else {
        return Err(DocError::InvalidMedia(format!(
            "unsupported DIB header size {header_size}"
        )));
    };
    let palette_entries = if colors_used != 0 {
        usize::try_from(colors_used).unwrap_or(usize::MAX)
    } else if bits_per_pixel <= 8 {
        1_usize << bits_per_pixel
    } else {
        0
    };
    let palette_bytes = palette_entries
        .checked_mul(palette_entry_size)
        .ok_or_else(|| DocError::InvalidMedia("DIB palette size overflow".into()))?;
    let pixel_offset = 14_usize
        .checked_add(header_size)
        .and_then(|value| value.checked_add(palette_bytes))
        .ok_or_else(|| DocError::InvalidMedia("BMP pixel offset overflow".into()))?;
    if pixel_offset - 14 > dib.len() {
        return Err(DocError::InvalidMedia("DIB palette exceeds payload".into()));
    }
    let file_size = 14_usize
        .checked_add(dib.len())
        .ok_or_else(|| DocError::InvalidMedia("BMP file size overflow".into()))?;
    let mut bmp = Vec::with_capacity(file_size);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(
        &u32::try_from(file_size)
            .map_err(|_| DocError::InvalidMedia("BMP file exceeds u32".into()))?
            .to_le_bytes(),
    );
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(
        &u32::try_from(pixel_offset)
            .map_err(|_| DocError::InvalidMedia("BMP pixel offset exceeds u32".into()))?
            .to_le_bytes(),
    );
    bmp.extend_from_slice(dib);
    Ok(bmp)
}

fn page_setup(geometry: &SectionGeometry) -> PageSetup {
    let defaults = PageSetup::default();
    PageSetup {
        width_twips: geometry
            .page_width_twips
            .map_or(defaults.width_twips, u32::from),
        height_twips: geometry
            .page_height_twips
            .map_or(defaults.height_twips, u32::from),
        margin_top_twips: geometry
            .margin_top_twips
            .map_or(defaults.margin_top_twips, |value| {
                u32::from(value.unsigned_abs())
            }),
        margin_bottom_twips: geometry
            .margin_bottom_twips
            .map_or(defaults.margin_bottom_twips, |value| {
                u32::from(value.unsigned_abs())
            }),
        margin_left_twips: geometry
            .margin_left_twips
            .map_or(defaults.margin_left_twips, u32::from),
        margin_right_twips: geometry
            .margin_right_twips
            .map_or(defaults.margin_right_twips, u32::from),
        landscape: geometry.orientation == Some(PageOrientation::Landscape),
        header_distance_twips: geometry
            .header_distance_twips
            .map_or(defaults.header_distance_twips, u32::from),
        footer_distance_twips: geometry
            .footer_distance_twips
            .map_or(defaults.footer_distance_twips, u32::from),
    }
}

fn column_layout(geometry: &SectionGeometry) -> Option<ColumnLayout> {
    let count = geometry.column_count?;
    (count > 1).then_some(ColumnLayout {
        count: u32::from(count),
        space_twips: geometry.column_spacing_twips.map(u32::from),
        separator: geometry.line_between_columns.unwrap_or(false),
        column_widths_twips: Vec::new(),
    })
}

fn section_break(code: Option<u8>) -> SectionBreakType {
    match code {
        Some(3) => SectionBreakType::EvenPage,
        Some(4) => SectionBreakType::OddPage,
        Some(2) => SectionBreakType::NextPage,
        _ => SectionBreakType::Continuous,
    }
}

fn paragraph_alignment(value: Option<u8>) -> Option<ParagraphAlignment> {
    match value {
        Some(0 | 7) => Some(ParagraphAlignment::Left),
        Some(1 | 8) => Some(ParagraphAlignment::Center),
        Some(2 | 9) => Some(ParagraphAlignment::Right),
        Some(3..=6) => Some(ParagraphAlignment::Justify),
        _ => None,
    }
}

fn underline_style(value: u8) -> Option<UnderlineStyle> {
    match value {
        0 => None,
        2 => Some(UnderlineStyle::Words),
        3 | 20 => Some(UnderlineStyle::Double),
        4 | 21 => Some(UnderlineStyle::Dotted),
        6 | 22 => Some(UnderlineStyle::Thick),
        7 | 9 | 23 | 25 => Some(UnderlineStyle::Dash),
        11 | 26 => Some(UnderlineStyle::DotDash),
        12 | 27 => Some(UnderlineStyle::DotDotDash),
        8 | 43 => Some(UnderlineStyle::Wave),
        _ => Some(UnderlineStyle::Single),
    }
}

fn color_ref(value: u32) -> [u8; 3] {
    [
        (value & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        ((value >> 16) & 0xFF) as u8,
    ]
}

fn indexed_color(value: u8) -> Option<[u8; 3]> {
    [
        None,
        Some([0, 0, 0]),
        Some([0, 0, 255]),
        Some([0, 255, 255]),
        Some([0, 128, 0]),
        Some([255, 0, 255]),
        Some([255, 0, 0]),
        Some([255, 255, 0]),
        Some([255, 255, 255]),
        Some([0, 0, 128]),
        Some([0, 128, 128]),
        Some([0, 64, 0]),
        Some([128, 0, 128]),
        Some([128, 0, 0]),
        Some([128, 128, 0]),
        Some([128, 128, 128]),
        Some([192, 192, 192]),
    ]
    .get(usize::from(value))
    .copied()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_word_colors_and_relative_toggles() {
        assert_eq!(indexed_color(6), Some([255, 0, 0]));
        assert!(resolve_toggle(
            Some(ToggleValue::SameAsStyle),
            Some(ToggleValue::On),
            Some(ToggleValue::SameAsStyle)
        ));
        assert!(!resolve_toggle(
            Some(ToggleValue::OppositeStyle),
            Some(ToggleValue::On),
            Some(ToggleValue::OppositeStyle)
        ));
    }

    #[test]
    fn wraps_dib_in_a_bmp_file_header() {
        let mut dib = vec![0_u8; 44];
        dib[0..4].copy_from_slice(&40_u32.to_le_bytes());
        dib[14..16].copy_from_slice(&24_u16.to_le_bytes());
        let bmp = dib_to_bmp(&dib).unwrap();
        assert!(bmp.starts_with(b"BM"));
        assert_eq!(u32::from_le_bytes(bmp[10..14].try_into().unwrap()), 54);
    }
}
