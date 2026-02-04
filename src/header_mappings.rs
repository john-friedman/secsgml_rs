//! Header field name mappings for standardization

use std::collections::HashMap;
use std::sync::OnceLock;
use std::borrow::Cow;

/// Mapping entry with optional regex pattern for value extraction
pub struct HeaderMapping {
    pub to: &'static str,
    /// Regex pattern - group 1 is extracted as the value
    pub regex: Option<&'static str>,
}

impl HeaderMapping {
    const fn simple(to: &'static str) -> Self {
        Self { to, regex: None }
    }

    const fn with_regex(to: &'static str, regex: &'static str) -> Self {
        Self { to, regex: Some(regex) }
    }
}

static HEADER_MAPPINGS: OnceLock<HashMap<&'static str, HeaderMapping>> = OnceLock::new();

pub fn get_header_mappings() -> &'static HashMap<&'static str, HeaderMapping> {
    HEADER_MAPPINGS.get_or_init(|| {
        let mut m = HashMap::with_capacity(64);

        m.insert("paper", HeaderMapping::simple("paper"));
        m.insert("accession number", HeaderMapping::simple("accession-number"));
        m.insert("conformed submission type", HeaderMapping::simple("type"));
        m.insert("public document count", HeaderMapping::simple("public-document-count"));
        m.insert("public-document_count", HeaderMapping::simple("public-document-count"));
        m.insert("conformed period of report", HeaderMapping::simple("period"));
        m.insert("filed as of date", HeaderMapping::simple("filing-date"));
        m.insert("date as of change", HeaderMapping::simple("date-of-filing-date-change"));
        m.insert("effectiveness date", HeaderMapping::simple("effectiveness-date"));
        m.insert("filer", HeaderMapping::simple("filer"));
        m.insert("company data", HeaderMapping::simple("company-data"));
        m.insert("company conformed name", HeaderMapping::simple("conformed-name"));
        m.insert("central index key", HeaderMapping::simple("cik"));
        m.insert("state of incorporation", HeaderMapping::simple("state-of-incorporation"));
        m.insert("fiscal year end", HeaderMapping::simple("fiscal-year-end"));
        m.insert("filing values", HeaderMapping::simple("filing-values"));
        m.insert("form type", HeaderMapping::simple("form-type"));
        m.insert("sec act", HeaderMapping::with_regex("act", r"(?:\d{2})(\d{2})\s+Act"));
        m.insert("sec file number", HeaderMapping::simple("file-number"));
        m.insert("film number", HeaderMapping::simple("film-number"));
        m.insert("business address", HeaderMapping::simple("business-address"));
        m.insert("street 1", HeaderMapping::simple("street1"));
        m.insert("street 2", HeaderMapping::simple("street2"));
        m.insert("city", HeaderMapping::simple("city"));
        m.insert("state", HeaderMapping::simple("state"));
        m.insert("zip", HeaderMapping::simple("zip"));
        m.insert("business phone", HeaderMapping::simple("phone"));
        m.insert("mail address", HeaderMapping::simple("mail-address"));
        m.insert("former company", HeaderMapping::simple("former-company"));
        m.insert("former conformed name", HeaderMapping::simple("former-conformed-name"));
        m.insert("date of name change", HeaderMapping::simple("date-changed"));
        m.insert("sros", HeaderMapping::simple("sros"));
        m.insert("subject company", HeaderMapping::simple("subject-company"));
        m.insert("standard industrial classification", HeaderMapping::with_regex("assigned-sic", r"\[(\d+)\]"));
        m.insert("irs number", HeaderMapping::simple("irs-number"));
        m.insert("filed by", HeaderMapping::simple("filed-by"));
        m.insert("items", HeaderMapping::simple("items"));
        m.insert("group members", HeaderMapping::simple("group-members"));
        m.insert("organization name", HeaderMapping::simple("organization-name"));
        m.insert("recieved date", HeaderMapping::simple("recieved-date"));
        m.insert("action date", HeaderMapping::simple("action-date"));
        m.insert("non us state territory", HeaderMapping::simple("non-us-state-territory"));
        m.insert("address is a non us location", HeaderMapping::simple("address-is-a-non-us-location"));
        m.insert("ein", HeaderMapping::simple("ein"));
        m.insert("class-contract-ticker-symbol", HeaderMapping::simple("class-contract-ticker-symbol"));
        m.insert("class-contract-name", HeaderMapping::simple("class-contract-name"));
        m.insert("class-contract-id", HeaderMapping::simple("class-contract-id"));
        m.insert("sec-document", HeaderMapping::simple("sec-document"));
        m.insert("sec-header", HeaderMapping::simple("sec-header"));
        m.insert("acceptance-datetime", HeaderMapping::simple("acceptance-datetime"));
        m.insert("series-and-classes-contracts-data", HeaderMapping::simple("series-and-classes-contracts-data"));
        m.insert("existing-series-and-classes-contracts", HeaderMapping::simple("existing-series-and-classes-contracts"));
        m.insert("merger-series-and-classes-contracts", HeaderMapping::simple("merger-series-and-classes-contracts"));
        m.insert("new-series-and-classes-contracts", HeaderMapping::simple("new-series-and-classes-contracts"));
        m.insert("series", HeaderMapping::simple("series"));
        m.insert("owner-cik", HeaderMapping::simple("owner-cik"));
        m.insert("series-id", HeaderMapping::simple("series-id"));
        m.insert("series-name", HeaderMapping::simple("series-name"));
        m.insert("acquiring-data", HeaderMapping::simple("acquiring-data"));
        m.insert("target-data", HeaderMapping::simple("target-data"));
        m.insert("new-classes-contracts", HeaderMapping::simple("new-classes-contracts"));
        m.insert("new-series", HeaderMapping::simple("new-series"));
        m.insert("relationship", HeaderMapping::simple("relationship"));

        m
    })
}

