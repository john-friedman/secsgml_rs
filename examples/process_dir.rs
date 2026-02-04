use secsgmlrs::parse_sgml_into_memory;
use std::fs;
use std::path::Path;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let test_dir = "test_data/20040401";

    let start_total = Instant::now();
    let mut parse_time_total = 0.0;
    let mut processed = 0;
    let mut failed = 0;
    let mut total_docs = 0;

    // Read directory
    for entry in fs::read_dir(test_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        // Skip if not a file
        if !path.is_file() {
            continue;
        }
        
        // Check for .nc extension (or process all files)
        let process_file = if let Some(ext) = path.extension() {
            ext == "nc" || ext == "sgml" || ext == "txt"
        } else {
            true // Process files without extension too
        };
        
        if !process_file {
            continue;
        }

        let filename = path.file_stem().unwrap().to_string_lossy();
        
        match process_single_file(&path, &filename, &mut parse_time_total) {
            Ok(num_docs) => {
                processed += 1;
                total_docs += num_docs;
            }
            Err(e) => {
                eprintln!("✗ Error: {}", e);
                failed += 1;
            }
        }
    }

    let elapsed_total = start_total.elapsed();

    println!("Summary:");
    println!("  Files processed: {}", processed);
    println!("  Files failed: {}", failed);
    println!("  Total documents: {}", total_docs);
    println!("  Total time (with I/O): {:.2}s", elapsed_total.as_secs_f64());
    println!("  Parse time (pure): {:.2}s", parse_time_total);
    if processed > 0 {
        println!("  Avg per file: {:.2}ms", (parse_time_total * 1000.0) / processed as f64);
    }

    Ok(())
}

fn process_single_file(
    path: &Path,
    filename: &str,
    parse_time_total: &mut f64,
) -> Result<usize, Box<dyn std::error::Error>> {
    // Read file (NOT timed)
    let data = fs::read(path)?;
    
    // Time ONLY parsing
    let parse_start = Instant::now();
    let (_metadata_json, documents) = parse_sgml_into_memory(
        &data,
        vec![],     // No document type filtering
        false,      // Don't keep filtered metadata
        true,       // Standardize metadata keys
    )?;
    *parse_time_total += parse_start.elapsed().as_secs_f64();
    
    Ok(documents.len())
}