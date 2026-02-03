//! UU-encoding detection and decoding
//!
//! SEC filings embed binary files (PDF, images, etc.) using UU-encoding.

/// Check if content is UU-encoded by looking for "begin XXX filename" pattern
/// in the first two lines where XXX is a 3-digit Unix permission mode.
pub fn is_uuencoded(content: &[u8]) -> bool {
    // Find first non-whitespace
    let content = trim_start(content);
    
    // Check first line
    if check_begin_line(content) {
        return true;
    }
    
    // Check second line
    if let Some(newline_pos) = memchr::memchr(b'\n', content) {
        if check_begin_line(&content[newline_pos + 1..]) {
            return true;
        }
    }
    
    false
}

fn check_begin_line(line: &[u8]) -> bool {
    if !line.starts_with(b"begin ") {
        return false;
    }
    
    // Need at least "begin XXX f" = 11 chars
    if line.len() < 11 {
        return false;
    }
    
    // Check that positions 6,7,8 are digits (the permission mode)
    let mode = &line[6..9];
    mode.iter().all(|&b| b.is_ascii_digit())
}

/// Decode UU-encoded content
/// 
/// UU-encoding format:
/// - First line: "begin <mode> <filename>"
/// - Data lines: first char is length (32 + n), followed by encoded data
/// - Last line: "end"
pub fn decode_uuencoded(content: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(content.len() * 3 / 4);
    
    // Convert to string for line processing (UU is ASCII-safe)
    let text: String = match std::str::from_utf8(content) {
        Ok(s) => s.to_string(),
        Err(_) => String::from_utf8_lossy(content).into_owned(),
    };
    
    let mut lines = text.lines();
    
    // Find the "begin" line
    let mut found_begin = false;
    for line in lines.by_ref() {
        if line.starts_with("begin ") {
            found_begin = true;
            break;
        }
    }
    
    if !found_begin {
        return result;
    }
    
    // Process data lines
    for line in lines {
        let line = line.trim();
        
        if line.is_empty() || line == "end" {
            break;
        }
        
        if let Some(decoded) = decode_uu_line(line) {
            result.extend_from_slice(&decoded);
        }
    }
    
    result
}

/// Decode a single UU-encoded line
fn decode_uu_line(line: &str) -> Option<Vec<u8>> {
    let bytes = line.as_bytes();
    
    if bytes.is_empty() {
        return None;
    }
    
    // First character indicates number of bytes on this line
    let length_char = bytes[0];
    if length_char < 32 || length_char > 95 {
        return None;
    }
    
    let expected_bytes = ((length_char - 32) & 63) as usize;
    if expected_bytes == 0 {
        return None;
    }
    
    let mut result = Vec::with_capacity(expected_bytes);
    let encoded = &bytes[1..];
    
    // Process 4-character groups into 3 bytes
    let mut i = 0;
    while i + 3 < encoded.len() && result.len() < expected_bytes {
        let c0 = decode_char(encoded[i]);
        let c1 = decode_char(encoded[i + 1]);
        let c2 = decode_char(encoded[i + 2]);
        let c3 = decode_char(encoded[i + 3]);
        
        if result.len() < expected_bytes {
            result.push((c0 << 2) | (c1 >> 4));
        }
        if result.len() < expected_bytes {
            result.push((c1 << 4) | (c2 >> 2));
        }
        if result.len() < expected_bytes {
            result.push((c2 << 6) | c3);
        }
        
        i += 4;
    }
    
    // Truncate to expected length (handles padding)
    result.truncate(expected_bytes);
    
    Some(result)
}

/// Decode a single UU character to its 6-bit value
#[inline]
fn decode_char(c: u8) -> u8 {
    // UU encoding: value = (char - 32) & 63
    // Valid chars are 32-95 (space to underscore)
    if c >= 32 && c <= 95 {
        (c - 32) & 63
    } else {
        0
    }
}

/// Trim leading whitespace from byte slice
fn trim_start(data: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < data.len() {
        match data[start] {
            b' ' | b'\t' | b'\n' | b'\r' => start += 1,
            _ => break,
        }
    }
    &data[start..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_uuencoded() {
        assert!(is_uuencoded(b"begin 644 test.pdf\n"));
        assert!(is_uuencoded(b"  \n  begin 644 test.pdf\n"));
        assert!(is_uuencoded(b"\nbegin 644 test.pdf\n"));
        assert!(!is_uuencoded(b"not uuencoded content"));
        assert!(!is_uuencoded(b"begin abc test.pdf\n")); // non-numeric mode
    }

    #[test]
    fn test_decode_simple() {
        // "Cat" encoded in UU
        let encoded = b"begin 644 test.txt\n#0V%T\n`\nend\n";
        let decoded = decode_uuencoded(encoded);
        assert_eq!(decoded, b"Cat");
    }

    #[test]
    fn test_decode_char() {
        assert_eq!(decode_char(b' '), 0);  // space = 0
        assert_eq!(decode_char(b'!'), 1);  // ! = 1
        assert_eq!(decode_char(b'`'), 0);  // backtick = 0 (also used for 0)
    }
}