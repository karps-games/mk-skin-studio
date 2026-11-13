pub const MAX_DECODED: usize = 64 << 20;

pub fn is_yaz0(src: &[u8]) -> bool {
    src.len() >= 16 && &src[..4] == b"Yaz0"
}

pub fn decode(src: &[u8]) -> Result<Vec<u8>, String> {
    if !is_yaz0(src) {
        return Err("E:file.format".into());
    }
    let size = u32::from_be_bytes(src[4..8].try_into().unwrap()) as usize;
    if size == 0 || size > MAX_DECODED {
        return Err("E:file.format".into());
    }
    let mut dst = vec![0u8; size];
    szs::decode_into(&mut dst, src).map_err(|_| "E:file.format")?;
    Ok(dst)
}

pub fn encode(src: &[u8]) -> Result<Vec<u8>, String> {
    if src.len() > MAX_DECODED {
        return Err("E:file.format".into());
    }
    szs::encode(src, szs::EncodeAlgo::MKW).map_err(|_| "E:build.compress".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaz0_round_trip_synthetic() {
        let mut data = Vec::new();
        for i in 0..50_000u32 {
            data.extend_from_slice(&(i % 251).to_be_bytes());
            if i % 7 == 0 {
                data.extend_from_slice(b"repeat-repeat-repeat");
            }
        }
        let packed = encode(&data).unwrap();
        assert!(is_yaz0(&packed));
        assert!(packed.len() < data.len());
        assert_eq!(decode(&packed).unwrap(), data);
    }

    #[test]
    fn yaz0_rejects_garbage_and_huge_headers() {
        assert!(decode(b"not yaz0 at all!").is_err());
        let mut h = b"Yaz0".to_vec();
        h.extend_from_slice(&(u32::MAX).to_be_bytes());
        h.extend_from_slice(&[0; 8]);
        assert!(decode(&h).is_err());
    }
}
