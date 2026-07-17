//! Source-anchored PICF and `OfficeArt` image extraction.

use crate::{DocError, DocLimits, Result, ToggleValue, WordBinaryDocument, binary::checked_slice};

const PICF_HEADER_SIZE: usize = 68;
const PICF_HEADER_SIZE_U32: u32 = 68;
const NIL_PICF_HEADER_SIZE: usize = 68;
const NIL_PICF_HEADER_SIZE_U32: u32 = 68;
const MM_SHAPE: i16 = 0x0064;
const MM_SHAPEFILE: i16 = 0x0066;
const OFFICE_ART_SP_CONTAINER: u16 = 0xF004;
const OFFICE_ART_FBSE: u16 = 0xF007;

/// Storage variant declared by `PICF.mfpf.mm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureStorage {
    Shape,
    ShapeFile,
}

/// PICMID dimensions and scaling retained in Word twips/tenths of a percent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PictureGeometry {
    pub width_twips: i16,
    pub height_twips: i16,
    pub scale_x_tenths_percent: u16,
    pub scale_y_tenths_percent: u16,
}

/// Image format identified by an exact `OfficeArt` BLIP record type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlipFormat {
    Emf,
    Wmf,
    Pict,
    Jpeg,
    Png,
    Dib,
    Tiff,
}

impl BlipFormat {
    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Emf => "image/x-emf",
            Self::Wmf => "image/x-wmf",
            Self::Pict => "image/x-pict",
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Dib => "image/bmp",
            Self::Tiff => "image/tiff",
        }
    }

    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Emf => "emf",
            Self::Wmf => "wmf",
            Self::Pict => "pict",
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Dib => "dib",
            Self::Tiff => "tiff",
        }
    }
}

/// One exactly bounded BLIP payload found under an inline picture record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlipImage {
    pub format: BlipFormat,
    pub record_type: u16,
    /// Absolute byte offset of the `OfficeArt` record in the DOC Data stream.
    pub source_offset: u32,
    /// True for compressed EMF/WMF/PICT payloads.
    pub compressed: bool,
    pub data: Vec<u8>,
}

/// One inline picture proven by its text anchor and character properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlinePicture {
    /// Global CP of the U+0001 picture character.
    pub cp: u32,
    /// Absolute Data-stream offset supplied by `sprmCPicLocation`.
    pub data_offset: u32,
    /// PICF `lcb`, including the 68-byte header and `OfficeArt` data.
    pub record_length: u32,
    pub storage: PictureStorage,
    pub geometry: PictureGeometry,
    /// ANSI source name retained as bytes because its code page is external.
    pub source_name: Option<Vec<u8>>,
    /// Exact `OfficeArtInlineSpContainer` bytes for later shape serialization.
    pub office_art: Vec<u8>,
    pub images: Vec<BlipImage>,
}

/// All source-proven inline pictures in global CP order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaCollection {
    pictures: Vec<InlinePicture>,
}

