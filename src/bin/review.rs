//! CLI tool for one-shot code review
//!
//! Usage:
//!   review <file>           - Review a single file
//!   review --dir <dir>      - Review all modified files in directory
//!   review --diff           - Review git diff (staged or unstaged)

use ai_code_review::{Backend, CodeReviewer, PromptType};
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: review <file|--dir <dir>|--diff>");
        eprintln!("  <file>       Review a single file");
        eprintln!("  --dir <dir>  Review all source files in directory");
        eprintln!("  --diff       Review git diff (changed files)");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --backend <gemini|claude>  AI backend (default: gemini)");
        eprintln!("  --prompt <default|quick|security|architecture>");
        eprintln!("  --context                  Enable project context (module tree, dependencies)");
        std::process::exit(1);
    }

    // Parse arguments
    let mut backend = Backend::Gemini;
    let mut prompt_type = PromptType::Default;
    let mut mode = Mode::File(PathBuf::new());
    let mut context_enabled = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--backend" => {
                i += 1;
                if i < args.len() {
                    backend = match args[i].to_lowercase().as_str() {
                        "claude" => Backend::Claude,
                        _ => Backend::Gemini,
                    };
                }
            }
            "--prompt" => {
                i += 1;
                if i < args.len() {
                    prompt_type = match args[i].to_lowercase().as_str() {
                        "quick" => PromptType::Quick,
                        "security" => PromptType::Security,
                        "architecture" => PromptType::Architecture,
                        _ => PromptType::Default,
                    };
                }
            }
            "--dir" => {
                i += 1;
                if i < args.len() {
                    mode = Mode::Dir(PathBuf::from(&args[i]));
                }
            }
            "--diff" => {
                mode = Mode::Diff;
            }
            "--context" => {
                context_enabled = true;
            }
            arg if !arg.starts_with('-') => {
                mode = Mode::File(PathBuf::from(arg));
            }
            _ => {}
        }
        i += 1;
    }

    match mode {
        Mode::File(path) => {
            if path.as_os_str().is_empty() {
                eprintln!("Error: No file specified");
                std::process::exit(1);
            }
            review_file(&path, backend, prompt_type, context_enabled);
        }
        Mode::Dir(dir) => {
            review_directory(&dir, backend, prompt_type, context_enabled);
        }
        Mode::Diff => {
            review_diff(backend, prompt_type, context_enabled);
        }
    }
}

enum Mode {
    File(PathBuf),
    Dir(PathBuf),
    Diff,
}

fn review_file(path: &PathBuf, backend: Backend, prompt_type: PromptType, context_enabled: bool) {
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    let reviewer = match CodeReviewer::new(parent) {
        Ok(r) => r.with_backend(backend).with_prompt_type(prompt_type).with_context(context_enabled),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    match reviewer.review_file(path) {
        Ok(result) => {
            println!("## Review: {}\n", result.name);
            println!("{}", result.review);
        }
        Err(e) => {
            eprintln!("Review failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn review_directory(dir: &PathBuf, backend: Backend, prompt_type: PromptType, context_enabled: bool) {
    let reviewer = match CodeReviewer::new(dir) {
        Ok(r) => r.with_backend(backend).with_prompt_type(prompt_type).with_context(context_enabled),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Find source files
    let extensions = ["rs", "ts", "tsx", "js", "jsx", "py"];
    let files = find_files(dir, &extensions);

    if files.is_empty() {
        println!("No source files found in {:?}", dir);
        return;
    }

    for file in files {
        match reviewer.review_file(&file) {
            Ok(result) => {
                println!("## Review: {}\n", result.name);
                println!("{}\n", result.review);
                println!("---\n");
            }
            Err(e) => {
                eprintln!("Review failed for {:?}: {}", file, e);
            }
        }
    }
}

fn review_diff(backend: Backend, prompt_type: PromptType, context_enabled: bool) {
    // Get changed files from git
    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .output();

    let changed_files: Vec<PathBuf> = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .collect(),
        Err(_) => {
            eprintln!("Failed to get git diff");
            std::process::exit(1);
        }
    };

    if changed_files.is_empty() {
        println!("No changed files");
        return;
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let reviewer = match CodeReviewer::new(&cwd) {
        Ok(r) => r.with_backend(backend).with_prompt_type(prompt_type).with_context(context_enabled),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    for file in changed_files {
        if !file.exists() {
            continue;
        }
        match reviewer.review_file(&file) {
            Ok(result) => {
                println!("## Review: {}\n", result.name);
                println!("{}\n", result.review);
                println!("---\n");
            }
            Err(e) => {
                eprintln!("Review failed for {:?}: {}", file, e);
            }
        }
    }
}

fn find_files(dir: &PathBuf, extensions: &[&str]) -> Vec<PathBuf> {
    let mut result = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip hidden directories and common non-source directories
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
                    result.extend(find_files(&path, extensions));
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    result.push(path);
                }
            }
        }
    }

    result
}
