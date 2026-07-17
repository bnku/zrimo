//! `SttbfFfn` and source font metadata parsing.

use crate::{DocError, DocLimits, Fib, Result, binary::checked_slice};

const FFN_FIXED_SIZE: usize = 39;

/// One source font record addressed by `ftc` values in character properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontEntry {
    pub index: u16,
    /// Raw FFID family/pitch flags.
    pub family_id: u8,
    /// Visual weight in the source record (400 normal, 700 bold).
    pub weight: i16,
    /// Windows character-set identifier.
    pub charset: u8,
    pub name: String,
    pub alternate_name: Option<String>,
    pub panose: [u8; 10],
    pub signature: [u8; 24],
}

/// Ordered font table from the document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FontTable {
    fonts: Vec<FontEntry>,
}

impl FontTable {
    /// Parses the FIB-referenced non-extended `SttbfFfn`.
    ///
    /// # Errors
    ///
    /// Returns a typed error for invalid STTB/FFN lengths, unsupported charset
    /// values, invalid UTF-16 names, inconsistent aliases, or resource limits.
    pub fn parse(fib: &Fib, table_stream: &[u8], limits: DocLimits) -> Result<Self> {
        let location = fib
            .locations
            .fonts()
            .filter(|location| !location.is_empty())
            .ok_or_else(|| DocError::InvalidFont("SttbfFfn location is empty".into()))?;
        let data = checked_slice(table_stream, location.offset, location.length, "SttbfFfn")?;
        Self::parse_bytes(data, limits)
    }

    fn parse_bytes(data: &[u8], limits: DocLimits) -> Result<Self> {
        if data.starts_with(&[0xFF, 0xFF]) {
            return Err(DocError::InvalidFont(
                "SttbfFfn unexpectedly uses extended STTB framing".into(),
            ));
        }
        let count = usize::from(read_u16(data, 0, "SttbfFfn.cData")?);
        if count > 0x7FF0 || count > limits.max_fonts {
            return Err(DocError::ResourceLimit {
                resource: "font",
                actual: u64::try_from(count).unwrap_or(u64::MAX),
                limit: u64::try_from(limits.max_fonts.min(0x7FF0)).unwrap_or(u64::MAX),
            });
        }
        let extra = read_u16(data, 2, "SttbfFfn.cbExtra")?;
        if extra != 0 {
            return Err(DocError::InvalidFont(format!(
                "SttbfFfn.cbExtra is {extra}; expected zero"
            )));
        }
        let mut offset = 4_usize;
        let mut fonts = Vec::with_capacity(count);
        for index in 0..count {
            let length = usize::from(*data.get(offset).ok_or_else(|| {
                DocError::InvalidFont(format!("missing FFN {index} length at byte {offset}"))
            })?);
            offset += 1;
            let end = offset
                .checked_add(length)
                .ok_or_else(|| DocError::InvalidFont("FFN end overflow".into()))?;
            let bytes = data.get(offset..end).ok_or_else(|| {
                DocError::InvalidFont(format!(
                    "FFN {index} ends at {end}, beyond font table length {}",
                    data.len()
                ))
            })?;
            fonts.push(parse_font(
                u16::try_from(index)
                    .map_err(|_| DocError::InvalidFont("font index overflow".into()))?,
                bytes,
            )?);
            offset = end;
        }
        if offset != data.len() {
            return Err(DocError::InvalidFont(format!(
                "{} trailing bytes follow SttbfFfn entries",
                data.len() - offset
            )));
        }
        Ok(Self { fonts })
    }

    #[must_use]
    pub fn fonts(&self) -> &[FontEntry] {
        &self.fonts
    }

    #[must_use]
    pub fn get(&self, index: u16) -> Option<&FontEntry> {
        self.fonts.get(usize::from(index))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.fonts.len()
    }
}