impl MediaCollection {
    /// Resolves picture anchors through CHPX and parses only their exact PICF
    /// ranges. The Data stream is never scanned for byte signatures.
    ///
    /// # Errors
    ///
    /// Returns a typed error for contradictory anchors, malformed PICF or
    /// `OfficeArt` framing, invalid image signatures, or resource-limit breaches.
    pub fn parse(document: &WordBinaryDocument, limits: DocLimits) -> Result<Self> {
        let formatting = document.semantic_formatting(limits)?;
        let mut pictures = Vec::new();
        let mut retained_bytes = 0_usize;
        let mut office_art_records = 0_usize;

        for run in &formatting.character_runs {
            let Some(data_offset) = run.properties.picture_location else {
                continue;
            };
            let text = document.decode_range(run.source.cp_start, run.source.cp_end)?;
            let anchors = text
                .utf16
                .iter()
                .enumerate()
                .filter_map(|(index, character)| (*character == 0x0001).then_some(index))
                .collect::<Vec<_>>();
            // sprmCPicLocation also names ObjectPool entries on field separators;
            // those CHPX runs are not PICFAndOfficeArtData picture anchors.
            if anchors.is_empty() {
                continue;
            }
            if anchors.len() != 1 {
                return Err(DocError::InvalidMedia(format!(
                    "CHPX range [{}, {}) with sprmCPicLocation contains {} picture characters",
                    run.source.cp_start,
                    run.source.cp_end,
                    anchors.len()
                )));
            }
            if run.properties.special != Some(ToggleValue::On) {
                return Err(DocError::InvalidMedia(format!(
                    "picture character in CHPX range [{}, {}) has no explicit sprmCFSpec=1",
                    run.source.cp_start, run.source.cp_end
                )));
            }
            let anchor_delta = u32::try_from(anchors[0])
                .map_err(|_| DocError::InvalidMedia("picture CP delta overflow".into()))?;
            let cp = run
                .source
                .cp_start
                .checked_add(anchor_delta)
                .ok_or_else(|| DocError::InvalidMedia("picture CP overflow".into()))?;
            if run.properties.binary_data == Some(true) {
                if run.source.cp_end != run.source.cp_start.saturating_add(1) {
                    return Err(DocError::InvalidMedia(format!(
                        "binary-data CHPX range [{}, {}) must contain exactly one character",
                        run.source.cp_start, run.source.cp_end
                    )));
                }
                let data = document.data_stream().ok_or_else(|| {
                    DocError::InvalidMedia(format!(
                        "binary-data character at CP {cp} references absent Data stream"
                    ))
                })?;
                validate_nil_picf_and_bin_data(data, data_offset)?;
                continue;
            }
            if pictures.len() >= limits.max_media_items {
                return Err(resource_limit(
                    "inline-picture",
                    pictures.len().saturating_add(1),
                    limits.max_media_items,
                ));
            }
            let data = document.data_stream().ok_or_else(|| {
                DocError::InvalidMedia(format!("picture at CP {cp} references absent Data stream"))
            })?;
            let picture = parse_picf(data, data_offset, cp, limits, &mut office_art_records)?;
            let picture_bytes = picture
                .office_art
                .len()
                .checked_add(
                    picture
                        .images
                        .iter()
                        .try_fold(0_usize, |total, image| total.checked_add(image.data.len()))
                        .ok_or_else(|| {
                            DocError::InvalidMedia("media byte count overflow".into())
                        })?,
                )
                .ok_or_else(|| DocError::InvalidMedia("media byte count overflow".into()))?;
            retained_bytes = retained_bytes
                .checked_add(picture_bytes)
                .ok_or_else(|| DocError::InvalidMedia("media byte count overflow".into()))?;
            if retained_bytes > limits.max_media_bytes {
                return Err(resource_limit(
                    "media-byte",
                    retained_bytes,
                    limits.max_media_bytes,
                ));
            }
            pictures.push(picture);
        }
        pictures.sort_by_key(|picture| picture.cp);
        if pictures.windows(2).any(|pair| pair[0].cp == pair[1].cp) {
            return Err(DocError::InvalidMedia(
                "multiple picture records resolve to the same CP".into(),
            ));
        }
        Ok(Self { pictures })
    }

    #[must_use]
    pub fn pictures(&self) -> &[InlinePicture] {
        &self.pictures
    }
}

