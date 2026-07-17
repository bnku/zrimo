use std::{fs, path::PathBuf};

use legacy_doc::{
    DocError, DocLimits, FormattingIndex, StoryKind, WordBinaryDocument, decode_grpprl,
};

fn corpus() -> PathBuf {
    PathBuf::from(std::env::var("CORPUS_DIR").expect("CORPUS_DIR is required"))
}

fn parse(name: &str) -> WordBinaryDocument {
    let bytes = fs::read(corpus().join(name)).expect("DOC fixture must be readable");
    WordBinaryDocument::from_bytes(&bytes).expect("Word 97+ fixture must parse")
}

#[test]
#[ignore = "requires npm run corpus:fetch"]
fn parses_public_word97_story_and_piece_table_corpus() {
    for (name, expected_version) in [
        ("word97-simple.doc", 0x00C1),
        ("word2000-bug46817.doc", 0x00D9),
        ("word97-footnote.doc", 0x0101),
        ("word2003-test2.doc", 0x010C),
        ("word97-header-footer-unicode.doc", 0x0112),
        ("word97-simple-table.doc", 0x00C1),
        ("word97-comments.doc", 0x0101),
    ] {
        let document = parse(name);
        assert_eq!(document.fib().effective_version, expected_version, "{name}");
        let main = document.story(StoryKind::Main).expect("main story");
        assert!(!main.content.utf16.is_empty(), "empty main story in {name}");
        assert_eq!(
            main.content.utf16.len(),
            usize::try_from(document.fib().stories.main).unwrap(),
            "main story CP alignment changed in {name}",
        );
        assert!(!document.piece_table().pieces().is_empty(), "{name}");
        let formatting = document
            .formatting_index(DocLimits::default())
            .expect("BTE/FKP formatting index must parse");
        assert!(!formatting.character_runs.is_empty(), "CHPX: {name}");
        assert!(!formatting.paragraph_runs.is_empty(), "PAPX: {name}");
        assert_fkp_grpprls(&formatting, name);
        let logical = formatting
            .to_logical(document.piece_table())
            .expect("every FKP range must map exactly to source CPs");
        assert!(!logical.character_runs.is_empty(), "logical CHPX: {name}");
        assert!(!logical.paragraph_runs.is_empty(), "logical PAPX: {name}");
        assert!(
            logical
                .character_runs
                .iter()
                .chain(&logical.paragraph_runs)
                .all(|run| run.cp_start < run.cp_end
                    && run.cp_end <= document.piece_table().cp_end())
        );
        let semantic = logical
            .resolve_properties(document.piece_table())
            .expect("FKP and piece PRM properties must apply in source order");
        assert_eq!(
            semantic.character_runs.len(),
            logical.character_runs.len(),
            "semantic CHPX run count: {name}",
        );
        assert_eq!(
            semantic.paragraph_runs.len(),
            logical.paragraph_runs.len(),
            "semantic PAPX run count: {name}",
        );
        let styles = document
            .styles(DocLimits::default())
            .unwrap_or_else(|error| panic!("STSH styles must parse in {name}: {error:?}"));
        assert!(
            styles.styles().iter().flatten().next().is_some(),
            "stylesheet has no non-empty styles: {name}"
        );
        for style in styles.styles().iter().flatten() {
            styles
                .inherited_properties(style.index)
                .expect("every style inheritance chain and UPX must apply");
        }
        let composed = semantic
            .apply_styles(&styles)
            .unwrap_or_else(|error| panic!("style application in {name}: {error:?}"));
        assert_eq!(
            composed.paragraph_runs.len(),
            semantic.paragraph_runs.len(),
            "styled PAPX run count: {name}",
        );
        assert!(
            composed
                .character_runs
                .iter()
                .all(|run| run.cp_start < run.cp_end),
            "styled CHPX contains an empty range: {name}",
        );
        let fonts = document
            .fonts(DocLimits::default())
            .unwrap_or_else(|error| panic!("font table in {name}: {error:?}"));
        assert!(!fonts.is_empty(), "font table is empty: {name}");

        let sections = document
            .sections(DocLimits::default())
            .expect("PLCFSED/SEPX must parse");
        assert!(!sections.is_empty(), "sections: {name}");
        assert_eq!(
            sections.sections().last().unwrap().cp_end,
            document.fib().stories.main,
            "section ranges must cover the main story: {name}",
        );
        assert_extended_layers(&document, name);
    }
}

