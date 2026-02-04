//! SEC SGML Filing Parser
//!
//! High-performance parser for SEC SGML filings.

mod error;
mod header_mappings;
mod types;
pub mod uudecode;
mod parse;



#[cfg(feature = "python")]
mod python;

pub use error::{ParseError, Result};
pub use types::{
    DocumentMetadata, MetadataValue, ParseOptions, ParsedSubmission, 
    SubmissionFormat, SubmissionMetadata,
};
pub use parse::{parse_sgml, parse_sgml_file};
pub use uudecode::decode_uuencoded;


/// Parse SGML and return JSON metadata bytes + document contents.
/// 
/// This is the primary function for Python integration.
/// Returns (metadata_json_bytes, document_contents).
pub fn parse_sgml_into_memory(
    data: &[u8],
    filter_document_types: Vec<String>,
    keep_filtered_metadata: bool,
    standardize_metadata: bool,
) -> Result<(Vec<u8>, Vec<Vec<u8>>)> {
    let options = ParseOptions {
        filter_document_types,
        keep_filtered_metadata,
        standardize_metadata,
        parallel: true, //reenablew TODO
    };
    
    let result = parse_sgml(data, options)?;
    
    // Serialize metadata to JSON bytes
    let metadata_json = serde_json::to_vec(&result.metadata)?;
    
    Ok((metadata_json, result.documents))
}