fn validate_nil_picf_and_bin_data(data: &[u8], data_offset: u32) -> Result<()> {
    let header = checked_slice(
        data,
        data_offset,
        NIL_PICF_HEADER_SIZE_U32,
        "NilPICFAndBinData",
    )?;
    let signed_length = i32::from_le_bytes(header[0..4].try_into().unwrap());
    let record_length = u32::try_from(signed_length).map_err(|_| {
        DocError::InvalidMedia(format!(
            "NilPICFAndBinData at {data_offset} has negative lcb {signed_length}"
        ))
    })?;
    if record_length < NIL_PICF_HEADER_SIZE_U32 {
        return Err(DocError::InvalidMedia(format!(
            "NilPICFAndBinData at {data_offset} has lcb {record_length} below {NIL_PICF_HEADER_SIZE}"
        )));
    }
    checked_slice(data, data_offset, record_length, "NilPICFAndBinData")?;
    let cb_header = u16::from_le_bytes(header[4..6].try_into().unwrap());
    if usize::from(cb_header) != NIL_PICF_HEADER_SIZE {
        return Err(DocError::InvalidMedia(format!(
            "NilPICFAndBinData at {data_offset} has cbHeader {cb_header}, expected {NIL_PICF_HEADER_SIZE}"
        )));
    }
    if header[6..].iter().any(|byte| *byte != 0) {
        return Err(DocError::InvalidMedia(format!(
            "NilPICFAndBinData at {data_offset} has nonzero ignored header bytes"
        )));
    }
    Ok(())
}

impl WordBinaryDocument {
    /// Parses source-anchored inline pictures without scanning mixed Data bytes.
    ///
    /// # Errors
    ///
    /// Returns a typed formatting, PICF, `OfficeArt`, signature, or limit error.
    pub fn media(&self, limits: DocLimits) -> Result<MediaCollection> {
        MediaCollection::parse(self, limits)
    }
}

fn parse_picf(
    data: &[u8],
    data_offset: u32,
    cp: u32,
    limits: DocLimits,
    record_count: &mut usize,
) -> Result<InlinePicture> {
    let header = checked_slice(data, data_offset, PICF_HEADER_SIZE_U32, "PICF")?;
    let signed_length = i32::from_le_bytes(header[0..4].try_into().unwrap());
    let record_length = u32::try_from(signed_length).map_err(|_| {
        DocError::InvalidMedia(format!(
            "PICF at {data_offset} has negative lcb {signed_length}"
        ))
    })?;
    if record_length < PICF_HEADER_SIZE_U32 {
        return Err(DocError::InvalidMedia(format!(
            "PICF at {data_offset} has lcb {record_length} below {PICF_HEADER_SIZE}"
        )));
    }
    let record = checked_slice(data, data_offset, record_length, "PICFAndOfficeArtData")?;
    let cb_header = u16::from_le_bytes(record[4..6].try_into().unwrap());
    if usize::from(cb_header) != PICF_HEADER_SIZE {
        return Err(DocError::InvalidMedia(format!(
            "PICF at {data_offset} has cbHeader {cb_header}, expected {PICF_HEADER_SIZE}"
        )));
    }
    let storage = match i16::from_le_bytes(record[6..8].try_into().unwrap()) {
        MM_SHAPE => PictureStorage::Shape,
        MM_SHAPEFILE => PictureStorage::ShapeFile,
        value => {
            return Err(DocError::InvalidMedia(format!(
                "picture anchor at CP {cp} references PICF at Data offset {data_offset} with unsupported MFPF.mm {value:#06X}"
            )));
        }
    };
    if record[66..68] != [0, 0] {
        return Err(DocError::InvalidMedia(format!(
            "PICF at {data_offset} has nonzero cProps"
        )));
    }
    let geometry = parse_geometry(record, data_offset)?;
    let (source_name, picture_start) = if storage == PictureStorage::ShapeFile {
        let name_length = usize::from(*record.get(PICF_HEADER_SIZE).ok_or_else(|| {
            DocError::InvalidMedia(format!(
                "shape-file PICF at {data_offset} has no cchPicName"
            ))
        })?);
        let start = PICF_HEADER_SIZE + 1;
        let end = start
            .checked_add(name_length)
            .ok_or_else(|| DocError::InvalidMedia("picture name length overflow".into()))?;
        let name = record.get(start..end).ok_or_else(|| {
            DocError::InvalidMedia(format!("shape-file name at {data_offset} exceeds PICF lcb"))
        })?;
        (Some(name.to_vec()), end)
    } else {
        (None, PICF_HEADER_SIZE)
    };
    let office_art = record.get(picture_start..).ok_or_else(|| {
        DocError::InvalidMedia(format!("PICF at {data_offset} has no OfficeArt data"))
    })?;
    if office_art.is_empty() {
        return Err(DocError::InvalidMedia(format!(
            "PICF at {data_offset} has empty OfficeArt data"
        )));
    }
    let picture_start_u32 = u32::try_from(picture_start)
        .map_err(|_| DocError::InvalidMedia("OfficeArt offset overflow".into()))?;
    let office_art_offset = data_offset
        .checked_add(picture_start_u32)
        .ok_or_else(|| DocError::InvalidMedia("OfficeArt source offset overflow".into()))?;
    let images = parse_inline_office_art(office_art, office_art_offset, limits, record_count)?;
    Ok(InlinePicture {
        cp,
        data_offset,
        record_length,
        storage,
        geometry,
        source_name,
        office_art: office_art.to_vec(),
        images,
    })
}

