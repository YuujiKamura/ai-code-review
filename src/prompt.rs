//! Review prompts in Japanese

use std::fmt::Write as FmtWrite;

use crate::context::{ProjectContext, RawContext};

/// Default code review prompt (Japanese)
pub const DEFAULT_REVIEW_PROMPT: &str = r#"以下のコード変更をレビューしてください。

ファイル: {file_name}

```
{content}
```

## レビュー観点（優先度順）

1. **設計・アーキテクチャ**
   - この変更はこのファイルにあるべきか（責務の分離）
   - 関数/モジュールの肥大化につながっていないか
   - 適切な抽象化がされているか
   - 高凝集か（関連する機能が1箇所にまとまっているか）
   - 疎結合か（他モジュールへの依存が最小限か、具体型ではなくインターフェースに依存しているか）

2. **コード品質**
   - 関数が長すぎないか（50行超えは要注意）
   - 重複コードはないか
   - 命名は適切か

3. **バグ・セキュリティ**（明らかな問題のみ）
   - 潜在的なバグ
   - セキュリティリスク

## 出力形式

- 問題がある場合は「⚠」で具体的に指摘
- 設計改善の提案があれば「💡」で提案
- 重大な問題があれば「🚨」で警告
- 問題がない場合は「✓ 問題なし」
- 簡潔に（5行以内）"#;

/// Quick review prompt (shorter, faster)
pub const QUICK_REVIEW_PROMPT: &str = r#"以下のコード変更を簡潔にレビューしてください。

ファイル: {file_name}

```
{content}
```

重大な問題のみ指摘してください。問題がなければ「✓ OK」と回答。
2行以内で回答。"#;

/// Security-focused review prompt
pub const SECURITY_REVIEW_PROMPT: &str = r#"以下のコードをセキュリティ観点でレビューしてください。

ファイル: {file_name}

```
{content}
```

## チェック項目

1. インジェクション脆弱性（SQL, コマンド, XSS等）
2. 認証・認可の問題
3. 機密情報の露出（APIキー、パスワード等）
4. 安全でない暗号化・ハッシュ
5. パストラバーサル

## 出力形式

- 🚨 重大なセキュリティリスク
- ⚠ 潜在的なリスク
- ✓ セキュリティ上の問題なし"#;

/// Architecture review prompt
pub const ARCHITECTURE_REVIEW_PROMPT: &str = r#"以下のコードをアーキテクチャの観点からレビューしてください。

ファイル: {file_name}

```
{content}
```

## チェック項目

1. 単一責任の原則（SRP）に違反していないか
2. 依存関係は適切か（循環依存がないか）
3. 高凝集か（1つのモジュールが関連する機能だけを持ち、無関係な責務が混在していないか）
4. 疎結合か（モジュール間の依存が最小限で、具体型ではなくトレイト/インターフェースに依存しているか）
5. このファイル/モジュールに置くべきコードか、より適切な配置場所はないか
6. public APIは最小限か

## 出力形式

- 💡 配置場所の改善提案
- ⚠ 責務の重複・設計上の問題
- 🔄 関連ファイルとの不整合
- ✓ 構造上の問題なし"#;

/// Analyze prompt - let AI analyze code structure and patterns
pub const ANALYZE_PROMPT: &str = r#"以下のコードを分析してください。

{context}

## 分析してほしいこと

1. **このコードは何をしているか** - 目的と責務
2. **依存関係** - 何をimport/使用しているか、何から使用されているか
3. **設計パターン** - 使われているパターン、または使うべきパターン
4. **改善点** - 構造上の問題、リファクタリングの余地

簡潔に回答してください。
"#;

/// Discovery prompt - helps expand project from goal to architecture
pub const DISCOVERY_PROMPT: &str = r#"以下のプロジェクトについて、目的からアーキテクチャへの展開を支援してください。

## 目的
{goal}

## 現在の構造
{structure}

## 分析してほしいこと

1. **責務の発見**
   - この目的を達成するために必要な責務は何か
   - それぞれの責務は独立しているか、依存関係はあるか

2. **境界の設計**
   - モジュール/ファイルとしてどう分割すべきか
   - 入力・処理・出力の境界はどこか
   - 外部との接点（API、CLI、ファイル等）はどこか

3. **不足の指摘**
   - 現在の構造に足りないものは何か
   - 追加すべきモジュール/ファイルは何か

4. **次のステップ**
   - 今すぐやるべきことは何か（1-3個）
   - 後回しにしていいことは何か

## 出力形式

### 責務マップ
```
責務A: 説明
  → 配置先: src/xxx.rs
責務B: 説明
  → 配置先: src/yyy.rs
```

### 推奨構造
```
src/
├── ...
```

### 次のアクション
1. ...
2. ...
"#;

