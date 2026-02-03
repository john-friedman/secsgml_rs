//! CLI tool for parsing SEC SGML files

use secsgml::{parse_sgml_file, write_to_tar, ParseOptions};
use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <sgml_file> [--output <tar_file>] [--no-standardize] [--no-parallel]", args[0]);
        eprintln!("\nParses an SEC SGML filing and outputs metadata as JSON.");
        eprintln!("Use --output to write a TAR archive.");
        std::process::exit(1);
    }
    
    let path = PathBuf::from(&args[1]);
    
    if !path.exists() {
        eprintln!("Error: File not found: {}", path.display());
        std::process::exit(1);
    }
    
    let mut options = ParseOptions::new();
    let mut output_path: Option<PathBuf> = None;
    
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--no-standardize" => options.standardize_metadata = false,
            "--no-parallel" => options.parallel = false,
            "--output" | "-o" => {
                i += 1;
                if i < args.len() {
                    output_path = Some(PathBuf::from(&args[i]));
                } else {
                    eprintln!("Error: --output requires a path");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }
    
    match parse_sgml_file(&path, options) {
        Ok(result) => {
            println!("Format: {:?}", result.format);
            println!("Documents: {}", result.documents.len());
            
            // Print document info
            for (i, doc) in result.metadata.documents.iter().enumerate() {
                println!("  [{}] {:?} - {:?} ({} bytes)", 
                    i + 1,
                    doc.doc_type().unwrap_or("?"),
                    doc.filename().unwrap_or("unnamed"),
                    doc.size_bytes
                );
            }
            
            // Write TAR if output specified
            if let Some(out) = output_path {
                match write_to_tar(&result, &out) {
                    Ok(_) => println!("\nWrote TAR to: {}", out.display()),
                    Err(e) => {
                        eprintln!("Error writing TAR: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Print metadata as JSON
                match serde_json::to_string_pretty(&result.metadata) {
                    Ok(json) => println!("\nMetadata:\n{}", json),
                    Err(e) => eprintln!("Error serializing metadata: {}", e),
                }
            }
        }
        Err(e) => {
            eprintln!("Error parsing file: {}", e);
            std::process::exit(1);
        }
    }
}