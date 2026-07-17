use std::{env, fs, process::ExitCode};

use legacy_doc::{DocLimits, WordBinaryDocument};

fn main() -> ExitCode {
    let document = match load_document() {
        Ok(document) => document,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    match inspect_document(&document) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn inspect_document(document: &WordBinaryDocument) -> Result<(), String> {
    let limits = DocLimits::default();
    print_fib(document);
    println!(
        "sections: {:#?}",
        document
            .sections(limits)
            .map_err(|error| error.to_string())?
    );
    print_comments(document, limits)?;
    if std::env::var_os("DOC_INSPECT_STYLES").is_some() {
        println!(
            "styles: {:#?}",
            document.styles(limits).map_err(|error| error.to_string())?
        );
    }
    let lists = document.lists(limits).map_err(|error| error.to_string())?;
    println!("lists: {lists:#?}");
    print_media_headers(document, limits)?;
    print_semantics(document, limits)
}

fn print_semantics(document: &WordBinaryDocument, limits: DocLimits) -> Result<(), String> {
    match (
        document.styled_formatting(limits),
        document.tables(limits),
        document.notes(limits),
    ) {
        (Ok(formatting), Ok(tables), Ok(notes)) => {
            println!("tables: {}", tables.tables().len());
            for (table_index, table) in tables.tables().iter().enumerate() {
                println!(
                    "table {table_index}: [{}, {}) depth={} rows={}",
                    table.cp_start,
                    table.cp_end,
                    table.depth,
                    table.rows.len()
                );
                for (row_index, row) in table.rows.iter().enumerate() {
                    println!(
                        "  row {row_index}: edges={:?} gap={:?} margins={:?} height={:?} cells={} nowrap={:?} fit={:?}",
                        row.properties
                            .definition
                            .as_ref()
                            .map(|definition| &definition.cell_edges_twips),
                        row.properties.gap_half_twips,
                        row.properties.default_cell_margins,
                        row.properties.row_height_twips,
                        row.cells.len(),
                        row.cells
                            .iter()
                            .map(|cell| cell.format.no_wrap)
                            .collect::<Vec<_>>(),
                        row.cells
                            .iter()
                            .map(|cell| cell.format.fit_text)
                            .collect::<Vec<_>>(),
                    );
                }
            }
            println!("notes: {:#?}", notes.notes());
            if std::env::var_os("DOC_INSPECT_RUNS").is_some() {
                for run in &formatting.character_runs {
                    println!(
                        "character [{}, {}): size={:?} fonts={:?}/{:?}/{:?} spacing={:?} style={:?} sprms={:?}",
                        run.cp_start,
                        run.cp_end,
                        run.properties.font_size_half_points,
                        run.properties.font_ascii,
                        run.properties.font_east_asian,
                        run.properties.font_bidi,
                        run.properties.character_spacing_twips,
                        run.character_style_index.or(run.paragraph_style_index),
                        run.sprms
                            .iter()
                            .map(|sprm| (sprm.opcode, sprm.operand.as_slice()))
                            .collect::<Vec<_>>(),
                    );
                }
            }
            for paragraph in formatting.paragraph_runs {
                let text = document
                    .decode_range(paragraph.cp_start, paragraph.cp_end)
                    .map_or_else(|error| format!("<{error}>"), |text| text.text);
                println!(
                    "paragraph [{}, {}) depth={:?} in_table={:?} ttp={:?} keep={:?}/{:?} page_break={:?} before={:?} after={:?} line={:?}: {:?}",
                    paragraph.cp_start,
                    paragraph.cp_end,
                    paragraph.properties.table_depth,
                    paragraph.properties.in_table,
                    paragraph.properties.table_terminating_paragraph,
                    paragraph.properties.keep_together,
                    paragraph.properties.keep_with_next,
                    paragraph.properties.page_break_before,
                    paragraph.properties.space_before_twips,
                    paragraph.properties.space_after_twips,
                    paragraph.properties.line_spacing,
                    text,
                );
                if paragraph.properties.table_terminating_paragraph == Some(true)
                    || std::env::var_os("DOC_INSPECT_RUNS").is_some()
                {
                    for sprm in &paragraph.sprms {
                        println!(
                            "  sprm={:#06X} group={:?} len={} operand={:02X?}",
                            sprm.opcode,
                            sprm.group,
                            sprm.operand.len(),
                            sprm.operand,
                        );
                    }
                }
            }
            Ok(())
        }
        (Err(error), _, _) | (_, Err(error), _) | (_, _, Err(error)) => Err(error.to_string()),
    }
}

fn print_media_headers(document: &WordBinaryDocument, limits: DocLimits) -> Result<(), String> {
    if std::env::var_os("DOC_INSPECT_MEDIA").is_none() {
        return Ok(());
    }
    let formatting = document
        .semantic_formatting(limits)
        .map_err(|error| error.to_string())?;
    let data = document.data_stream().unwrap_or_default();
    for run in formatting.character_runs {
        let Some(offset) = run.properties.picture_location else {
            continue;
        };
        let start = usize::try_from(offset).map_err(|error| error.to_string())?;
        let end = start.saturating_add(84).min(data.len());
        let header = data.get(start..end).unwrap_or_default();
        println!(
            "picture [{}, {}) Data[{offset}]={header:02X?}",
            run.source.cp_start, run.source.cp_end
        );
    }
    Ok(())
}

fn load_document() -> Result<WordBinaryDocument, String> {
    let input = env::args_os()
        .nth(1)
        .ok_or_else(|| "usage: inspect <input.doc>".to_string())?;
    let bytes = fs::read(input).map_err(|error| error.to_string())?;
    WordBinaryDocument::from_bytes(&bytes).map_err(|error| error.to_string())
}

fn print_comments(document: &WordBinaryDocument, limits: DocLimits) -> Result<(), String> {
    let comments = document
        .comments(limits)
        .map_err(|error| error.to_string())?;
    println!("comment authors: {:#?}", comments.authors());
    println!("comments: {:#?}", comments.comments());
    Ok(())
}

fn print_fib(document: &WordBinaryDocument) {
    println!("stories: {:#?}", document.fib().stories);
    for index in 0..document.fib().locations.len() {
        if let Some(location) = document.fib().locations.get(index)
            && !location.is_empty()
        {
            println!("fib[{index}] = {location:?}");
        }
    }
}