/// Holistic review prompt - checks code against project requirements
pub const HOLISTIC_REVIEW_PROMPT: &str = r#"以下のコードを、プロジェクト全体の文脈からレビューしてください。

{content}

## チェック項目

1. **要件との整合性**
   - コードはプロジェクトの目的に沿っているか
   - 命名はドメイン用語と一致しているか
   - 欠けている機能はないか

2. **表現の適切さ**
   - このコードは意図を明確に表現しているか
   - 抽象化レベルは適切か（技術詳細 vs ビジネスロジック）
   - 他の開発者が読んで目的を理解できるか

3. **プロジェクト構造との調和**
   - このファイルの役割は明確か
   - 他のモジュールとの責務分担は適切か

## 出力形式

- 💡 表現改善の提案
- ⚠ 要件との乖離
- 🎯 目的との整合性の問題
- ✓ 問題なし"#;

/// Architecture review prompt with context placeholder
pub const ARCHITECTURE_REVIEW_WITH_CONTEXT_PROMPT: &str = r#"以下のコードをアーキテクチャの観点からレビューしてください。

{context}

ファイル: {file_name}

```
{code}
```

## チェック項目（コンテキスト情報を踏まえて）

1. このファイルの責務は、同じディレクトリの他ファイルと重複していないか
2. 高凝集か（このファイル内の関数・型は共通の目的に向かっているか、無関係な機能が混在していないか）
3. 疎結合か（他ファイルへの依存は最小限か、具体型ではなくトレイト/インターフェースに依存しているか）
4. 関連ファイル（一緒に変更されたファイル）との整合性は取れているか
5. 依存方向は適切か（循環依存がないか）
6. このファイルにあるべきコードか、別の場所が適切か
7. public APIは最小限か

## 出力形式

- 💡 配置場所の改善提案
- ⚠ 責務の重複・設計上の問題
- 🔄 関連ファイルとの不整合
- ✓ 構造上の問題なし"#;

/// Build a prompt with context information
pub fn build_prompt_with_context(
    template: &str,
    file_name: &str,
    code: &str,
    context: &str,
) -> String {
    template
        .replace("{file_name}", file_name)
        .replace("{code}", code)
        .replace("{content}", &format!("{}\n\n{}", context, code))
        .replace("{context}", context)
}

/// Build a prompt from template
pub fn build_prompt(template: &str, file_name: &str, content: &str) -> String {
    template
        .replace("{file_name}", file_name)
        .replace("{content}", content)
}

/// Build a discovery prompt with goal and project structure
pub fn build_discovery_prompt(template: &str, goal: &str, structure: &str) -> String {
    template
        .replace("{goal}", goal)
        .replace("{structure}", structure)
}

/// Build an analyze prompt with raw context
pub fn build_analyze_prompt(template: &str, context: &str) -> String {
    template.replace("{context}", context)
}

/// Prompt type for easy selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptType {
    /// Default comprehensive review
    #[default]
    Default,
    /// Quick review (minimal)
    Quick,
    /// Security-focused review
    Security,
    /// Architecture-focused review
    Architecture,
    /// Holistic review - checks code against project requirements
    Holistic,
    /// Discovery - helps expand project from goal to architecture
    Discovery,
    /// Analyze - let AI analyze code structure (minimal parsing, AI does the work)
    Analyze,
    /// Custom prompt
    Custom,
}

