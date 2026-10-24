pub(crate) fn encode_seg(segment: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 0x0f) as usize] as char);
            }
        }
    }
    out
}

pub(crate) fn encode_object_id(name: &str) -> String {
    encode_seg(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_encodes_slashes() {
        assert_eq!(
            encode_object_id("photos/my cat.jpeg"),
            "photos%2Fmy%20cat.jpeg"
        );
        assert_eq!(encode_seg("é"), "%C3%A9");
        assert_eq!(encode_seg("+"), "%2B");
    }
}