fn assert_fkp_grpprls(formatting: &FormattingIndex, name: &str) {
    for run in &formatting.character_runs {
        decode_grpprl(&run.grpprl).expect("every CHPX grpprl must be exactly framed");
    }
    for run in &formatting.paragraph_runs {
        if decode_grpprl(&run.grpprl).is_err() {
            assert_eq!(run.grpprl.last(), Some(&0), "PAPX padding in {name}");
            decode_grpprl(&run.grpprl[..run.grpprl.len() - 1])
                .expect("PAPX may contain only one terminal zero alignment byte");
        }
    }
}

fn assert_extended_layers(document: &WordBinaryDocument, name: &str) {
    document
        .fields(DocLimits::default())
        .unwrap_or_else(|error| panic!("field PLCs in {name}: {error:?}"));
    document
        .notes(DocLimits::default())
        .unwrap_or_else(|error| panic!("note PLCs in {name}: {error:?}"));
    document
        .comments(DocLimits::default())
        .unwrap_or_else(|error| panic!("comment PLCs in {name}: {error:?}"));
    document
        .lists(DocLimits::default())
        .unwrap_or_else(|error| panic!("PlfLst/PlfLfo lists in {name}: {error:?}"));
    document
        .media(DocLimits::default())
        .unwrap_or_else(|error| panic!("source-anchored media in {name}: {error:?}"));
    document
        .to_ooxml_ir(DocLimits::default())
        .unwrap_or_else(|error| panic!("OOXML projection in {name}: {error:?}"));
}

#[test]
#[ignore = "requires npm run corpus:fetch"]
fn parses_public_word97_list_definitions_and_instances() {
    let document = parse("word97-comments.doc");
    let lists = document
        .lists(DocLimits::default())
        .expect("PlfLst and PlfLfo must parse");

    assert_eq!(lists.definitions().len(), 11);
    assert_eq!(lists.overrides().len(), 11);
    assert!(
        lists
            .definitions()
            .iter()
            .any(|definition| { !definition.simple && definition.levels.len() == 9 })
    );
    assert!(lists.definitions().iter().any(|definition| {
        definition
            .levels
            .iter()
            .any(legacy_doc::ListLevel::is_bullet)
    }));
    assert!(lists.definitions().iter().all(|definition| {
        lists.definition(definition.lsid).is_some()
            && definition
                .levels
                .iter()
                .all(|level| !level.ooxml_level_text().is_empty() || level.number_format == 0xFF)
    }));
    assert!(
        lists
            .overrides()
            .iter()
            .all(|list| { list.num_id > 0 && lists.definition(list.lsid).is_some() })
    );
}