fn parse_font(index: u16, data: &[u8]) -> Result<FontEntry> {
    if data.len() < FFN_FIXED_SIZE + 2 || !(data.len() - FFN_FIXED_SIZE).is_multiple_of(2) {
        return Err(DocError::InvalidFont(format!(
            "FFN {index} length {} cannot contain its fixed fields and UTF-16 name",
            data.len()
        )));
    }
    let family_id = data[0];
    let weight = i16::from_le_bytes([data[1], data[2]]);
    if !(0..=1000).contains(&weight) {
        return Err(DocError::InvalidFont(format!(
            "FFN {index} weight {weight} is outside 0..=1000"
        )));
    }
    let charset = data[3];
    if !matches!(
        charset,
        0 | 1
            | 2
            | 77
            | 128
            | 129
            | 130
            | 134
            | 136
            | 161
            | 162
            | 163
            | 177
            | 178
            | 186
            | 204
            | 222
            | 238
            | 255
    ) {
        return Err(DocError::InvalidFont(format!(
            "FFN {index} has unsupported charset {charset}"
        )));
    }
    let alternate_index = usize::from(data[4]);
    let panose = data[5..15].try_into().unwrap();
    let signature = data[15..39].try_into().unwrap();
    let units = data[FFN_FIXED_SIZE..]
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let name_end = units.iter().position(|unit| *unit == 0).ok_or_else(|| {
        DocError::InvalidFont(format!("FFN {index} primary name is not null-terminated"))
    })?;
    if name_end == 0 {
        return Err(DocError::InvalidFont(format!(
            "FFN {index} primary name is empty"
        )));
    }
    let name = decode_name(index, "primary", &units[..name_end])?;
    let alternate_name = if alternate_index == 0 {
        if name_end + 1 != units.len() {
            return Err(DocError::InvalidFont(format!(
                "FFN {index} has trailing name data but ixchSzAlt is zero"
            )));
        }
        None
    } else {
        if alternate_index != name_end + 1 || alternate_index >= units.len() {
            return Err(DocError::InvalidFont(format!(
                "FFN {index} ixchSzAlt {alternate_index} does not follow its primary name"
            )));
        }
        let alternate_end = units[alternate_index..]
            .iter()
            .position(|unit| *unit == 0)
            .map(|relative| alternate_index + relative)
            .ok_or_else(|| {
                DocError::InvalidFont(format!("FFN {index} alternate name is not terminated"))
            })?;
        if alternate_end == alternate_index || alternate_end + 1 != units.len() {
            return Err(DocError::InvalidFont(format!(
                "FFN {index} alternate name is empty or has trailing data"
            )));
        }
        Some(decode_name(
            index,
            "alternate",
            &units[alternate_index..alternate_end],
        )?)
    };
    Ok(FontEntry {
        index,
        family_id,
        weight,
        charset,
        name,
        alternate_name,
        panose,
        signature,
    })
}

fn decode_name(index: u16, kind: &str, units: &[u16]) -> Result<String> {
    String::from_utf16(units)
        .map_err(|_| DocError::InvalidFont(format!("FFN {index} {kind} name is invalid UTF-16")))
}

fn read_u16(data: &[u8], offset: usize, field: &str) -> Result<u16> {
    let end = offset
        .checked_add(2)
        .ok_or_else(|| DocError::InvalidFont(format!("{field} offset overflow")))?;
    let bytes = data.get(offset..end).ok_or_else(|| {
        DocError::InvalidFont(format!(
            "{field} range [{offset}, {end}) exceeds {} bytes",
            data.len()
        ))
    })?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_primary_and_alternate_unicode_font_names() {
        let first = ffn("Arial", None, 0, 400);
        let second = ffn("SimSun", Some("宋体"), 134, 700);
        let mut table = Vec::new();
        table.extend_from_slice(&2_u16.to_le_bytes());
        table.extend_from_slice(&0_u16.to_le_bytes());
        for font in [&first, &second] {
            table.push(u8::try_from(font.len()).unwrap());
            table.extend_from_slice(font);
        }
        let parsed = FontTable::parse_bytes(&table, DocLimits::default()).unwrap();
        assert_eq!(parsed.get(0).unwrap().name, "Arial");
        assert_eq!(
            parsed.get(1).unwrap().alternate_name.as_deref(),
            Some("宋体")
        );
        assert_eq!(parsed.get(1).unwrap().weight, 700);
    }

    #[test]
    fn rejects_bad_alias_index_and_trailing_bytes() {
        let mut font = ffn("Arial", None, 0, 400);
        font[4] = 2;
        let mut table = vec![1, 0, 0, 0, u8::try_from(font.len()).unwrap()];
        table.extend_from_slice(&font);
        assert!(matches!(
            FontTable::parse_bytes(&table, DocLimits::default()),
            Err(DocError::InvalidFont(_))
        ));
        table[4] = 0;
        table.push(0);
        assert!(matches!(
            FontTable::parse_bytes(&table, DocLimits::default()),
            Err(DocError::InvalidFont(_))
        ));
    }

    fn ffn(name: &str, alternate: Option<&str>, charset: u8, weight: i16) -> Vec<u8> {
        let name_units = name.encode_utf16().collect::<Vec<_>>();
        let mut result = vec![0_u8; FFN_FIXED_SIZE];
        result[1..3].copy_from_slice(&weight.to_le_bytes());
        result[3] = charset;
        if alternate.is_some() {
            result[4] = u8::try_from(name_units.len() + 1).unwrap();
        }
        for unit in name_units {
            result.extend_from_slice(&unit.to_le_bytes());
        }
        result.extend_from_slice(&0_u16.to_le_bytes());
        if let Some(alternate) = alternate {
            for unit in alternate.encode_utf16() {
                result.extend_from_slice(&unit.to_le_bytes());
            }
            result.extend_from_slice(&0_u16.to_le_bytes());
        }
        result
    }
}
