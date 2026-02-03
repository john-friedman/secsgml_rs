//! Core SGML parsing logic

use crate::error::{ParseError, Result};
use crate::header_mappings::{standardize_key, transform_value};
use crate::types::*;
use crate::uudecode::{decode_uuencoded, is_uuencoded};
use memchr::memmem;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

// Tag patterns for fast searching
const DOC_START: &[u8] = b"<DOCUMENT>";
const DOC_END: &[u8] = b"</DOCUMENT>";
const TEXT_START: &[u8] = b"<TEXT>";
const TEXT_END: &[u8] = b"</TEXT>";

/// Parse SGML from a file path using memory mapping
pub fn parse_sgml_file(path: impl AsRef<Path>, options: ParseOptions) -> Result<ParsedSubmission> {
    let file = std::fs::File::open(path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    parse_sgml(&mmap, options)
}

/// Parse SGML from a byte slice
pub fn parse_sgml(data: &[u8], options: ParseOptions) -> Result<ParsedSubmission> {
    // Find all document boundaries first (fast SIMD scan)
    let doc_boundaries = find_document_boundaries(data);
    
    // Parse submission header (everything before first <DOCUMENT>)
    let header_end = doc_boundaries.first().map(|(start, _)| *start).unwrap_or(data.len());
    let (mut submission_meta, format) = parse_submission_metadata(&data[..header_end], options.standardize_metadata)?;
    
    // Parse documents (potentially in parallel)
    let parsed_docs: Vec<(DocumentMetadata, Vec<u8>)> = if options.parallel && doc_boundaries.len() > 1 {
        doc_boundaries
            .par_iter()
            .map(|(start, end)| parse_single_document(&data[*start..*end], format, options.standardize_metadata))
            .collect::<Result<Vec<_>>>()?
    } else {
        doc_boundaries
            .iter()
            .map(|(start, end)| parse_single_document(&data[*start..*end], format, options.standardize_metadata))
            .collect::<Result<Vec<_>>>()?
    };
    
    // Split metadata and content
    let (doc_metas, documents): (Vec<_>, Vec<_>) = parsed_docs.into_iter().unzip();
    
    // Apply document type filter
    let (doc_metas, documents) = apply_filter(doc_metas, documents, &options);
    
    submission_meta.documents = doc_metas;
    
    Ok(ParsedSubmission {
        metadata: submission_meta,
        documents,
        format,
    })
}

/// Find all (start, end) byte positions of <DOCUMENT>...</DOCUMENT> blocks
fn find_document_boundaries(data: &[u8]) -> Vec<(usize, usize)> {
    let mut boundaries = Vec::new();
    let finder_start = memmem::Finder::new(DOC_START);
    let finder_end = memmem::Finder::new(DOC_END);
    
    let mut pos = 0;
    while let Some(start) = finder_start.find(&data[pos..]) {
        let abs_start = pos + start;
        
        // Find corresponding </DOCUMENT>
        if let Some(end) = finder_end.find(&data[abs_start..]) {
            let abs_end = abs_start + end + DOC_END.len();
            boundaries.push((abs_start, abs_end));
            pos = abs_end;
        } else {
            break;
        }
    }
    
    boundaries
}

/// Parse a single <DOCUMENT>...</DOCUMENT> block
fn parse_single_document(
    doc_data: &[u8],
    format: SubmissionFormat,
    standardize: bool,
) -> Result<(DocumentMetadata, Vec<u8>)> {
    // Find <TEXT> tag
    let text_start = memmem::find(doc_data, TEXT_START)
        .ok_or_else(|| ParseError::InvalidStructure("Missing <TEXT> tag".into()))?;
    
    // Parse document metadata (between <DOCUMENT> and <TEXT>)
    let meta_slice = &doc_data[DOC_START.len()..text_start];
    let mut doc_meta = parse_document_metadata(meta_slice, standardize);
    
    // Find </TEXT> and extract content
    let content_start = text_start + TEXT_START.len();
    let content_end = memmem::find(&doc_data[content_start..], TEXT_END)
        .map(|pos| content_start + pos)
        .unwrap_or(doc_data.len());
    
    let raw_content = &doc_data[content_start..content_end];
    
    // Check if UU-encoded and decode if needed
    let is_binary = is_uuencoded(raw_content);
    let content = if is_binary {
        decode_uuencoded(raw_content)
    } else {
        clean_document_content(raw_content, format, false).to_vec()
    };
    
    doc_meta.size_bytes = content.len();
    
    Ok((doc_meta, content))
}

/// Parse document metadata block (key-value pairs like <TYPE>10-K)
fn parse_document_metadata(data: &[u8], standardize: bool) -> DocumentMetadata {
    let mut fields = HashMap::new();
    
    for line in data.split(|&b| b == b'\n') {
        let line = trim(line);
        if line.is_empty() {
            continue;
        }
        
        // Parse <KEY>value format
        if line.starts_with(b"<") {
            if let Some((key, value)) = parse_tag_line(line) {
                let key_str = bytes_to_string(key);
                let value_str = bytes_to_string(value);
                
                let (final_key, final_value) = if standardize {
                    (standardize_key(&key_str), transform_value(&key_str, &value_str))
                } else {
                    (key_str, value_str)
                };
                
                fields.insert(final_key, final_value);
            }
        }
    }
    
    DocumentMetadata {
        fields,
        size_bytes: 0,
        start_byte: None,
        end_byte: None,
    }
}

/// Parse a <KEY>value line, returns (key, value)
fn parse_tag_line(line: &[u8]) -> Option<(&[u8], &[u8])> {
    // Find closing >
    let gt_pos = memchr::memchr(b'>', line)?;
    
    // Key is between < and >
    let key = &line[1..gt_pos];
    
    // Value is after >
    let value = trim(&line[gt_pos + 1..]);
    
    Some((key, value))
}

/// Detect submission format from first bytes
fn detect_format(data: &[u8]) -> SubmissionFormat {
    let trimmed = trim_start(data);
    
    if trimmed.starts_with(b"-") {
        SubmissionFormat::TabPrivacy
    } else if trimmed.starts_with(b"<SE") {
        SubmissionFormat::TabDefault
    } else {
        SubmissionFormat::Archive
    }
}

/// Parse submission header metadata
fn parse_submission_metadata(data: &[u8], standardize: bool) -> Result<(SubmissionMetadata, SubmissionFormat)> {
    let format = detect_format(data);
    
    let fields = match format {
        SubmissionFormat::TabPrivacy => {
            // Find end of privacy message (first blank line)
            let privacy_end = find_double_newline(data).unwrap_or(0);
            let privacy_msg = bytes_to_string(&data[..privacy_end]);
            
            let rest = &data[privacy_end..];
            let rest = trim_start(rest);
            
            let mut fields = parse_tab_metadata(rest, standardize);
            fields.insert(
                if standardize { "privacy-enhanced-message".into() } else { "PRIVACY-ENHANCED-MESSAGE".into() },
                MetadataValue::String(privacy_msg),
            );
            fields
        }
        SubmissionFormat::TabDefault => {
            parse_tab_metadata(data, standardize)
        }
        SubmissionFormat::Archive => {
            parse_archive_metadata(data, standardize)
        }
    };
    
    Ok((SubmissionMetadata { fields, documents: Vec::new() }, format))
}

/// Parse tab-delimited format metadata
/// This format uses indentation (tabs) to indicate nesting
fn parse_tab_metadata(data: &[u8], standardize: bool) -> HashMap<String, MetadataValue> {
    let mut root: HashMap<String, MetadataValue> = HashMap::new();
    
    // Track path through nested structure as keys
    let mut path: Vec<String> = Vec::new();
    
    // First, fix line wraparound (lines > 1023 chars are continued)
    let lines = fix_line_wraparound(data);
    
    for line in lines {
        let line_bytes = line.as_bytes();
        if trim(line_bytes).is_empty() {
            continue;
        }
        
        // Count leading tabs for indent level
        let indent_level = line_bytes.iter().take_while(|&&b| b == b'\t').count();
        let line_content = &line[indent_level..];
        let line_content = line_content.trim_end();
        
        if line_content.is_empty() {
            continue;
        }
        
        // Adjust path to current indent level
        path.truncate(indent_level);
        
        // Parse the line
        if let Some(colon_pos) = line_content.find(':') {
            // Check for special SEC-DOCUMENT/SEC-HEADER format: <TAG>value : date
            if line_content.starts_with("<SEC-DOCUMENT>") || line_content.starts_with("<SEC-HEADER>") {
                if let Some((key, value)) = parse_sec_header_line(line_content) {
                    let final_key = if standardize { standardize_key(&key) } else { key };
                    insert_at_path(&mut root, &path, final_key, MetadataValue::String(value));
                }
            } else {
                // Normal KEY: value
                let key = line_content[..colon_pos].trim();
                let value = line_content[colon_pos + 1..].trim();
                
                let final_key = if standardize { standardize_key(key) } else { key.to_string() };
                
                if value.is_empty() {
                    // Section start - add to path
                    insert_at_path(&mut root, &path, final_key.clone(), MetadataValue::Object(HashMap::new()));
                    path.push(final_key);
                } else {
                    // Regular value
                    let final_value = if standardize {
                        transform_value(key, value)
                    } else {
                        value.to_string()
                    };
                    insert_at_path(&mut root, &path, final_key, MetadataValue::String(final_value));
                }
            }
        } else if line_content.starts_with('<') && line_content.contains('>') {
            // Tag format <KEY>value
            if let Some(gt_pos) = line_content.find('>') {
                let key = &line_content[1..gt_pos];
                let value = line_content[gt_pos + 1..].trim();
                
                // Skip closing tags
                if key.starts_with('/') {
                    continue;
                }
                
                let final_key = if standardize { standardize_key(key) } else { key.to_string() };
                let final_value = if standardize {
                    transform_value(key, value)
                } else {
                    value.to_string()
                };
                
                insert_at_path(&mut root, &path, final_key, MetadataValue::String(final_value));
            }
        }
    }
    
    root
}

/// Parse archive format metadata (XML-like tags with explicit closing tags)
fn parse_archive_metadata(data: &[u8], standardize: bool) -> HashMap<String, MetadataValue> {
    let mut root: HashMap<String, MetadataValue> = HashMap::new();
    
    // Track path through nested structure
    let mut path: Vec<String> = Vec::new();
    
    // First pass: identify which tags are sections (have closing tags)
    let keyvals = parse_archive_keyvals(data);
    let section_tags: std::collections::HashSet<&[u8]> = keyvals
        .iter()
        .filter_map(|(key, _)| {
            if key.starts_with(b"/") {
                Some(&key[1..])
            } else {
                None
            }
        })
        .collect();
    
    // Second pass: build nested structure
    for (key, value) in &keyvals {
        // Skip SUBMISSION tag
        if key == b"SUBMISSION" {
            continue;
        }
        
        if key.starts_with(b"/") {
            // Closing tag - pop path
            path.pop();
            continue;
        }
        
        let key_str = bytes_to_string(key);
        let value_str = bytes_to_string(value);
        
        let final_key = if standardize { standardize_key(&key_str) } else { key_str.clone() };
        
        if !value.is_empty() {
            // Has value - it's a field
            let final_value = if standardize {
                transform_value(&key_str, &value_str)
            } else {
                value_str
            };
            insert_at_path(&mut root, &path, final_key, MetadataValue::String(final_value));
        } else if section_tags.contains(key.as_slice()) {
            // Section - create nested object and add to path
            insert_at_path(&mut root, &path, final_key.clone(), MetadataValue::Object(HashMap::new()));
            path.push(final_key);
        } else {
            // Empty field
            insert_at_path(&mut root, &path, final_key, MetadataValue::String(String::new()));
        }
    }
    
    root
}

/// Parse archive format into key-value pairs
fn parse_archive_keyvals(data: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut keyvals = Vec::new();
    
    for line in data.split(|&b| b == b'\n') {
        let line = trim(line);
        if line.is_empty() {
            continue;
        }
        
        // Find pattern [A-Z0-9]>
        if let Some(gt_pos) = find_tag_end(line) {
            let (key, value) = if line.starts_with(b"</") {
                // Closing tag: include the /
                (&line[1..gt_pos + 1], &line[gt_pos + 2..])
            } else if line.starts_with(b"<") {
                (&line[1..gt_pos + 1], &line[gt_pos + 2..])
            } else {
                continue;
            };
            
            keyvals.push((key.to_vec(), trim(value).to_vec()));
        }
    }
    
    keyvals
}

/// Find position of tag end (char before >) that's alphanumeric
fn find_tag_end(line: &[u8]) -> Option<usize> {
    for (i, &b) in line.iter().enumerate() {
        if b == b'>' && i > 0 {
            let prev = line[i - 1];
            if prev.is_ascii_alphanumeric() {
                return Some(i - 1);
            }
        }
    }
    None
}

/// Parse special SEC-DOCUMENT/SEC-HEADER line format
fn parse_sec_header_line(line: &str) -> Option<(String, String)> {
    // Format: <SEC-DOCUMENT>filename.txt : date
    let gt_pos = line.find('>')?;
    let tag_name = &line[1..gt_pos];
    
    let rest = &line[gt_pos + 1..];
    
    // Find " : " separator
    if let Some(colon_pos) = rest.find(" : ") {
        let filename = rest[..colon_pos].trim();
        let date = rest[colon_pos + 3..].trim();
        Some((tag_name.to_string(), format!("{} : {}", filename, date)))
    } else {
        Some((tag_name.to_string(), rest.trim().to_string()))
    }
}

/// Navigate to path and insert value, handling duplicate keys by converting to lists
fn insert_at_path(
    root: &mut HashMap<String, MetadataValue>,
    path: &[String],
    key: String,
    value: MetadataValue,
) {
    if path.is_empty() {
        insert_or_append(root, key, value);
        return;
    }
    
    // Navigate through the path
    let mut current = root as *mut HashMap<String, MetadataValue>;
    
    for path_key in path {
        let current_ref = unsafe { &mut *current };
        
        match current_ref.get_mut(path_key) {
            Some(MetadataValue::Object(obj)) => {
                current = obj as *mut _;
            }
            Some(MetadataValue::List(list)) => {
                // Get the last object in the list
                if let Some(MetadataValue::Object(obj)) = list.last_mut() {
                    current = obj as *mut _;
                } else {
                    return; // Can't navigate further
                }
            }
            _ => return, // Can't navigate further
        }
    }
    
    let target = unsafe { &mut *current };
    insert_or_append(target, key, value);
}

/// Insert value into map, converting to list if key exists
fn insert_or_append(map: &mut HashMap<String, MetadataValue>, key: String, value: MetadataValue) {
    if let Some(existing) = map.get_mut(&key) {
        match existing {
            MetadataValue::List(list) => {
                list.push(value);
            }
            _ => {
                let old = std::mem::replace(existing, MetadataValue::List(Vec::new()));
                if let MetadataValue::List(list) = existing {
                    list.push(old);
                    list.push(value);
                }
            }
        }
    } else {
        map.insert(key, value);
    }
}

/// Clean document content: strip wrapper tags and fix line wraparound
fn clean_document_content(content: &[u8], format: SubmissionFormat, is_binary: bool) -> Vec<u8> {
    let mut content = trim(content);
    
    // Strip opening wrapper tags
    if content.starts_with(b"<PDF>") {
        content = &content[5..];
    } else if content.starts_with(b"<XBRL>") {
        content = &content[6..];
    } else if content.starts_with(b"<XML>") {
        content = &content[5..];
    }
    
    // Strip closing wrapper tags
    let content = trim(content);
    let content = if content.ends_with(b"</PDF>") {
        &content[..content.len() - 6]
    } else if content.ends_with(b"</XBRL>") {
        &content[..content.len() - 7]
    } else if content.ends_with(b"</XML>") {
        &content[..content.len() - 6]
    } else {
        content
    };
    
    // Fix line wraparound for tab-delimited formats (non-binary)
    if !is_binary && matches!(format, SubmissionFormat::TabPrivacy | SubmissionFormat::TabDefault) {
        let lines = fix_line_wraparound(content);
        return lines.join("\n").into_bytes();
    }
    
    trim(content).to_vec()
}

/// Fix tab-delimited content line wraparound (1023 char max per line)
fn fix_line_wraparound(data: &[u8]) -> Vec<String> {
    let text = bytes_to_string(data);
    let lines: Vec<&str> = text.lines().collect();
    let mut result: Vec<String> = Vec::with_capacity(lines.len());
    
    let mut last_was_continuation = false;
    
    for line in lines {
        if !result.is_empty() && last_was_continuation {
            let last_idx = result.len() - 1;
            result[last_idx].push_str(line);
        } else {
            result.push(line.to_string());
        }
        
        last_was_continuation = line.len() >= 1023;
    }
    
    result
}

/// Apply document type filter
fn apply_filter(
    doc_metas: Vec<DocumentMetadata>,
    documents: Vec<Vec<u8>>,
    options: &ParseOptions,
) -> (Vec<DocumentMetadata>, Vec<Vec<u8>>) {
    if options.filter_document_types.is_empty() {
        return (doc_metas, documents);
    }
    
    let type_key = if options.standardize_metadata { "type" } else { "TYPE" };
    
    let indices: Vec<usize> = doc_metas
        .iter()
        .enumerate()
        .filter_map(|(i, meta)| {
            meta.fields.get(type_key).and_then(|t| {
                if options.filter_document_types.contains(t) {
                    Some(i)
                } else {
                    None
                }
            })
        })
        .collect();
    
    if options.keep_filtered_metadata {
        // Keep all metadata, filter only documents
        let filtered_docs: Vec<Vec<u8>> = indices.iter().map(|&i| documents[i].clone()).collect();
        (doc_metas, filtered_docs)
    } else {
        // Filter both
        let filtered_metas: Vec<DocumentMetadata> = indices.iter().map(|&i| doc_metas[i].clone()).collect();
        let filtered_docs: Vec<Vec<u8>> = indices.iter().map(|&i| documents[i].clone()).collect();
        (filtered_metas, filtered_docs)
    }
}

/// Find double newline (blank line separator)
fn find_double_newline(data: &[u8]) -> Option<usize> {
    memmem::find(data, b"\n\n")
}

/// Convert bytes to string, trying UTF-8 then Latin-1
fn bytes_to_string(data: &[u8]) -> String {
    match std::str::from_utf8(data) {
        Ok(s) => s.to_string(),
        Err(_) => {
            // Latin-1 fallback (every byte is valid)
            data.iter().map(|&b| b as char).collect()
        }
    }
}

/// Trim leading whitespace from byte slice
fn trim_start(data: &[u8]) -> &[u8] {
    let start = data.iter().position(|&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r')).unwrap_or(data.len());
    &data[start..]
}

/// Trim trailing whitespace from byte slice
fn trim_end(data: &[u8]) -> &[u8] {
    let end = data.iter().rposition(|&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r')).map(|p| p + 1).unwrap_or(0);
    &data[..end]
}

/// Trim both ends
fn trim(data: &[u8]) -> &[u8] {
    trim_end(trim_start(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_format() {
        assert_eq!(detect_format(b"<SEC-DOCUMENT>"), SubmissionFormat::TabDefault);
        assert_eq!(detect_format(b"-----BEGIN PRIVACY"), SubmissionFormat::TabPrivacy);
        assert_eq!(detect_format(b"<SUBMISSION>"), SubmissionFormat::Archive);
    }

    #[test]
    fn test_parse_tag_line() {
        let (key, value) = parse_tag_line(b"<TYPE>10-K").unwrap();
        assert_eq!(key, b"TYPE");
        assert_eq!(value, b"10-K");
        
        let (key, value) = parse_tag_line(b"<FILENAME>form10k.htm").unwrap();
        assert_eq!(key, b"FILENAME");
        assert_eq!(value, b"form10k.htm");
    }

    #[test]
    fn test_find_document_boundaries() {
        let data = b"header<DOCUMENT>doc1</DOCUMENT>middle<DOCUMENT>doc2</DOCUMENT>end";
        let bounds = find_document_boundaries(data);
        assert_eq!(bounds.len(), 2);
    }

    #[test]
    fn test_fix_line_wraparound() {
        let short_line = "short line";
        let long_line = "x".repeat(1023);
        let continuation = "continued";
        
        let input = format!("{}\n{}\n{}", short_line, long_line, continuation);
        let result = fix_line_wraparound(input.as_bytes());
        
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], short_line);
        assert_eq!(result[1], format!("{}{}", long_line, continuation));
    }

    #[test]
    fn test_clean_document_content() {
        let content = b"  <PDF>actual content</PDF>  ";
        let cleaned = clean_document_content(content, SubmissionFormat::Archive, false);
        assert_eq!(cleaned, b"actual content");
    }
}