impl PromptType {
    /// Get the template for this prompt type
    pub fn template(&self) -> &'static str {
        match self {
            PromptType::Default => DEFAULT_REVIEW_PROMPT,
            PromptType::Quick => QUICK_REVIEW_PROMPT,
            PromptType::Security => SECURITY_REVIEW_PROMPT,
            PromptType::Architecture => ARCHITECTURE_REVIEW_PROMPT,
            PromptType::Holistic => HOLISTIC_REVIEW_PROMPT,
            PromptType::Discovery => DISCOVERY_PROMPT,
            PromptType::Analyze => ANALYZE_PROMPT,
            PromptType::Custom => "", // Custom prompts provide their own template
        }
    }

    /// Check if this prompt type requires a goal instead of file content
    pub fn requires_goal(&self) -> bool {
        matches!(self, PromptType::Discovery)
    }

    /// Check if this prompt type uses raw context (AI does the parsing)
    pub fn uses_raw_context(&self) -> bool {
        matches!(self, PromptType::Analyze | PromptType::Discovery)
    }
}
/// Format a `ProjectContext` into a prompt-friendly string
///
/// This is the presentation logic for `ProjectContext`. The data collection
/// lives in `context.rs`, while this function handles how that data is
/// rendered into a prompt string.
pub fn format_project_context(ctx: &ProjectContext) -> String {
    let mut output = String::new();

    // Project description (from requirements)
    if let Some(ref desc) = ctx.requirements.description {
        output.push_str("## プロジェクト概要\n");
        output.push_str(desc);
        output.push_str("\n\n");
    }

    // README summary (from requirements)
    if let Some(ref readme) = ctx.requirements.readme_summary {
        output.push_str("## README（抜粋）\n");
        output.push_str(readme);
        output.push_str("\n\n");
    }

    // Module docs (from requirements)
    if let Some(ref docs) = ctx.requirements.module_docs {
        output.push_str("## モジュールドキュメント\n");
        output.push_str(docs);
        output.push_str("\n\n");
    }

    // Module structure
    if !ctx.module_tree.is_empty() {
        output.push_str("## プロジェクト構造\n```\n");
        output.push_str(&ctx.module_tree);
        output.push_str("```\n\n");
    }

    // Related files (co-changed)
    if !ctx.related_files.is_empty() {
        output.push_str("## 最近一緒に変更されたファイル\n");
        for rf in &ctx.related_files {
            let _ = writeln!(output, "- {} ({}回)", rf.path, rf.co_change_count);
        }
        output.push('\n');
    }

    // Dependencies
    if !ctx.dependencies.imports.is_empty() || !ctx.dependencies.imported_by.is_empty() {
        output.push_str("## 依存関係\n");
        if !ctx.dependencies.imports.is_empty() {
            output.push_str("このファイルが使用: ");
            output.push_str(&ctx.dependencies.imports.join(", "));
            output.push('\n');
        }
        if !ctx.dependencies.imported_by.is_empty() {
            output.push_str("このファイルを使用: ");
            output.push_str(&ctx.dependencies.imported_by.join(", "));
            output.push('\n');
        }
        output.push('\n');
    }

    // Sibling files
    if !ctx.sibling_files.is_empty() {
        output.push_str("## 同じディレクトリのファイル\n");
        output.push_str(&ctx.sibling_files.join(", "));
        output.push_str("\n\n");
    }

    output
}

/// Format a `RawContext` into a prompt-friendly string
///
/// This is the presentation logic for `RawContext`. The data collection
/// lives in `context.rs`, while this function handles how that data is
/// rendered into a prompt string.
pub fn format_raw_context(ctx: &RawContext) -> String {
    let mut result = String::new();

    // Structure
    if !ctx.structure.is_empty() {
        result.push_str("## プロジェクト構造\n```\n");
        result.push_str(&ctx.structure);
        result.push_str("```\n\n");
    }

    // Co-changed files
    if !ctx.cochanged.is_empty() {
        result.push_str("## 一緒に変更されるファイル\n");
        for (file, count) in &ctx.cochanged {
            result.push_str(&format!("- {} ({}回)\n", file, count));
        }
        result.push('\n');
    }

    // Related file contents
    if !ctx.related_files.is_empty() {
        result.push_str("## 関連ファイルの内容\n");
        for (name, file_content) in &ctx.related_files {
            result.push_str(&format!("### {}\n```\n", name));
            // Truncate if too long
            if file_content.len() > 2000 {
                // Find safe UTF-8 boundary
                let truncate_at = file_content.floor_char_boundary(2000);
                result.push_str(&file_content[..truncate_at]);
                result.push_str("\n... (truncated)");
            } else {
                result.push_str(file_content);
            }
            result.push_str("\n```\n\n");
        }
    }

    // Docs
    if let Some(docs) = &ctx.docs {
        result.push_str("## プロジェクト要件/ドキュメント\n");
        result.push_str(docs);
        result.push('\n');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt() {
        let prompt = build_prompt(QUICK_REVIEW_PROMPT, "test.rs", "fn main() {}");
        assert!(prompt.contains("test.rs"));
        assert!(prompt.contains("fn main() {}"));
    }

    #[test]
    fn test_prompt_type_template() {
        assert!(!PromptType::Default.template().is_empty());
        assert!(!PromptType::Quick.template().is_empty());
        assert!(!PromptType::Security.template().is_empty());
        assert!(!PromptType::Architecture.template().is_empty());
        assert!(!PromptType::Holistic.template().is_empty());
        assert!(PromptType::Custom.template().is_empty());
    }

    #[test]
    fn test_build_prompt_with_context() {
        let context = "## プロジェクト構造\nsrc/\n└── main.rs";
        let prompt = build_prompt_with_context(
            ARCHITECTURE_REVIEW_WITH_CONTEXT_PROMPT,
            "test.rs",
            "fn main() {}",
            context,
        );
        assert!(prompt.contains("test.rs"));
        assert!(prompt.contains("fn main() {}"));
        assert!(prompt.contains("プロジェクト構造"));
    }
}