fn parse_geometry(record: &[u8], data_offset: u32) -> Result<PictureGeometry> {
    let width_twips = i16::from_le_bytes(record[28..30].try_into().unwrap());
    let height_twips = i16::from_le_bytes(record[30..32].try_into().unwrap());
    let horizontal_scale = u16::from_le_bytes(record[32..34].try_into().unwrap());
    let vertical_scale = u16::from_le_bytes(record[34..36].try_into().unwrap());
    if width_twips <= 0 || height_twips <= 0 {
        return Err(DocError::InvalidMedia(format!(
            "PICF at {data_offset} has nonpositive goal size {width_twips}x{height_twips} twips"
        )));
    }
    if horizontal_scale == 0 || vertical_scale == 0 {
        return Err(DocError::InvalidMedia(format!(
            "PICF at {data_offset} has zero picture scale"
        )));
    }
    Ok(PictureGeometry {
        width_twips,
        height_twips,
        scale_x_tenths_percent: horizontal_scale,
        scale_y_tenths_percent: vertical_scale,
    })
}

fn parse_inline_office_art(
    data: &[u8],
    source_offset: u32,
    limits: DocLimits,
    record_count: &mut usize,
) -> Result<Vec<BlipImage>> {
    let shape = parse_record(data, 0, source_offset, limits, record_count)?;
    if shape.version != 0xF || shape.record_type != OFFICE_ART_SP_CONTAINER {
        return Err(DocError::InvalidMedia(format!(
            "OfficeArtInlineSpContainer at {source_offset} does not begin with an F004 shape container"
        )));
    }
    validate_container(shape.payload, shape.payload_offset, 1, limits, record_count)?;

    let mut images = Vec::new();
    let mut position = shape.total_length;
    while position < data.len() {
        let record = parse_record(data, position, source_offset, limits, record_count)?;
        match record.record_type {
            OFFICE_ART_FBSE => parse_fbse(record, limits, record_count, &mut images)?,
            0xF018..=0xF117 => {
                if let Some(image) = parse_blip(record)? {
                    images.push(image);
                }
            }
            value => {
                return Err(DocError::InvalidMedia(format!(
                    "unexpected OfficeArt file block type {value:#06X} at {}",
                    record.source_offset
                )));
            }
        }
        position = position
            .checked_add(record.total_length)
            .ok_or_else(|| DocError::InvalidMedia("OfficeArt position overflow".into()))?;
    }
    Ok(images)
}

