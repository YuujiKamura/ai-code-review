//! Review prompts in Japanese

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
pub const ARCHITECTURE_REVIEW_PROMPT: &str = r#"以下のコード変更をアーキテクチャの観点からレビューしてください。

ファイル: {file_name}

```
{content}
```

## チェック項目

1. 単一責任の原則（SRP）に違反していないか
2. 依存関係は適切か
3. モジュール間の結合度は低く保たれているか
4. このファイル/モジュールに置くべきコードか
5. より適切な配置場所はないか

## 出力形式

- 💡 アーキテクチャ改善の提案
- ⚠ 設計上の問題
- ✓ 構造上の問題なし"#;

/// Build a prompt from template
pub fn build_prompt(template: &str, file_name: &str, content: &str) -> String {
    template
        .replace("{file_name}", file_name)
        .replace("{content}", content)
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
            PromptType::Custom => "", // Custom prompts provide their own template
        }
    }
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
        assert!(PromptType::Custom.template().is_empty());
    }
}
