//! Write parsed SGML data to TAR archive

use crate::error::Result;
use crate::types::{ParsedSubmission, SubmissionMetadata};
use std::io::{Write, Seek};
use std::path::Path;
use std::fs::File;

/// TAR block size
const BLOCK_SIZE: usize = 512;

/// Write a parsed submission to a TAR file
pub fn write_to_tar(submission: &ParsedSubmission, output_path: impl AsRef<Path>) -> Result<()> {
    let output_path = output_path.as_ref();
    
    // Create parent directories if needed
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    
    let file = File::create(output_path)?;
    write_to_tar_writer(submission, file)
}

/// Write a parsed submission to any Write + Seek destination
pub fn write_to_tar_writer<W: Write + Seek>(submission: &ParsedSubmission, mut writer: W) -> Result<()> {
    // Clone metadata so we can modify it with byte positions
    let mut metadata = submission.metadata.clone();
    
    // Calculate document positions in the TAR
    calculate_tar_positions(&mut metadata, &submission.documents)?;
    
    // Serialize metadata to JSON
    let metadata_json = serde_json::to_string(&metadata)?;
    let metadata_bytes = metadata_json.as_bytes();
    
    // Write metadata.json entry
    write_tar_entry(&mut writer, "metadata.json", metadata_bytes)?;
    
    // Write each document
    for (i, content) in submission.documents.iter().enumerate() {
        let doc_meta = &metadata.documents[i];
        
        // Get filename: use filename field, or fallback to sequence.txt
        let filename = doc_meta.filename()
            .map(|s| s.to_string())
            .or_else(|| doc_meta.sequence().map(|s| format!("{}.txt", s)))
            .unwrap_or_else(|| format!("{}.txt", i + 1));
        
        write_tar_entry(&mut writer, &filename, content)?;
    }
    
    // Write TAR end-of-archive markers (two zero blocks)
    let zero_block = [0u8; BLOCK_SIZE];
    writer.write_all(&zero_block)?;
    writer.write_all(&zero_block)?;
    
    Ok(())
}

/// Calculate byte positions for each document in the TAR
fn calculate_tar_positions(metadata: &mut SubmissionMetadata, documents: &[Vec<u8>]) -> Result<()> {
    // Step 1: Insert placeholder positions (10-digit) to get accurate JSON size
    for doc in &mut metadata.documents {
        doc.start_byte = Some("9999999999".to_string());
        doc.end_byte = Some("9999999999".to_string());
    }
    
    // Step 2: Calculate metadata JSON size with placeholders
    let placeholder_json = serde_json::to_string(&metadata)?;
    let metadata_size = placeholder_json.len();
    
    // Step 3: Calculate positions
    // After metadata.json: 512-byte header + content + padding to 512 boundary
    let metadata_padded = metadata_size + pad_to_block(metadata_size);
    let mut current_pos = BLOCK_SIZE + metadata_padded; // header + padded content
    
    // Step 4: Calculate each document's position
    for (i, content) in documents.iter().enumerate() {
        let doc_size = content.len();
        
        // Document starts after its 512-byte header
        let start_byte = current_pos + BLOCK_SIZE;
        let end_byte = start_byte + doc_size;
        
        // Update metadata with zero-padded 10-digit positions
        metadata.documents[i].start_byte = Some(format!("{:010}", start_byte));
        metadata.documents[i].end_byte = Some(format!("{:010}", end_byte));
        
        // Move to next entry: header + content + padding
        let content_padded = doc_size + pad_to_block(doc_size);
        current_pos += BLOCK_SIZE + content_padded;
    }
    
    Ok(())
}

/// Calculate padding needed to reach next block boundary
fn pad_to_block(size: usize) -> usize {
    let remainder = size % BLOCK_SIZE;
    if remainder == 0 {
        0
    } else {
        BLOCK_SIZE - remainder
    }
}

/// Write a single TAR entry (header + content + padding)
fn write_tar_entry<W: Write>(writer: &mut W, filename: &str, content: &[u8]) -> Result<()> {
    // Build TAR header
    let header = build_tar_header(filename, content.len())?;
    writer.write_all(&header)?;
    
    // Write content
    writer.write_all(content)?;
    
    // Write padding to next block boundary
    let padding_size = pad_to_block(content.len());
    if padding_size > 0 {
        let padding = vec![0u8; padding_size];
        writer.write_all(&padding)?;
    }
    
    Ok(())
}