fn validate_container(
    data: &[u8],
    source_offset: u32,
    depth: usize,
    limits: DocLimits,
    record_count: &mut usize,
) -> Result<()> {
    if depth > limits.max_office_art_depth {
        return Err(resource_limit(
            "office-art-depth",
            depth,
            limits.max_office_art_depth,
        ));
    }
    let mut position = 0_usize;
    while position < data.len() {
        let record = parse_record(data, position, source_offset, limits, record_count)?;
        if record.version == 0xF {
            validate_container(
                record.payload,
                record.payload_offset,
                depth + 1,
                limits,
                record_count,
            )?;
        }
        position = position
            .checked_add(record.total_length)
            .ok_or_else(|| DocError::InvalidMedia("OfficeArt position overflow".into()))?;
    }
    Ok(())
}

fn parse_fbse(
    record: OfficeArtRecord<'_>,
    limits: DocLimits,
    record_count: &mut usize,
    images: &mut Vec<BlipImage>,
) -> Result<()> {
    if record.version != 0x2 || record.payload.len() < 36 {
        return Err(DocError::InvalidMedia(format!(
            "OfficeArtFBSE at {} has invalid version/length",
            record.source_offset
        )));
    }
    let embedded_size = usize::try_from(u32::from_le_bytes(
        record.payload[20..24].try_into().unwrap(),
    ))
    .map_err(|_| DocError::InvalidMedia("FBSE embedded size overflow".into()))?;
    let name_length = usize::from(record.payload[33]);
    if name_length > 0xFE || !name_length.is_multiple_of(2) {
        return Err(DocError::InvalidMedia(format!(
            "OfficeArtFBSE at {} has invalid cbName {name_length}",
            record.source_offset
        )));
    }
    let embedded_start = 36_usize
        .checked_add(name_length)
        .ok_or_else(|| DocError::InvalidMedia("FBSE name length overflow".into()))?;
    let embedded_end = embedded_start
        .checked_add(embedded_size)
        .ok_or_else(|| DocError::InvalidMedia("FBSE embedded length overflow".into()))?;
    if embedded_end != record.payload.len() {
        return Err(DocError::InvalidMedia(format!(
            "OfficeArtFBSE at {} declares {embedded_size} embedded bytes but payload length is {}",
            record.source_offset,
            record.payload.len()
        )));
    }
    let mut position = embedded_start;
    while position < embedded_end {
        let embedded = parse_record(
            record.payload,
            position,
            record.payload_offset,
            limits,
            record_count,
        )?;
        if let Some(image) = parse_blip(embedded)? {
            images.push(image);
        } else {
            return Err(DocError::InvalidMedia(format!(
                "OfficeArtFBSE at {} embeds non-BLIP record {:#06X}",
                record.source_offset, embedded.record_type
            )));
        }
        position = position
            .checked_add(embedded.total_length)
            .ok_or_else(|| DocError::InvalidMedia("FBSE position overflow".into()))?;
    }
    Ok(())
}

fn parse_blip(record: OfficeArtRecord<'_>) -> Result<Option<BlipImage>> {
    let (format, prefix_length, compressed) = match record.record_type {
        0xF01A => (
            BlipFormat::Emf,
            metafile_prefix(record, 0x3D4, 0x3D5)?,
            true,
        ),
        0xF01B => (
            BlipFormat::Wmf,
            metafile_prefix(record, 0x216, 0x217)?,
            true,
        ),
        0xF01C => (
            BlipFormat::Pict,
            metafile_prefix(record, 0x542, 0x543)?,
            true,
        ),
        0xF01D | 0xF02A => (
            BlipFormat::Jpeg,
            raster_prefix(record, &[0x46A, 0x6E2], &[0x46B, 0x6E3])?,
            false,
        ),
        0xF01E => (
            BlipFormat::Png,
            raster_prefix(record, &[0x6E0], &[0x6E1])?,
            false,
        ),
        0xF01F => (
            BlipFormat::Dib,
            raster_prefix(record, &[0x7A8], &[0x7A9])?,
            false,
        ),
        0xF029 => (
            BlipFormat::Tiff,
            raster_prefix(record, &[0x6E4], &[0x6E5])?,
            false,
        ),
        _ => return Ok(None),
    };
    let data = record.payload.get(prefix_length..).ok_or_else(|| {
        DocError::InvalidMedia(format!(
            "BLIP {:#06X} at {} is shorter than its header",
            record.record_type, record.source_offset
        ))
    })?;
    if data.is_empty() {
        return Err(DocError::InvalidMedia(format!(
            "BLIP {:#06X} at {} has empty file data",
            record.record_type, record.source_offset
        )));
    }
    validate_signature(format, data, record.source_offset)?;
    let is_compressed = compressed && record.payload[prefix_length - 2] == 0x00;
    Ok(Some(BlipImage {
        format,
        record_type: record.record_type,
        source_offset: record.source_offset,
        compressed: is_compressed,
        data: data.to_vec(),
    }))
}

