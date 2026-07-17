//! Resource limits applied before parser allocations.

/// Resource budgets for untrusted DOC input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocLimits {
    /// Maximum compound-file input size.
    pub max_input_bytes: usize,
    /// Maximum size of an individual CFB stream read into memory.
    pub max_stream_bytes: usize,
    /// Maximum number of text pieces in the CLX piece table.
    pub max_pieces: usize,
    /// Maximum number of reusable property groups in the CLX.
    pub max_piece_property_groups: usize,
    /// Maximum number of entries retained from the document stylesheet.
    pub max_styles: usize,
    /// Maximum permitted depth of a style inheritance chain.
    pub max_style_depth: usize,
    /// Maximum number of FFN entries retained from the font table.
    pub max_fonts: usize,
    /// Maximum number of reconstructed tables.
    pub max_tables: usize,
    /// Maximum number of reconstructed table rows.
    pub max_table_rows: usize,
    /// Maximum number of reconstructed table cells.
    pub max_table_cells: usize,
    /// Maximum number of header/footer and separator story descriptors.
    pub max_header_footer_stories: usize,
    /// Maximum cumulative footnote and endnote records.
    pub max_notes: usize,
    /// Maximum number of source comments.
    pub max_comments: usize,
    /// Maximum number of list definitions and format overrides.
    pub max_lists: usize,
    /// Maximum field-character records across all document stories.
    pub max_field_characters: usize,
    /// Maximum number of source-anchored inline picture records.
    pub max_media_items: usize,
    /// Maximum cumulative extracted BLIP payload bytes.
    pub max_media_bytes: usize,
    /// Maximum `OfficeArt` records visited while parsing all inline pictures.
    pub max_office_art_records: usize,
    /// Maximum nested `OfficeArt` container depth.
    pub max_office_art_depth: usize,
    /// Maximum declared character positions across all stories.
    pub max_characters: u32,
    /// Maximum number of `fc`/`lcb` pairs retained from the FIB.
    pub max_fib_pairs: usize,
    /// Maximum number of formatting pages referenced by one BTE PLCF.
    pub max_formatting_pages: usize,
    /// Maximum total PAPX or CHPX runs retained from FKP pages.
    pub max_formatting_runs: usize,
    /// Maximum number of main-document sections.
    pub max_sections: usize,
}

impl Default for DocLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 64 * 1024 * 1024,
            max_stream_bytes: 128 * 1024 * 1024,
            max_pieces: 1_000_000,
            max_piece_property_groups: 1_000_000,
            max_styles: 4_094,
            max_style_depth: 4_094,
            max_fonts: 0x7FF0,
            max_tables: 100_000,
            max_table_rows: 1_000_000,
            max_table_cells: 10_000_000,
            max_header_footer_stories: 1_000_000,
            max_notes: 1_000_000,
            max_comments: 1_000_000,
            max_lists: 100_000,
            max_field_characters: 2_000_000,
            max_media_items: 100_000,
            max_media_bytes: 128 * 1024 * 1024,
            max_office_art_records: 1_000_000,
            max_office_art_depth: 64,
            max_characters: 50_000_000,
            max_fib_pairs: 4_096,
            max_formatting_pages: 1_000_000,
            max_formatting_runs: 10_000_000,
            max_sections: 100_000,
        }
    }
}