#[test]
#[ignore = "requires npm run corpus:fetch"]
fn preserves_table_and_auxiliary_story_markers() {
    let table = parse("word97-simple-table.doc");
    assert!(
        table
            .story(StoryKind::Main)
            .expect("main story")
            .content
            .utf16
            .contains(&0x0007),
        "table cell markers must remain source-aligned",
    );
    let reconstructed = table
        .tables(DocLimits::default())
        .expect("source TTP and TDefTable records must reconstruct the table");
    assert!(
        !reconstructed.is_empty(),
        "table grid must not be flattened"
    );
    assert!(
        reconstructed
            .tables()
            .iter()
            .flat_map(|table| &table.rows)
            .all(|row| !row.cells.is_empty()),
        "every reconstructed row must retain cells",
    );

    let header = parse("word97-header-footer-unicode.doc");
    assert!(
        !header
            .story(StoryKind::Headers)
            .expect("header story")
            .content
            .utf16
            .is_empty(),
        "header/footer story must be retained",
    );
    let header_links = header
        .header_footers(DocLimits::default())
        .expect("PlcfHdd stories must link to source sections");
    assert_eq!(
        header_links.sections().len(),
        header.sections(DocLimits::default()).unwrap().len(),
        "every section must have a resolved header/footer role set",
    );
    assert!(
        header_links.stories().iter().any(|story| !story.is_empty()),
        "header/footer fixture must expose a non-empty source story",
    );

    let footnote = parse("word97-footnote.doc");
    assert!(
        !footnote
            .story(StoryKind::Footnotes)
            .expect("footnote story")
            .content
            .utf16
            .is_empty(),
        "footnote story must be retained",
    );
    let notes = footnote
        .notes(DocLimits::default())
        .expect("footnote and endnote PLCs must reconstruct");
    assert_eq!(notes.notes().len(), 2);
    assert_eq!(notes.references().len(), 2);
}

#[test]
#[ignore = "requires npm run corpus:fetch"]
fn parses_public_comment_references_metadata_and_bodies() {
    let document = parse("word97-comments.doc");
    let story = document
        .story(StoryKind::Comments)
        .expect("comment story must be retained");
    assert_eq!(story.cp_end - story.cp_start, 60);
    let comments = document
        .comments(DocLimits::default())
        .expect("PlcfandRef/PlcfandTxt must parse");
    assert_eq!(comments.comments().len(), 3);
    assert_eq!(comments.authors(), [" ", "Ryan Lauck"]);
    assert_eq!(comments.comments()[0].initials, "Ryan Lauc");
    assert_eq!(comments.comments()[0].author_index, 1);
    assert_eq!(comments.comments()[0].author, "Ryan Lauck");
    assert!(comments.comments().windows(2).all(|pair| {
        pair[0].reference_cp < pair[1].reference_cp && pair[0].cp_end == pair[1].cp_start
    }));
    for comment in comments.comments() {
        assert_eq!(
            document
                .decode_range(comment.cp_start, comment.cp_start + 1)
                .unwrap()
                .utf16,
            [0x0005]
        );
        assert_eq!(
            document
                .decode_range(comment.cp_end - 1, comment.cp_end)
                .unwrap()
                .utf16,
            [0x000D]
        );
    }
}

#[test]
#[ignore = "requires npm run corpus:fetch"]
fn parses_public_word97_ranged_comment_bookmark() {
    let document = parse("word97-ranged-comment.doc");
    let comments = document
        .comments(DocLimits::default())
        .expect("annotation bookmark tables must resolve");
    assert_eq!(comments.comments().len(), 1);
    let comment = &comments.comments()[0];
    assert_eq!(comment.author, "John Greer");
    assert_eq!(comment.initials, "jmg");
    assert_eq!(comment.bookmark_tag, 133_850_505);
    assert_eq!(
        (comment.anchor_cp_start, comment.anchor_cp_end),
        (Some(8385), Some(8392))
    );
    assert_eq!(document.decode_range(8385, 8392).unwrap().text, "comment");
    assert_eq!(
        comments
            .ranges_starting_at(8385)
            .map(|item| item.comment_id)
            .collect::<Vec<_>>(),
        [1]
    );
    assert_eq!(
        comments
            .ranges_ending_at(8392)
            .map(|item| item.comment_id)
            .collect::<Vec<_>>(),
        [1]
    );
}

#[test]
#[ignore = "requires npm run corpus:fetch"]
fn rejects_word6_with_a_typed_version_error() {
    let bytes = fs::read(corpus().join("word6.doc")).expect("DOC fixture must be readable");
    assert!(matches!(
        WordBinaryDocument::from_bytes(&bytes),
        Err(DocError::UnsupportedVersion(_))
    ));
}