fn raster_prefix(
    record: OfficeArtRecord<'_>,
    one_uid_instances: &[u16],
    two_uid_instances: &[u16],
) -> Result<usize> {
    require_atom(record)?;
    let prefix = if one_uid_instances.contains(&record.instance) {
        17
    } else if two_uid_instances.contains(&record.instance) {
        33
    } else {
        return Err(DocError::InvalidMedia(format!(
            "BLIP {:#06X} at {} has invalid recInstance {:#05X}",
            record.record_type, record.source_offset, record.instance
        )));
    };
    if record.payload.len() <= prefix {
        return Err(DocError::InvalidMedia(format!(
            "BLIP {:#06X} at {} is truncated",
            record.record_type, record.source_offset
        )));
    }
    Ok(prefix)
}

fn metafile_prefix(
    record: OfficeArtRecord<'_>,
    one_uid_instance: u16,
    two_uid_instance: u16,
) -> Result<usize> {
    require_atom(record)?;
    let prefix = if record.instance == one_uid_instance {
        50
    } else if record.instance == two_uid_instance {
        66
    } else {
        return Err(DocError::InvalidMedia(format!(
            "metafile BLIP {:#06X} at {} has invalid recInstance {:#05X}",
            record.record_type, record.source_offset, record.instance
        )));
    };
    if record.payload.len() <= prefix {
        return Err(DocError::InvalidMedia(format!(
            "metafile BLIP {:#06X} at {} is truncated",
            record.record_type, record.source_offset
        )));
    }
    Ok(prefix)
}

fn require_atom(record: OfficeArtRecord<'_>) -> Result<()> {
    if record.version == 0 {
        Ok(())
    } else {
        Err(DocError::InvalidMedia(format!(
            "BLIP {:#06X} at {} has recVer {}, expected 0",
            record.record_type, record.source_offset, record.version
        )))
    }
}

fn validate_signature(format: BlipFormat, data: &[u8], source_offset: u32) -> Result<()> {
    let valid = match format {
        BlipFormat::Jpeg => data.starts_with(&[0xFF, 0xD8]),
        BlipFormat::Png => data.starts_with(b"\x89PNG\r\n\x1a\n"),
        BlipFormat::Tiff => data.starts_with(b"II*\0") || data.starts_with(b"MM\0*"),
        BlipFormat::Dib => data
            .get(..4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .is_some_and(|size| matches!(size, 12 | 40 | 52 | 56 | 108 | 124)),
        // Metafile BLIPFileData can be OfficeArt-compressed, so its signature
        // is validated after decompression by the renderer/converter layer.
        BlipFormat::Emf | BlipFormat::Wmf | BlipFormat::Pict => true,
    };
    if valid {
        Ok(())
    } else {
        Err(DocError::InvalidMedia(format!(
            "{} BLIP at {source_offset} has an invalid file signature",
            format.extension()
        )))
    }
}

#[derive(Debug, Clone, Copy)]
struct OfficeArtRecord<'a> {
    version: u8,
    instance: u16,
    record_type: u16,
    source_offset: u32,
    payload_offset: u32,
    payload: &'a [u8],
    total_length: usize,
}

