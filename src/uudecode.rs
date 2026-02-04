//! UU-encoding detection and decoding
//!
//! SEC filings embed binary files (PDF, images, etc.) using UU-encoding.

/// Error types for uudecode operations
#[derive(Debug, PartialEq)]
pub enum UuDecodeError {
    IllegalChar,
    TrailingGarbage,
}

impl std::fmt::Display for UuDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UuDecodeError::IllegalChar => write!(f, "Illegal char"),
            UuDecodeError::TrailingGarbage => write!(f, "Trailing garbage"),
        }
    }
}

impl std::error::Error for UuDecodeError {}

/// Decode a line of uuencoded data.
/// 
/// The first character encodes the binary data length (in bytes).
/// Each subsequent character encodes 6 bits using "excess-space" encoding
/// where space (32) represents 0.
/// 
/// Valid characters are in range [32, 96] (space through backtick).
pub fn a2b_uu(data: &[u8]) -> Result<Vec<u8>, UuDecodeError> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    let bin_len = ((data[0].wrapping_sub(b' ')) & 0o77) as usize;
    
    let mut bin_data = Vec::with_capacity(bin_len);
    let mut leftbits = 0;
    let mut leftchar: u32 = 0;
    let mut remaining = bin_len;
    
    let mut ascii_idx = 1;
    let ascii_len = data.len();
    
    while remaining > 0 {
        // Get character or 0 if past end
        let this_ch = if ascii_idx < ascii_len {
            let byte = data[ascii_idx];
            ascii_idx += 1;
            
            if byte == b'\n' || byte == b'\r' {
                0u8
            } else {
                if byte < b' ' || byte > (b' ' + 64) {
                    return Err(UuDecodeError::IllegalChar);
                }
                (byte - b' ') & 0o77
            }
        } else {
            // Past end of input - use 0
            0u8
        };
        
        leftchar = (leftchar << 6) | (this_ch as u32);
        leftbits += 6;
        
        if leftbits >= 8 {
            leftbits -= 8;
            bin_data.push(((leftchar >> leftbits) & 0xff) as u8);
            leftchar &= (1 << leftbits) - 1;
            remaining -= 1;
        }
    }
    
    // Trailing garbage check...
    let bytes_processed = bin_len;
    let chars_needed = (bytes_processed * 8 + 5) / 6;
    let start_check = 1 + chars_needed;
    
    if start_check < data.len() {
        for &byte in &data[start_check..] {
            if byte != b' ' && byte != (b' ' + 64) && byte != b'\n' && byte != b'\r' {
                return Err(UuDecodeError::TrailingGarbage);
            }
        }
    }
    
    Ok(bin_data)
}
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
        if line.starts_with("begin") {
            found_begin = true;
            break;
        }
    }
    
    if !found_begin {
        return result;
    }
    
    // Process data lines
    for line in lines {
        let stripped = line.trim_end_matches('\r');
        
        if stripped.is_empty() || stripped == "end" {
            break;
        }
        
        if let Some(decoded) = decode_uu_line(stripped) {
            result.extend_from_slice(&decoded);
        }
    }
    
    result
}

/// Decode a single UU-encoded line (matching Python's fallback behavior)

fn decode_uu_line(line: &str) -> Option<Vec<u8>> {
    let clean_line: String = line.chars()
        .filter(|&c| c as u32 >= 32 && c as u32 <= 95)  // Changed from 96 to 95
        .collect();
    
    if clean_line.is_empty() {
        return None;
    }
    
    // Calculate how many encoded characters we need
    let length_char = clean_line.chars().next()?;
    let expected_bytes = ((length_char as u32 - 32) & 63) as usize;
    let nbytes = (expected_bytes * 4 + 5) / 3;  // Number of encoded chars needed
    
    // Only pass the required number of characters to a2b_uu
    let truncated_line: String = clean_line.chars().take(nbytes + 1).collect();  // +1 for length char
    
    a2b_uu(truncated_line.as_bytes()).ok()
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