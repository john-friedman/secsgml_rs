//! Type definitions for parsed SGML data

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Submission format detected from file content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubmissionFormat {
    TabPrivacy,
    TabDefault,
    Archive,
}

/// A metadata value: string, list, or nested object
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetadataValue {
    String(String),
    List(Vec<MetadataValue>),
    Object(HashMap<String, MetadataValue>),
}

impl MetadataValue {
    pub fn string(s: impl Into<String>) -> Self {
        MetadataValue::String(s.into())
    }

    pub fn object() -> Self {
        MetadataValue::Object(HashMap::new())
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            MetadataValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&HashMap<String, MetadataValue>> {
        match self {
            MetadataValue::Object(obj) => Some(obj),
            _ => None,
        }
    }

    pub fn as_object_mut(&mut self) -> Option<&mut HashMap<String, MetadataValue>> {
        match self {
            MetadataValue::Object(obj) => Some(obj),
            _ => None,
        }
    }
}

impl Default for MetadataValue {
    fn default() -> Self {
        MetadataValue::Object(HashMap::new())
    }
}

/// Metadata for a single document within the submission
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentMetadata {
    #[serde(flatten)]
    pub fields: HashMap<String, String>,

    #[serde(rename = "secsgml_size_bytes")]
    pub size_bytes: usize,

    #[serde(rename = "secsgml_start_byte", skip_serializing_if = "Option::is_none")]
    pub start_byte: Option<String>,

    #[serde(rename = "secsgml_end_byte", skip_serializing_if = "Option::is_none")]
    pub end_byte: Option<String>,
}

impl DocumentMetadata {
    pub fn doc_type(&self) -> Option<&str> {
        self.fields.get("type").map(|s| s.as_str())
    }

    pub fn filename(&self) -> Option<&str> {
        self.fields.get("filename").map(|s| s.as_str())
    }

    pub fn sequence(&self) -> Option<&str> {
        self.fields.get("sequence").map(|s| s.as_str())
    }
}

/// Complete submission metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubmissionMetadata {
    #[serde(flatten)]
    pub fields: HashMap<String, MetadataValue>,

    pub documents: Vec<DocumentMetadata>,
}

/// Options for parsing
#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    /// Filter to specific document types (empty = all)
    pub filter_document_types: Vec<String>,
    /// Keep metadata for filtered-out documents
    pub keep_filtered_metadata: bool,
    /// Standardize keys to lowercase kebab-case
    pub standardize_metadata: bool,
}

impl ParseOptions {
    pub fn new() -> Self {
        Self {
            standardize_metadata: true,
            ..Default::default()
        }
    }

    pub fn preserve_original() -> Self {
        Self {
            standardize_metadata: false,
            ..Default::default()
        }
    }

    pub fn with_filter(mut self, types: Vec<String>) -> Self {
        self.filter_document_types = types;
        self
    }
}
/// Result of parsing an SGML submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedSubmission {
    pub metadata: SubmissionMetadata,
    #[serde(skip)]
    pub documents: Vec<Vec<u8>>,
    pub format: SubmissionFormat,
}