fn parse_record<'a>(
    containing: &'a [u8],
    position: usize,
    containing_offset: u32,
    limits: DocLimits,
    record_count: &mut usize,
) -> Result<OfficeArtRecord<'a>> {
    *record_count = record_count
        .checked_add(1)
        .ok_or_else(|| DocError::InvalidMedia("OfficeArt record count overflow".into()))?;
    if *record_count > limits.max_office_art_records {
        return Err(resource_limit(
            "office-art-record",
            *record_count,
            limits.max_office_art_records,
        ));
    }
    let header_end = position
        .checked_add(8)
        .ok_or_else(|| DocError::InvalidMedia("OfficeArt header offset overflow".into()))?;
    let header = containing.get(position..header_end).ok_or_else(|| {
        DocError::InvalidMedia(format!(
            "OfficeArt record at {containing_offset}+{position} has a truncated header"
        ))
    })?;
    let version_instance = u16::from_le_bytes(header[0..2].try_into().unwrap());
    let version = (version_instance & 0x000F) as u8;
    let instance = version_instance >> 4;
    let record_type = u16::from_le_bytes(header[2..4].try_into().unwrap());
    if !(0xF000..=0xFFFF).contains(&record_type) {
        return Err(DocError::InvalidMedia(format!(
            "OfficeArt record at {containing_offset}+{position} has invalid type {record_type:#06X}"
        )));
    }
    let payload_length = usize::try_from(u32::from_le_bytes(header[4..8].try_into().unwrap()))
        .map_err(|_| DocError::InvalidMedia("OfficeArt record length overflow".into()))?;
    let total_length = 8_usize
        .checked_add(payload_length)
        .ok_or_else(|| DocError::InvalidMedia("OfficeArt record length overflow".into()))?;
    let record_end = position
        .checked_add(total_length)
        .ok_or_else(|| DocError::InvalidMedia("OfficeArt record end overflow".into()))?;
    let payload = containing.get(header_end..record_end).ok_or_else(|| {
        DocError::InvalidMedia(format!(
            "OfficeArt record {record_type:#06X} at {containing_offset}+{position} declares {payload_length} payload bytes beyond its container"
        ))
    })?;
    let position_u32 = u32::try_from(position)
        .map_err(|_| DocError::InvalidMedia("OfficeArt source position overflow".into()))?;
    let source_offset = containing_offset
        .checked_add(position_u32)
        .ok_or_else(|| DocError::InvalidMedia("OfficeArt source offset overflow".into()))?;
    let payload_offset = source_offset
        .checked_add(8)
        .ok_or_else(|| DocError::InvalidMedia("OfficeArt payload offset overflow".into()))?;
    Ok(OfficeArtRecord {
        version,
        instance,
        record_type,
        source_offset,
        payload_offset,
        payload,
        total_length,
    })
}

