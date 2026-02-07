//! Tests for prompt building

use ai_code_review::{
    build_prompt, PromptType, ARCHITECTURE_REVIEW_PROMPT, DEFAULT_REVIEW_PROMPT,
    QUICK_REVIEW_PROMPT, SECURITY_REVIEW_PROMPT,
};

#[test]
fn test_build_prompt_replaces_placeholders() {
    let prompt = build_prompt(
        "ファイル: {file_name}\nコード:\n{content}",
        "main.rs",
        "fn main() { println!(\"Hello\"); }",
    );

    assert!(prompt.contains("main.rs"));
    assert!(prompt.contains("fn main()"));
    assert!(!prompt.contains("{file_name}"));
    assert!(!prompt.contains("{content}"));
}

#[test]
fn test_default_prompt_structure() {
    let prompt = build_prompt(DEFAULT_REVIEW_PROMPT, "test.rs", "let x = 1;");

    assert!(prompt.contains("test.rs"));
    assert!(prompt.contains("let x = 1;"));
    assert!(prompt.contains("設計・結合の均衡"));
    assert!(prompt.contains("コード品質"));
    assert!(prompt.contains("バグ・セキュリティ"));
}

#[test]
fn test_quick_prompt_structure() {
    let prompt = build_prompt(QUICK_REVIEW_PROMPT, "lib.rs", "pub fn foo() {}");

    assert!(prompt.contains("lib.rs"));
    assert!(prompt.contains("pub fn foo()"));
    assert!(prompt.contains("2行以内"));
}

#[test]
fn test_security_prompt_structure() {
    let prompt = build_prompt(SECURITY_REVIEW_PROMPT, "auth.rs", "fn login() {}");

    assert!(prompt.contains("auth.rs"));
    assert!(prompt.contains("インジェクション"));
    assert!(prompt.contains("認証・認可"));
    assert!(prompt.contains("機密情報"));
}

#[test]
fn test_architecture_prompt_structure() {
    let prompt = build_prompt(ARCHITECTURE_REVIEW_PROMPT, "service.rs", "struct Service {}");

    assert!(prompt.contains("service.rs"));
    assert!(prompt.contains("単一責任の原則"));
    assert!(prompt.contains("依存関係"));
    assert!(prompt.contains("結合の均衡"));
    assert!(prompt.contains("高凝集"));
    assert!(prompt.contains("変動性"));
}

#[test]
fn test_prompt_type_default() {
    assert_eq!(PromptType::default(), PromptType::Default);
}

#[test]
fn test_prompt_type_templates() {
    assert_eq!(PromptType::Default.template(), DEFAULT_REVIEW_PROMPT);
    assert_eq!(PromptType::Quick.template(), QUICK_REVIEW_PROMPT);
    assert_eq!(PromptType::Security.template(), SECURITY_REVIEW_PROMPT);
    assert_eq!(PromptType::Architecture.template(), ARCHITECTURE_REVIEW_PROMPT);
    assert_eq!(PromptType::Custom.template(), "");
}

#[test]
fn test_prompt_with_multiline_content() {
    let content = r#"
fn main() {
    let x = 1;
    let y = 2;
    println!("{}", x + y);
}
"#;
    let prompt = build_prompt(QUICK_REVIEW_PROMPT, "main.rs", content);

    assert!(prompt.contains("fn main()"));
    assert!(prompt.contains("println!"));
}

#[test]
fn test_prompt_with_special_characters() {
    let content = r#"let s = "Hello, 世界!";"#;
    let prompt = build_prompt(QUICK_REVIEW_PROMPT, "日本語ファイル.rs", content);

    assert!(prompt.contains("日本語ファイル.rs"));
    assert!(prompt.contains("世界"));
}
