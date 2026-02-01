# ai-code-review

AIによる自動コードレビューライブラリ

## なぜ便利か

1. **保存するだけで自動レビュー**: ファイルを保存するたびにAIが自動でコードをレビュー
2. **git diff対応**: 変更差分だけをレビューするため、効率的かつ低コスト
3. **複数バックエンド対応**: Gemini、Claudeなど好みのAIを選択可能
4. **日本語プロンプト**: 日本語で的確なレビューを受けられる
5. **カスタマイズ可能**: セキュリティ重視、アーキテクチャ重視など観点を選択可能

## インストール

```toml
[dependencies]
ai-code-review = { path = "../ai-code-review" }
```

## 使い方

### 基本的な使い方（ファイル監視）

```rust
use ai_code_review::{CodeReviewer, Backend, PromptType};
use std::path::Path;

fn main() -> ai_code_review::Result<()> {
    // プロジェクトディレクトリを監視
    let mut reviewer = CodeReviewer::new(Path::new("./src"))?
        .with_backend(Backend::Gemini)
        .with_extensions(&["rs", "ts", "py"])
        .with_prompt_type(PromptType::Default)
        .on_review(|result| {
            println!("=== {} のレビュー結果 ===", result.name);
            println!("{}", result.review);

            if result.has_issues {
                println!("⚠ 問題が検出されました");
            }
        });

    // 監視開始
    reviewer.start()?;

    // Ctrl+Cなどで停止するまで待機
    std::thread::park();

    reviewer.stop()?;
    Ok(())
}
```

### 単一ファイルのレビュー

```rust
use ai_code_review::{CodeReviewer, Backend};
use std::path::Path;

fn main() -> ai_code_review::Result<()> {
    let reviewer = CodeReviewer::new(Path::new("."))?
        .with_backend(Backend::Claude);

    let result = reviewer.review_file(Path::new("src/main.rs"))?;

    println!("レビュー結果:\n{}", result.review);
    println!("問題あり: {}", result.has_issues);

    Ok(())
}
```

### プロンプトタイプの選択

```rust
use ai_code_review::{CodeReviewer, PromptType};

// クイックレビュー（重大な問題のみ）
let reviewer = CodeReviewer::new(path)?
    .with_prompt_type(PromptType::Quick);

// セキュリティ重視
let reviewer = CodeReviewer::new(path)?
    .with_prompt_type(PromptType::Security);

// アーキテクチャ重視
let reviewer = CodeReviewer::new(path)?
    .with_prompt_type(PromptType::Architecture);
```

### カスタムプロンプト

```rust
let reviewer = CodeReviewer::new(path)?
    .with_prompt(r#"
以下のコードをレビューしてください。
ファイル: {file_name}
```
{content}
```
問題があれば指摘してください。
"#);
```

### ログファイルへの保存

```rust
let reviewer = CodeReviewer::new(path)?
    .with_log_file(Path::new(".code-reviews.log"));
```

## プロンプトタイプ

| タイプ | 説明 |
|--------|------|
| `Default` | 総合的なレビュー（設計、品質、バグ） |
| `Quick` | 重大な問題のみ（高速） |
| `Security` | セキュリティ観点のレビュー |
| `Architecture` | アーキテクチャ・設計観点のレビュー |
| `Custom` | カスタムプロンプト |

## ReviewResult

```rust
pub struct ReviewResult {
    pub path: PathBuf,        // ファイルパス
    pub name: String,         // ファイル名
    pub review: String,       // レビュー内容
    pub timestamp: String,    // タイムスタンプ
    pub has_issues: bool,     // 問題があるか
    pub severity: ReviewSeverity,  // 深刻度
}

pub enum ReviewSeverity {
    Ok,       // 問題なし
    Info,     // 情報・提案
    Warning,  // 警告
    Error,    // 重大な問題
}
```

## 依存クレート

- `folder-watcher` - ファイル監視
- `cli-ai-analyzer` - AI呼び出し

## ライセンス

MIT
