//! シンプルなコードレビューのテスト
use std::path::Path;
use std::env;
use ai_code_review::CodeReviewer;
use cli_ai_analyzer::Backend;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let file_path = args.get(1).map(|s| s.as_str()).unwrap_or("src/lib.rs");
    let test_file = Path::new(file_path);

    let reviewer = CodeReviewer::new(Path::new("."))?
        .with_backend(Backend::Gemini);

    println!("レビュー中: {:?}", test_file);
    let result = reviewer.review_file(test_file)?;

    println!("\n=== {} ===", result.path.display());
    println!("重要度: {:?}\n", result.severity);
    println!("{}", result.review);

    Ok(())
}