/// Build a USTAR TAR header
fn build_tar_header(filename: &str, size: usize) -> Result<[u8; BLOCK_SIZE]> {
    let mut header = [0u8; BLOCK_SIZE];
    
    // File name (bytes 0-99)
    let name_bytes = filename.as_bytes();
    let name_len = name_bytes.len().min(100);
    header[..name_len].copy_from_slice(&name_bytes[..name_len]);
    
    // File mode (bytes 100-107): "0000644\0" (rw-r--r--)
    header[100..107].copy_from_slice(b"0000644");
    header[107] = 0;
    
    // Owner UID (bytes 108-115): "0000000\0"
    header[108..115].copy_from_slice(b"0000000");
    header[115] = 0;
    
    // Owner GID (bytes 116-123): "0000000\0"
    header[116..123].copy_from_slice(b"0000000");
    header[123] = 0;
    
    // File size in octal (bytes 124-135)
    let size_str = format!("{:011o}", size);
    header[124..135].copy_from_slice(size_str.as_bytes());
    header[135] = 0;
    
    // Modification time (bytes 136-147): use 0
    header[136..147].copy_from_slice(b"00000000000");
    header[147] = 0;
    
    // Checksum placeholder (bytes 148-155): spaces for calculation
    header[148..156].copy_from_slice(b"        ");
    
    // Type flag (byte 156): '0' = regular file
    header[156] = b'0';
    
    // Link name (bytes 157-256): empty
    // Already zeroed
    
    // USTAR magic (bytes 257-262): "ustar\0"
    header[257..263].copy_from_slice(b"ustar\0");
    
    // USTAR version (bytes 263-264): "00"
    header[263..265].copy_from_slice(b"00");
    
    // Owner user name (bytes 265-296): empty
    // Owner group name (bytes 297-328): empty
    // Device major/minor (bytes 329-344): empty
    // Prefix (bytes 345-499): empty
    // All already zeroed
    
    // Calculate and write checksum
    let checksum: u32 = header.iter().map(|&b| b as u32).sum();
    let checksum_str = format!("{:06o}\0 ", checksum);
    header[148..156].copy_from_slice(checksum_str.as_bytes());
    
    Ok(header)
}

/// Write SGML file directly to TAR (parse + write in one step)
pub fn write_sgml_file_to_tar(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    options: crate::ParseOptions,
) -> Result<()> {
    let submission = crate::parse_sgml_file(input_path, options)?;
    write_to_tar(&submission, output_path)
}

/// Write SGML bytes directly to TAR (parse + write in one step)
pub fn write_sgml_bytes_to_tar(
    input_bytes: &[u8],
    output_path: impl AsRef<Path>,
    options: crate::ParseOptions,
) -> Result<()> {
    let submission = crate::parse_sgml(input_bytes, options)?;
    write_to_tar(&submission, output_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_sgml, ParseOptions};
    use std::io::Cursor;

    #[test]
    fn test_pad_to_block() {
        assert_eq!(pad_to_block(0), 0);
        assert_eq!(pad_to_block(1), 511);
        assert_eq!(pad_to_block(512), 0);
        assert_eq!(pad_to_block(513), 511);
        assert_eq!(pad_to_block(1024), 0);
    }

    #[test]
    fn test_build_tar_header() {
        let header = build_tar_header("test.txt", 100).unwrap();
        
        // Check filename
        assert_eq!(&header[0..8], b"test.txt");
        
        // Check magic
        assert_eq!(&header[257..262], b"ustar");
        
        // Check type flag
        assert_eq!(header[156], b'0');
    }

    #[test]
    fn test_write_to_tar() {
        let sgml = br#"<SEC-DOCUMENT>test.txt
<DOCUMENT>
<TYPE>10-K
<SEQUENCE>1
<FILENAME>form10k.htm
<TEXT>
Test content here.
</TEXT>
</DOCUMENT>
"#;

        let submission = parse_sgml(sgml, ParseOptions::new()).unwrap();
        
        let mut buffer = Cursor::new(Vec::new());
        write_to_tar_writer(&submission, &mut buffer).unwrap();
        
        let tar_data = buffer.into_inner();
        
        // Should have at least: metadata header + metadata + doc header + doc + 2 end blocks
        assert!(tar_data.len() >= BLOCK_SIZE * 4);
        
        // First entry should be metadata.json
        assert_eq!(&tar_data[0..13], b"metadata.json");
    }

    #[test]
    fn test_position_calculation() {
        let sgml = br#"<SEC-DOCUMENT>test.txt
<DOCUMENT>
<TYPE>10-K
<SEQUENCE>1
<FILENAME>doc1.htm
<TEXT>
First document content.
</TEXT>
</DOCUMENT>
<DOCUMENT>
<TYPE>EX-99
<SEQUENCE>2
<FILENAME>doc2.htm
<TEXT>
Second document.
</TEXT>
</DOCUMENT>
"#;

        let submission = parse_sgml(sgml, ParseOptions::new()).unwrap();
        
        let mut metadata = submission.metadata.clone();
        calculate_tar_positions(&mut metadata, &submission.documents).unwrap();
        
        // Positions should be 10-digit strings
        assert_eq!(metadata.documents[0].start_byte.as_ref().unwrap().len(), 10);
        assert_eq!(metadata.documents[0].end_byte.as_ref().unwrap().len(), 10);
        
        // Second doc should start after first
        let doc1_end: usize = metadata.documents[0].end_byte.as_ref().unwrap().parse().unwrap();
        let doc2_start: usize = metadata.documents[1].start_byte.as_ref().unwrap().parse().unwrap();
        assert!(doc2_start > doc1_end);
    }
}