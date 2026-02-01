# ai-code-review

実装中の設計ドリフトを検出するためのRustライブラリ

## 解決する問題

AIエージェントが実装に没頭していると、視野が狭くなる:

```
要件全体
  └── 機能A
        └── モジュールA1 ← 今ここ作ってる（視野）
        └── モジュールA2   ← 見えてない
  └── 機能B               ← 完全に忘れてる
```

- A1を作ってる間、A2との整合性を考えなくなる
- モジュール間の責務分離が曖昧になる
- 当初の設計から徐々に逸れていく

**diffレビューでは解決しない**。差分だけ見ても全体の設計は見えない。

## このライブラリの役割

**メインエージェント**（Claude等）が実装に集中している間、**セカンダリAI**（Gemini等）が俯瞰視点で設計をチェックする。

- 「このファイル、ここにあるべき?」
- 「A1とA2の責務かぶってない?」
- 「これ機能Bに置くべきじゃない?」

木を見てる奴と森を見てる奴を分ける。

## 使い方

### 設計レビュー（推奨）

```rust
use ai_code_review::{CodeReviewer, Backend, PromptType};
use std::path::Path;

// プロジェクト全体を見る
let reviewer = CodeReviewer::new(Path::new("./src"))?
    .with_backend(Backend::Gemini)  // メインと別のAIを使う
    .with_prompt_type(PromptType::Architecture);

// ファイル単体ではなく、設計観点でレビュー
let result = reviewer.review_file(Path::new("src/services/auth.rs"))?;

// 「このモジュールの責務は適切か」
// 「他モジュールとの依存関係は正しいか」
println!("{}", result.review);
```

### ファイル監視モード

```rust
let mut reviewer = CodeReviewer::new(Path::new("./src"))?
    .with_backend(Backend::Gemini)
    .with_prompt_type(PromptType::Architecture)
    .on_review(|result| {
        if result.has_issues {
            // 設計上の問題を検出したら通知
            println!("⚠ 設計警告: {}", result.name);
            println!("{}", result.review);
        }
    });

reviewer.start()?;
```

## プロンプトタイプ

| タイプ | 用途 |
|--------|------|
| `Architecture` | **設計・責務分離の観点**（推奨） |
| `Default` | 総合的なレビュー |
| `Security` | セキュリティ観点 |
| `Quick` | 重大な問題のみ |

### Architectureプロンプトのチェック項目

1. 単一責任の原則（SRP）に違反していないか
2. 依存関係は適切か
3. モジュール間の結合度は低く保たれているか
4. このファイル/モジュールに置くべきコードか
5. より適切な配置場所はないか

## 典型的なワークフロー

```
┌─────────────────┐     ┌─────────────────┐
│  メインAgent    │     │  監視Agent      │
│  (Claude)       │     │  (Gemini)       │
│                 │     │                 │
│  実装に集中     │────▶│  設計を俯瞰     │
│  A1モジュール   │     │  A1,A2,B全体    │
│                 │     │                 │
│  コード書く     │◀────│  「Bに置くべき」│
└─────────────────┘     └─────────────────┘
```

## インストール

```toml
[dependencies]
ai-code-review = { git = "https://github.com/YuujiKamura/ai-code-review" }
```

## 依存クレート

- [folder-watcher](https://github.com/YuujiKamura/folder-watcher) - ファイル監視
- [cli-ai-analyzer](https://github.com/YuujiKamura/cli-ai-analyzer) - AI呼び出し（Gemini/Claude両対応）

## 既存ツールとの違い

| ツール | 観点 |
|--------|------|
| CodeRabbit, Copilot等 | **diff（変更差分）** を見る |
| このライブラリ | **設計（全体構造）** を見る |

diffレビューは「この変更おかしくない?」を見る。
設計レビューは「この実装、全体の中でどうなの?」を見る。

## ライセンス

MIT
