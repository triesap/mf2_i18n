use alloc::vec::Vec;

use crate::{CoreError, CoreResult};

const PACK_MAGIC: &[u8; 8] = b"MF2PACK\0";
const HEADER_LEN: usize = 8 + 2 + 1 + 4 + 32 + 4 + 4 + 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackKind {
    Base,
    Overlay,
    IcuData,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackHeader {
    pub schema_version: u16,
    pub pack_kind: PackKind,
    pub flags: u32,
    pub id_map_hash: [u8; 32],
    pub locale_tag_sidx: u32,
    pub parent_tag_sidx: Option<u32>,
    pub build_epoch_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionEntry {
    pub section_type: u8,
    pub offset: u32,
    pub length: u32,
}

pub fn parse_pack_header(input: &[u8]) -> CoreResult<(PackHeader, usize)> {
    if input.len() < HEADER_LEN {
        return Err(CoreError::InvalidInput("pack header too short"));
    }
    if &input[..PACK_MAGIC.len()] != PACK_MAGIC {
        return Err(CoreError::InvalidInput("pack magic mismatch"));
    }
    let mut cursor = PACK_MAGIC.len();
    let schema_version = read_u16(input, &mut cursor)?;
    let kind = input
        .get(cursor)
        .copied()
        .ok_or(CoreError::InvalidInput("pack header missing kind"))?;
    cursor += 1;
    let pack_kind = match kind {
        0 => PackKind::Base,
        1 => PackKind::Overlay,
        2 => PackKind::IcuData,
        _ => return Err(CoreError::Unsupported("unknown pack kind")),
    };
    let flags = read_u32(input, &mut cursor)?;
    let mut id_map_hash = [0u8; 32];
    id_map_hash.copy_from_slice(&input[cursor..cursor + 32]);
    cursor += 32;
    let locale_tag_sidx = read_u32(input, &mut cursor)?;
    let parent_tag_raw = read_u32(input, &mut cursor)?;
    let parent_tag_sidx = if parent_tag_raw == u32::MAX {
        None
    } else {
        Some(parent_tag_raw)
    };
    let build_epoch_ms = read_u64(input, &mut cursor)?;

    Ok((
        PackHeader {
            schema_version,
            pack_kind,
            flags,
            id_map_hash,
            locale_tag_sidx,
            parent_tag_sidx,
            build_epoch_ms,
        },
        cursor,
    ))
}

pub fn parse_section_directory(
    input: &[u8],
    start: usize,
    count: usize,
) -> CoreResult<Vec<SectionEntry>> {
    let mut cursor = start;
    let mut sections = Vec::with_capacity(count);
    for _ in 0..count {
        let section_type = input
            .get(cursor)
            .copied()
            .ok_or(CoreError::InvalidInput("section directory out of bounds"))?;
        cursor += 1;
        let offset = read_u32(input, &mut cursor)?;
        let length = read_u32(input, &mut cursor)?;
        sections.push(SectionEntry {
            section_type,
            offset,
            length,
        });
    }
    Ok(sections)
}

fn read_u16(input: &[u8], cursor: &mut usize) -> CoreResult<u16> {
    let end = *cursor + 2;
    if end > input.len() {
        return Err(CoreError::InvalidInput("unexpected eof"));
    }
    let value = u16::from_le_bytes([input[*cursor], input[*cursor + 1]]);
    *cursor = end;
    Ok(value)
}

fn read_u32(input: &[u8], cursor: &mut usize) -> CoreResult<u32> {
    let end = *cursor + 4;
    if end > input.len() {
        return Err(CoreError::InvalidInput("unexpected eof"));
    }
    let value = u32::from_le_bytes([
        input[*cursor],
        input[*cursor + 1],
        input[*cursor + 2],
        input[*cursor + 3],
    ]);
    *cursor = end;
    Ok(value)
}

fn read_u64(input: &[u8], cursor: &mut usize) -> CoreResult<u64> {
    let end = *cursor + 8;
    if end > input.len() {
        return Err(CoreError::InvalidInput("unexpected eof"));
    }
    let value = u64::from_le_bytes([
        input[*cursor],
        input[*cursor + 1],
        input[*cursor + 2],
        input[*cursor + 3],
        input[*cursor + 4],
        input[*cursor + 5],
        input[*cursor + 6],
        input[*cursor + 7],
    ]);
    *cursor = end;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use super::{PACK_MAGIC, PackKind, SectionEntry, parse_pack_header, parse_section_directory};

    fn build_header(kind: u8) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(PACK_MAGIC);
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.push(kind);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 32]);
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(&42u64.to_le_bytes());
        bytes
    }

    #[test]
    fn parses_pack_header() {
        let bytes = build_header(0);
        let (header, cursor) = parse_pack_header(&bytes).expect("valid header");
        assert_eq!(header.schema_version, 0);
        assert_eq!(header.pack_kind, PackKind::Base);
        assert_eq!(header.locale_tag_sidx, 1);
        assert_eq!(header.parent_tag_sidx, None);
        assert_eq!(header.build_epoch_ms, 42);
        assert_eq!(cursor, bytes.len());
    }

    #[test]
    fn rejects_magic_mismatch() {
        let mut bytes = build_header(0);
        bytes[0] = b'X';
        let err = parse_pack_header(&bytes).expect_err("magic mismatch");
        assert_eq!(err, crate::CoreError::InvalidInput("pack magic mismatch"));
    }

    #[test]
    fn parses_section_directory() {
        let mut bytes = build_header(1);
        bytes.push(2);
        bytes.extend_from_slice(&10u32.to_le_bytes());
        bytes.extend_from_slice(&4u32.to_le_bytes());
        let sections = parse_section_directory(&bytes, bytes.len() - 9, 1).expect("sections");
        assert_eq!(
            sections,
            vec![SectionEntry {
                section_type: 2,
                offset: 10,
                length: 4
            }]
        );
    }
}
