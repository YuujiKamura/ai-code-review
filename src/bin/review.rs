//! CLI tool for one-shot code review
//!
//! Usage:
//!   review <file>           - Review a single file
//!   review --dir <dir>      - Review all modified files in directory
//!   review --diff           - Review git diff (staged or unstaged)
//!   review --hook           - Pre-commit hook mode (review staged diff)
//!   review --hook-install   - Install git pre-commit hook

use ai_code_review::{
    build_analyze_prompt, build_discovery_prompt, build_find_shared_prompt,
    gather_raw_context, generate_module_tree,
    shared_finder::find_shared_candidates,
    Backend, CodeReviewer, PromptType, ANALYZE_PROMPT, DISCOVERY_PROMPT, FIND_SHARED_PROMPT,
};
use cli_ai_analyzer::{prompt as ai_prompt, AnalyzeOptions};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: review <file|--dir <dir>|--diff|--discover|--analyze|--hook>");
        eprintln!("  <file>         Review a single file");
        eprintln!("  --dir <dir>    Review all source files in directory");
        eprintln!("  --diff         Review git diff (changed files)");
        eprintln!("  --discover     Discovery mode (requires --goal)");
        eprintln!("  --analyze <f>  Analyze file with AI (no AST parsing, AI does the work)");
        eprintln!("  --hook         Pre-commit hook mode (review staged diff)");
        eprintln!("  --hook-install Install git pre-commit hook");
        eprintln!("  --find-shared <dirA> <dirB>  Find shared/duplicated code between two projects");
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
            "--hook" => {
                mode = Mode::Hook;
            }
            "--hook-install" => {
                mode = Mode::HookInstall;
            }
            "--discover" => {
                mode = Mode::Discover(String::new()); // placeholder, goal filled later
            }
            "--analyze" => {
                i += 1;
                if i < args.len() {
                    mode = Mode::Analyze(PathBuf::from(&args[i]));
                    prompt_type = PromptType::Analyze;
                }
            }
            "--find-shared" => {
                i += 1;
                if i + 1 < args.len() {
                    mode = Mode::FindShared(PathBuf::from(&args[i]), PathBuf::from(&args[i + 1]));
                    i += 1;
                } else {
                    eprintln!("Error: --find-shared requires two directory paths");
                    std::process::exit(1);
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
    if let Mode::Discover(_) = &mode {
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
        Mode::FindShared(path_a, path_b) => {
            find_shared_modules(&path_a, &path_b, backend);
        }
        Mode::Hook => {
            run_hook(backend);
        }
        Mode::HookInstall => {
            install_hook();
        }
    }
}

enum Mode {
    File(PathBuf),
    Dir(PathBuf),
    Diff,
    Discover(String), // goal
    Analyze(PathBuf), // file to analyze with AI
    Hook,                        // Pre-commit hook mode
    HookInstall,                 // Install git pre-commit hook
    FindShared(PathBuf, PathBuf), // (path_a, path_b)
}

fn review_file(path: &Path, backend: Backend, prompt_type: PromptType, context_enabled: bool) {
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

fn review_directory(dir: &Path, backend: Backend, prompt_type: PromptType, context_enabled: bool) {
    let reviewer = match CodeReviewer::new(dir) {
        Ok(r) => r.with_backend(backend).with_prompt_type(prompt_type).with_context(context_enabled),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Find source files
    let extensions = ["rs", "ts", "tsx", "js", "jsx", "py"];
    let files = find_modified_files(dir, &extensions);

    if files.is_empty() {
        println!("No modified source files found in {:?}", dir);
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
    let output = {
        let mut cmd = Command::new("git");
        cmd.args(["diff", "--name-only", "HEAD"]);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        cmd.output()
    };

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

fn find_modified_files(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    // Try git status first
    let mut cmd = Command::new("git");
    cmd.args(["status", "--porcelain", "--untracked-files=no"]);
    cmd.current_dir(dir);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    if let Ok(output) = cmd.output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let files: Vec<PathBuf> = stdout
                .lines()
                .filter_map(|line| {
                    let line = line.trim();
                    if line.len() > 3 {
                        let raw_path = line[3..].trim();
                        // Handle renames: "R  old -> new" -> use "new"
                        let file = if raw_path.contains(" -> ") {
                            raw_path.rsplit(" -> ").next().unwrap_or(raw_path)
                        } else {
                            raw_path
                        };
                        let path = dir.join(file);
                        if path.exists() {
                            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                if extensions.contains(&ext) {
                                    return Some(path);
                                }
                            }
                        }
                    }
                    None
                })
                .collect();

            if !files.is_empty() {
                return files;
            }
        }
    }

    // Fallback to all files if not in git repo
    find_files(dir, extensions)
}

fn find_files(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
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

fn analyze_with_ai(file_path: &Path, backend: Backend) {
    if !file_path.exists() {
        eprintln!("Error: File not found: {:?}", file_path);
        std::process::exit(1);
    }

    let base_path = file_path.parent().unwrap_or(Path::new("."));

    // Gather raw context (no AST parsing, just file contents)
    let raw_ctx = gather_raw_context(file_path, base_path, 3, 50);

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


fn find_shared_modules(path_a: &Path, path_b: &Path, backend: Backend) {
    if !path_a.exists() {
        eprintln!("Error: Directory not found: {:?}", path_a);
        std::process::exit(1);
    }
    if !path_b.exists() {
        eprintln!("Error: Directory not found: {:?}", path_b);
        std::process::exit(1);
    }

    eprintln!("=== Shared Code Discovery ===");
    eprintln!("Project A: {}", path_a.display());
    eprintln!("Project B: {}", path_b.display());
    eprintln!();

    // Phase 1: Static analysis
    let report = find_shared_candidates(path_a, path_b);

    eprintln!(
        "Scanned: {} files (A) + {} files (B)",
        report.files_scanned_a, report.files_scanned_b
    );
    eprintln!("Found {} candidates\n", report.candidates.len());

    if report.candidates.is_empty() {
        println!("共有候補は見つかりませんでした。");
        return;
    }

    // Print static analysis results
    let report_text = report.to_prompt_string();
    println!("{}", report_text);

    // Phase 2: AI analysis
    eprintln!("--- AI Analysis ---\n");
    let prompt = build_find_shared_prompt(FIND_SHARED_PROMPT, &report_text);
    let options = AnalyzeOptions::default().with_backend(backend);

    match ai_prompt(&prompt, options) {
        Ok(response) => {
            println!("{}", response);
        }
        Err(e) => {
            eprintln!("AI analysis failed: {}", e);
            eprintln!("(Static analysis results are shown above)");
        }
    }
}

fn run_hook(backend: Backend) {
    let _cwd = std::env::current_dir().unwrap_or_default();

    // Get staged diff
    let diff = {
        let mut cmd = Command::new("git");
        cmd.args(["diff", "--cached"]);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        match cmd.output() {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => {
                return;
            }
        }
    };

    if diff.trim().is_empty() {
        return;
    }

    let diff_lines: Vec<&str> = diff.lines().collect();
    if diff_lines.len() > 500 {
        eprintln!(
            "⚠ Diff too large ({} lines), skipping review",
            diff_lines.len()
        );
        return;
    }

    eprintln!("=== AI Code Review (Hook) ===");
    eprintln!("Reviewing {} lines...\n", diff_lines.len());

    // Use concise review prompt for hook mode
    let prompt = format!(
        "Code review of staged changes. If critical issues found, start line with ⚠. If OK, respond ✓ LGTM. Be concise.\n\nFocus: design flaws, bugs, security issues.\n\n```diff\n{}\n```",
        diff
    );

    let options = AnalyzeOptions::default().with_backend(backend);
    match ai_prompt(&prompt, options) {
        Ok(review) => {
            eprintln!("{}\n", review);
            eprintln!("=== Review Complete ===\n");
            if review.contains("⚠") {
                eprintln!("[BLOCKED] Fix issues above before committing.");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Review error: {}", e);
            // Don't block on errors
        }
    }
}

fn install_hook() {
    let cwd = std::env::current_dir().unwrap_or_default();
    let hook_dir = cwd.join(".git").join("hooks");
    if !hook_dir.exists() {
        eprintln!("Error: Not a git repository (no .git/hooks)");
        std::process::exit(1);
    }
    let hook_path = hook_dir.join("pre-commit");
    let review_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("review"));
    let script = if cfg!(target_os = "windows") {
        format!("#!/bin/sh\n\"{}\" --hook\n", review_path.display())
    } else {
        format!("#!/bin/sh\n\"{}\" --hook\n", review_path.display())
    };
    std::fs::write(&hook_path, &script).expect("Failed to write hook");
    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755)).ok();
    }
    println!("✓ Pre-commit hook installed at {}", hook_path.display());
}
