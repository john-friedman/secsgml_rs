//! Integration tests - parses files from test_data/ and writes to TAR

use secsgml::{parse_sgml_file, write_to_tar, ParseOptions};
use std::path::Path;

fn get_test_files() -> Vec<std::path::PathBuf> {
    let test_dirs = ["test_data", "../test_data", "tests/test_data"];
    
    for dir in &test_dirs {
        let path = Path::new(dir);
        if path.exists() {
            let mut files = Vec::new();
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(ext) = path.extension() {
                        if ext == "txt" || ext == "sgml" {
                            files.push(path);
                        }
                    }
                }
            }
            if !files.is_empty() {
                return files;
            }
        }
    }
    
    Vec::new()
}

#[test]
fn test_parse_and_write_all_files() {
    let files = get_test_files();
    
    if files.is_empty() {
        eprintln!("No test files found in test_data/");
        return;
    }
    
    // Create output directory
    let output_dir = Path::new("test_output");
    std::fs::create_dir_all(output_dir).expect("Failed to create output directory");
    
    for file in &files {
        println!("Processing: {}", file.display());
        
        // Parse
        let result = parse_sgml_file(file, ParseOptions::new())
            .expect(&format!("Failed to parse {}", file.display()));
        
        println!("  Format: {:?}", result.format);
        println!("  Documents: {}", result.documents.len());
        
        for (i, doc) in result.metadata.documents.iter().enumerate() {
            println!("    [{}] type={:?} filename={:?} size={}", 
                i, doc.doc_type(), doc.filename(), doc.size_bytes);
        }
        
        // Write to TAR
        let stem = file.file_stem().unwrap().to_string_lossy();
        let tar_path = output_dir.join(format!("{}.tar", stem));
        
        write_to_tar(&result, &tar_path)
            .expect(&format!("Failed to write TAR for {}", file.display()));
        
        println!("  Wrote: {}", tar_path.display());
        
        // Verify TAR exists and has content
        let tar_size = std::fs::metadata(&tar_path)
            .expect("TAR file not created")
            .len();
        
        assert!(tar_size > 0, "TAR file is empty");
        println!("  TAR size: {} bytes", tar_size);
    }
}