fn resource_limit(resource: &'static str, actual: usize, limit: usize) -> DocError {
    DocError::ResourceLimit {
        resource,
        actual: u64::try_from(actual).unwrap_or(u64::MAX),
        limit: u64::try_from(limit).unwrap_or(u64::MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(version: u8, instance: u16, record_type: u16, payload: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend_from_slice(&((instance << 4) | u16::from(version)).to_le_bytes());
        result.extend_from_slice(&record_type.to_le_bytes());
        result.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
        result.extend_from_slice(payload);
        result
    }

    fn png_blip() -> Vec<u8> {
        let mut payload = vec![0_u8; 17];
        payload.extend_from_slice(b"\x89PNG\r\n\x1a\nfixture");
        record(0, 0x6E0, 0xF01E, &payload)
    }

    fn picf_with(file_block: &[u8]) -> Vec<u8> {
        let shape = record(0xF, 0, OFFICE_ART_SP_CONTAINER, &[]);
        let mut result = vec![0_u8; PICF_HEADER_SIZE];
        result[4..6].copy_from_slice(&u16::try_from(PICF_HEADER_SIZE).unwrap().to_le_bytes());
        result[6..8].copy_from_slice(&MM_SHAPE.to_le_bytes());
        result[28..30].copy_from_slice(&1440_i16.to_le_bytes());
        result[30..32].copy_from_slice(&720_i16.to_le_bytes());
        result[32..34].copy_from_slice(&1000_u16.to_le_bytes());
        result[34..36].copy_from_slice(&1000_u16.to_le_bytes());
        result.extend_from_slice(&shape);
        result.extend_from_slice(file_block);
        let length = i32::try_from(result.len()).unwrap();
        result[0..4].copy_from_slice(&length.to_le_bytes());
        result
    }

    #[test]
    fn parses_exact_anchored_png() {
        let data = picf_with(&png_blip());
        let mut records = 0;
        let picture = parse_picf(&data, 0, 7, DocLimits::default(), &mut records).unwrap();
        assert_eq!(picture.cp, 7);
        assert_eq!(picture.geometry.width_twips, 1440);
        assert_eq!(picture.images.len(), 1);
        assert_eq!(picture.images[0].format, BlipFormat::Png);
        assert!(picture.images[0].data.starts_with(b"\x89PNG"));
    }

    #[test]
    fn parses_embedded_fbse_png() {
        let blip = png_blip();
        let mut payload = vec![0_u8; 36];
        payload[20..24].copy_from_slice(&u32::try_from(blip.len()).unwrap().to_le_bytes());
        payload.extend_from_slice(&blip);
        let fbse = record(2, 6, OFFICE_ART_FBSE, &payload);
        let data = picf_with(&fbse);
        let mut records = 0;
        let picture = parse_picf(&data, 0, 0, DocLimits::default(), &mut records).unwrap();
        assert_eq!(picture.images.len(), 1);
        assert_eq!(picture.images[0].format, BlipFormat::Png);
    }

    #[test]
    fn rejects_blip_length_past_picf() {
        let mut blip = png_blip();
        blip[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let data = picf_with(&blip);
        let mut records = 0;
        assert!(matches!(
            parse_picf(&data, 0, 0, DocLimits::default(), &mut records),
            Err(DocError::InvalidMedia(_))
        ));
    }

    #[test]
    fn rejects_false_png_signature() {
        let mut payload = vec![0_u8; 17];
        payload.extend_from_slice(b"not a png");
        let data = picf_with(&record(0, 0x6E0, 0xF01E, &payload));
        let mut records = 0;
        assert!(matches!(
            parse_picf(&data, 0, 0, DocLimits::default(), &mut records),
            Err(DocError::InvalidMedia(_))
        ));
    }

    #[test]
    fn validates_bounded_nil_picf_binary_data() {
        let mut data = vec![0_u8; NIL_PICF_HEADER_SIZE + 3];
        let data_length = i32::try_from(data.len()).unwrap();
        data[0..4].copy_from_slice(&data_length.to_le_bytes());
        data[4..6].copy_from_slice(&NIL_PICF_HEADER_SIZE_U32.to_le_bytes()[..2]);
        data[NIL_PICF_HEADER_SIZE..].copy_from_slice(b"bin");
        validate_nil_picf_and_bin_data(&data, 0).unwrap();

        data[6] = 1;
        assert!(matches!(
            validate_nil_picf_and_bin_data(&data, 0),
            Err(DocError::InvalidMedia(_))
        ));
        data[6] = 0;
        let oversized_length = i32::try_from(data.len() + 1).unwrap();
        data[0..4].copy_from_slice(&oversized_length.to_le_bytes());
        assert!(matches!(
            validate_nil_picf_and_bin_data(&data, 0),
            Err(DocError::OutOfBounds { .. })
        ));
    }
}