/// Standardize a key: lookup in mapping or convert to lowercase kebab-case
pub fn standardize_key(key: &str) -> Cow<'static, str> {
    // Fast path: try direct lookup with ASCII lowercase comparison
    let mappings = get_header_mappings();
    
    // Check if key matches any known mapping (case-insensitive)
    for (known_key, mapping) in mappings.iter() {
        if key.eq_ignore_ascii_case(known_key) {
            return Cow::Borrowed(mapping.to);  // Zero allocation!
        }
    }
    
    // Unknown key - do full transformation
    let mut result = String::with_capacity(key.len());
    let mut prev_was_space = false;
    
    for c in key.chars() {
        if c.is_whitespace() {
            if !prev_was_space && !result.is_empty() {
                result.push('-');
            }
            prev_was_space = true;
        } else {
            result.push(c.to_ascii_lowercase());
            prev_was_space = false;
        }
    }
    
    Cow::Owned(result)
}
/// Apply regex transformation if the key has one defined
pub fn transform_value(key: &str, value: &str) -> String {
    let key_lower = key.to_lowercase();
    
    if let Some(mapping) = get_header_mappings().get(key_lower.as_str()) {
        if let Some(pattern) = mapping.regex {
            // Simple manual extraction for known patterns to avoid regex dependency
            // Pattern 1: "(?:\d{2})(\d{2})\s+Act" - extract 2 digits before " Act"
            if pattern.contains("Act") {
                if let Some(act_pos) = value.find(" Act") {
                    if act_pos >= 2 {
                        let potential = &value[act_pos - 2..act_pos];
                        if potential.chars().all(|c| c.is_ascii_digit()) {
                            return potential.to_string();
                        }
                    }
                }
            }
            // Pattern 2: "\[(\d+)\]" - extract digits inside brackets
            if pattern.contains(r"\[") {
                if let Some(start) = value.find('[') {
                    if let Some(end) = value.find(']') {
                        if end > start + 1 {
                            let inner = &value[start + 1..end];
                            if inner.chars().all(|c| c.is_ascii_digit()) {
                                return inner.to_string();
                            }
                        }
                    }
                }
            }
        }
    }
    
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standardize_key() {
        assert_eq!(standardize_key("CENTRAL INDEX KEY"), "cik");
        assert_eq!(standardize_key("central index key"), "cik");
        assert_eq!(standardize_key("COMPANY CONFORMED NAME"), "conformed-name");
        assert_eq!(standardize_key("UNKNOWN FIELD"), "unknown-field");
        assert_eq!(standardize_key("some  multiple   spaces"), "some-multiple-spaces");
    }

    #[test]
    fn test_transform_value_sic() {
        let result = transform_value("STANDARD INDUSTRIAL CLASSIFICATION", "SERVICES [7370]");
        assert_eq!(result, "7370");
    }

    #[test]
    fn test_transform_value_sec_act() {
        let result = transform_value("SEC ACT", "1934 Act");
        assert_eq!(result, "34");
    }

    #[test]
    fn test_transform_value_no_regex() {
        let result = transform_value("COMPANY CONFORMED NAME", "ACME CORP");
        assert_eq!(result, "ACME CORP");
    }
}