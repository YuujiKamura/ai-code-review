//! CLI tool for one-shot code review
//!
//! Usage:
//!   review <file>           - Review a single file
//!   review --dir <dir>      - Review all modified files in directory
//!   review --diff           - Review git diff (staged or unstaged)

use ai_code_review::{
    build_analyze_prompt, build_discovery_prompt, gather_raw_context, generate_module_tree,
    Backend, CodeReviewer, PromptType, ANALYZE_PROMPT, DISCOVERY_PROMPT,
};
use cli_ai_analyzer::{prompt as ai_prompt, AnalyzeOptions};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: review <file|--dir <dir>|--diff|--discover|--analyze>");
        eprintln!("  <file>         Review a single file");
        eprintln!("  --dir <dir>    Review all source files in directory");
        eprintln!("  --diff         Review git diff (changed files)");
        eprintln!("  --discover     Discovery mode (requires --goal)");
        eprintln!("  --analyze <f>  Analyze file with AI (no AST parsing, AI does the work)");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --backend <gemini|claude>  AI backend (default: gemini)");
        eprintln!("  --prompt <default|quick|security|architecture|holistic|discovery|analyze>");
        eprintln!("  --context                  Enable project context (module tree, dependencies)");
        eprintln!("  --goal <text>              Project goal for discovery mode");
        std::process::exit(1);
    }

    // Parse arguments
    let mut backend = Backend::Gemini;
    let mut prompt_type = PromptType::Default;
    let mut mode = Mode::File(PathBuf::new());
    let mut context_enabled = false;
    let mut goal: Option<String> = None;

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
                        "holistic" => PromptType::Holistic,
                        "discovery" => PromptType::Discovery,
                        "analyze" => PromptType::Analyze,
                        _ => PromptType::Default,
                    };
                    // holisticは自動でcontext有効
                    if prompt_type == PromptType::Holistic {
                        context_enabled = true;
                    }
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
            "--discover" => {
                // Will be set after parsing --goal
            }
            "--analyze" => {
                i += 1;
                if i < args.len() {
                    mode = Mode::Analyze(PathBuf::from(&args[i]));
                    prompt_type = PromptType::Analyze;
                }
            }
            "--goal" => {
                i += 1;
                if i < args.len() {
                    goal = Some(args[i].clone());
                }
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

    // Handle --discover mode
    if args.iter().any(|a| a == "--discover") {
        match goal {
            Some(g) => {
                mode = Mode::Discover(g);
                prompt_type = PromptType::Discovery;
            }
            None => {
                eprintln!("Error: --discover requires --goal <text>");
                std::process::exit(1);
            }
        }
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
        Mode::Discover(goal) => {
            discover_architecture(&goal, backend);
        }
        Mode::Analyze(path) => {
            analyze_with_ai(&path, backend);
        }
    }
}

enum Mode {
    File(PathBuf),
    Dir(PathBuf),
    Diff,
    Discover(String), // goal
    Analyze(PathBuf), // file to analyze with AI
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
            print_default_next_steps();
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
                print_default_next_steps();
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
                print_default_next_steps();
                println!("---\n");
            }
            Err(e) => {
                eprintln!("Review failed for {:?}: {}", file, e);
            }
        }
    }
}

fn print_default_next_steps() {
    println!();
    println!("Next Action (Default):");
    println!("- サブエージェント並列実装で対応してください");
    println!("- 重要/重大の指摘は先に対応");
    println!("- 変更後に再レビュー");
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

fn analyze_with_ai(file_path: &PathBuf, backend: Backend) {
    if !file_path.exists() {
        eprintln!("Error: File not found: {:?}", file_path);
        std::process::exit(1);
    }

    let base_path = file_path.parent().unwrap_or(Path::new("."));

    // Gather raw context (no AST parsing, just file contents)
    let raw_ctx = gather_raw_context(file_path, base_path, 3);

    // Read the target file
    let file_content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };

    // Build context string
    let file_name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let mut context = format!("## 対象ファイル: {}\n```\n{}\n```\n\n", file_name, file_content);
    context.push_str(&raw_ctx.to_prompt_string());

    // Build prompt and call AI
    let prompt = build_analyze_prompt(ANALYZE_PROMPT, &context);
    let options = AnalyzeOptions::default().with_backend(backend);

    println!("## Analyze: {}\n", file_name);

    match ai_prompt(&prompt, options) {
        Ok(response) => {
            println!("{}", response);
        }
        Err(e) => {
            eprintln!("Analysis failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn discover_architecture(goal: &str, backend: Backend) {
    let cwd = std::env::current_dir().unwrap_or_default();

    // Find src directory or use current directory
    let src_dir = if cwd.join("src").exists() {
        cwd.join("src")
    } else {
        cwd.clone()
    };

    // Generate project structure
    let structure = generate_module_tree(&src_dir, Path::new(""));

    // Also include root-level important files
    let mut full_structure = String::new();

    // Check for common project files
    let root_files = ["Cargo.toml", "package.json", "pyproject.toml", "README.md"];
    let existing_root: Vec<&str> = root_files
        .iter()
        .copied()
        .filter(|f| cwd.join(f).exists())
        .collect();

    if !existing_root.is_empty() {
        full_structure.push_str("Root files: ");
        full_structure.push_str(&existing_root.join(", "));
        full_structure.push_str("\n\n");
    }

    full_structure.push_str(&structure);

    // Build prompt
    let prompt = build_discovery_prompt(DISCOVERY_PROMPT, goal, &full_structure);

    // Call AI
    let options = AnalyzeOptions::default().with_backend(backend);

    println!("## Discovery: {}\n", goal);
    println!("現在の構造:\n```\n{}\n```\n", full_structure);
    println!("---\n");

    match ai_prompt(&prompt, options) {
        Ok(response) => {
            println!("{}", response);
        }
        Err(e) => {
            eprintln!("Discovery failed: {}", e);
            std::process::exit(1);
        }
    }
}
