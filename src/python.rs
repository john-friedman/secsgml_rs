//! Python bindings for secsgml using PyO3

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::exceptions::PyValueError;

/// Parse SGML content and return (metadata_json_bytes, documents)
/// 
/// Args:
///     data: Raw SGML bytes
///     filter_document_types: List of document types to keep (empty = all)
///     keep_filtered_metadata: Keep metadata for filtered-out documents
///     standardize_metadata: Standardize keys to lowercase kebab-case
/// 
/// Returns:
///     Tuple of (metadata_json_bytes, list_of_document_bytes)
#[pyfunction]
#[pyo3(signature = (data, filter_document_types=vec![], keep_filtered_metadata=false, standardize_metadata=true))]
fn parse_sgml_to_json(
    py: Python<'_>,
    data: &[u8],
    filter_document_types: Vec<String>,
    keep_filtered_metadata: bool,
    standardize_metadata: bool,
) -> PyResult<(PyObject, PyObject)> {
    let (metadata_json, documents) = crate::parse_sgml_to_json(
        data,
        filter_document_types,
        keep_filtered_metadata,
        standardize_metadata,
    ).map_err(|e| PyValueError::new_err(format!("Parse error: {}", e)))?;
    
    // Convert metadata JSON to Python bytes
    let py_metadata = PyBytes::new_bound(py, &metadata_json).into();
    
    // Convert documents to Python list of bytes
    let py_documents: Vec<PyObject> = documents
        .iter()
        .map(|doc| PyBytes::new_bound(py, doc).into())
        .collect();
    
    Ok((py_metadata, py_documents.into_py(py)))
}

/// Python module definition
#[pymodule]
fn secsgml_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_sgml_to_json, m)?)?;
    Ok(())
}