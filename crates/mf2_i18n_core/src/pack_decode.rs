use alloc::string::String;
use alloc::vec::Vec;

use crate::{CoreError, CoreResult, MessageId};

pub fn decode_string_pool(input: &[u8]) -> CoreResult<Vec<String>> {
    let mut cursor = 0usize;
    let count = read_u32(input, &mut cursor)? as usize;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let len = read_u32(input, &mut cursor)? as usize;
        let end = cursor + len;
        if end > input.len() {
            return Err(CoreError::InvalidInput("string pool out of bounds"));
        }
        let bytes = &input[cursor..end];
        let text = core::str::from_utf8(bytes)
            .map_err(|_| CoreError::InvalidInput("string pool invalid utf8"))?;
        let text = String::from(text);
        entries.push(text);
        cursor = end;
    }
    Ok(entries)
}

pub fn decode_dense_index(input: &[u8]) -> CoreResult<Vec<u32>> {
    let mut cursor = 0usize;
    let count = read_u32(input, &mut cursor)? as usize;
    let mut offsets = Vec::with_capacity(count);
    for _ in 0..count {
        offsets.push(read_u32(input, &mut cursor)?);
    }
    Ok(offsets)
}

pub fn decode_sparse_index(input: &[u8]) -> CoreResult<Vec<(MessageId, u32)>> {
    let mut cursor = 0usize;
    let count = read_u32(input, &mut cursor)? as usize;
    let mut pairs = Vec::with_capacity(count);
    for _ in 0..count {
        let id = read_u32(input, &mut cursor)?;
        let offset = read_u32(input, &mut cursor)?;
        pairs.push((MessageId::new(id), offset));
    }
    Ok(pairs)
}

pub fn read_bytecode_at<'a>(blob: &'a [u8], offset: u32) -> CoreResult<&'a [u8]> {
    let offset = offset as usize;
    if offset + 4 > blob.len() {
        return Err(CoreError::InvalidInput("bytecode offset out of bounds"));
    }
    let mut cursor = offset;
    let len = read_u32(blob, &mut cursor)? as usize;
    let end = cursor + len;
    if end > blob.len() {
        return Err(CoreError::InvalidInput("bytecode length out of bounds"));
    }
    Ok(&blob[cursor..end])
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

#[cfg(test)]
mod tests {
    use alloc::string::ToString;
    use alloc::vec;
    use alloc::vec::Vec;

    use super::{decode_dense_index, decode_sparse_index, decode_string_pool, read_bytecode_at};
    use crate::MessageId;

    #[test]
    fn decodes_string_pool() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(b"foo");
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(b"bar");
        let pool = decode_string_pool(&bytes).expect("pool");
        assert_eq!(pool, vec!["foo".to_string(), "bar".to_string()]);
    }

    #[test]
    fn decodes_dense_index() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&10u32.to_le_bytes());
        bytes.extend_from_slice(&20u32.to_le_bytes());
        let index = decode_dense_index(&bytes).expect("index");
        assert_eq!(index, vec![10, 20]);
    }

    #[test]
    fn decodes_sparse_index() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&10u32.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&20u32.to_le_bytes());
        let index = decode_sparse_index(&bytes).expect("index");
        assert_eq!(
            index,
            vec![(MessageId::new(1), 10), (MessageId::new(2), 20)]
        );
    }

    #[test]
    fn reads_bytecode_blob() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&[1, 2, 3, 4]);
        let slice = read_bytecode_at(&bytes, 0).expect("slice");
        assert_eq!(slice, &[1, 2, 3, 4]);
    }
}
