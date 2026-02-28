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

1. **設計・結合の均衡**（強度×距離×変動性で判断）
   - 強い結合が近い距離にあるか（高凝集＝良い）。関連機能が1箇所にまとまっているか
   - 弱い結合が遠い距離にあるか（疎結合＝良い）。モジュール境界を越える依存はコントラクト結合（公開APIのみ）か
   - 強い結合が遠い距離にないか（大域的複雑性＝悪い）。遠いモジュールの内部実装やDB直接参照がないか
   - 変動性の高いコード（コアロジック）ほど厳密な分離が必要。変動しないコードは多少の結合を許容

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
3. 結合の均衡は取れているか（強度×距離×変動性の3軸で判断）
   - 侵入結合（他の非公開実装やDBへの直接依存）がないか → コントラクト結合（公開APIのみ）に改善できないか
   - 強い結合は距離が近いか（同モジュール内＝高凝集で良い）、遠いのに強い結合は大域的複雑性（悪い）
   - 変動性の高い部分（コアロジック）ほど結合強度を下げるべき。安定した部分は許容
4. このファイル/モジュールに置くべきコードか、より適切な配置場所はないか
5. public APIは最小限か

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

/// Coding principles review prompt - checks against well-known design principles
pub const PRINCIPLES_REVIEW_PROMPT: &str = r#"以下のコードを、コーディング原則の観点からレビューしてください。

ファイル: {file_name}

```
{content}
```

## チェック項目（違反がある場合のみ指摘。各原則のBad/Good例を参考に判断せよ）

### 1. DRY（Don't Repeat Yourself）
同じ知識・ロジックが複数箇所に重複していないか。

Bad: 重複したバリデーション
```
class User { validates :email, presence: true, format: EMAIL_REGEXP }
class Admin { validates :email, presence: true, format: EMAIL_REGEXP }
```
Good: 共通化
```
module EmailValidatable
  included { validates :email, presence: true, format: EMAIL_REGEXP }
end
```

### 2. Tell, don't Ask
オブジェクトに状態を問い合わせて外で判断していないか。判断はオブジェクト自身に委譲すべき。

Bad: 外で判断
```
if user.admin? then user.grant_access else user.deny_access end
```
Good: オブジェクトに委譲
```
user.handle_access_request
# User内部で admin? を判断して grant/deny を決定
```

### 3. SRP（単一責任の原則）
1つのクラス/モジュールが複数の責務を持っていないか。

Bad: 複数責務
```
class User
  def save_to_database ... end  # データ永続化
  def send_welcome_email ... end  # メール送信
  def generate_report ... end  # レポート生成
end
```
Good: 責務を分離
```
class User ... end           # ユーザー情報管理のみ
class UserMailer ... end     # メール送信専用
class UserReportGenerator ... end  # レポート生成専用
```

### 4. OCP（開放閉鎖の原則）
新しい種類を追加するたびに既存コードを修正する構造（if/else連鎖）になっていないか。

Bad: 修正のたびにクラスを変更
```
fn calculate(customer_type: &str) -> f64 {
    if customer_type == "Regular" { price }
    else if customer_type == "Premium" { price * 0.9 }
    else if customer_type == "VIP" { price * 0.8 }
    // 新しい顧客タイプが増えるたびに修正が必要
}
```
Good: 拡張に開放、修正に閉鎖（ストラテジーパターン）
```
trait PricingStrategy { fn calculate(&self, base: f64) -> f64; }
struct RegularPricing;
impl PricingStrategy for RegularPricing { fn calculate(&self, base: f64) -> f64 { base } }
struct PremiumPricing;
impl PricingStrategy for PremiumPricing { fn calculate(&self, base: f64) -> f64 { base * 0.9 } }
```

### 5. LSP（リスコフの置換原則）
親の型を子の型で置換したとき、動作が破綻しないか。

Bad: 置換すると動作が壊れる
```
// Square extends Rectangle だが、Width設定時にHeightも変わる
// → Rectangle として使うと面積計算が期待と異なる
```
Good: 適切な抽象化
```
abstract class Shape { abstract area(): number }
class Rectangle extends Shape { ... }
class Square extends Shape { ... }  // 独立した実装
```

### 6. ISP（インターフェース分離の原則）
クライアントが使わないメソッドに依存させられていないか。

Bad: 大きすぎるインターフェース
```
trait Worker { fn work(); fn eat(); fn sleep(); }
// ロボットは eat/sleep を実装できない
```
Good: 責務ごとに分離
```
trait Workable { fn work(); }
trait Feedable { fn eat(); }
trait Sleepable { fn sleep(); }
```

### 7. DIP（依存性逆転の原則）
具象クラスに直接依存していないか。抽象（trait/interface）に依存すべき。

Bad: 具象に直接依存
```
struct OrderProcessor {
    repository: MySqlRepository,  // 具象型
    email: SmtpEmailService,      // 具象型
}
```
Good: 抽象に依存
```
struct OrderProcessor {
    repository: Box<dyn Repository>,    // trait object
    email: Box<dyn EmailService>,       // trait object
}
```

### 8. Composition over Inheritance（継承より委譲）
継承で解決しているが委譲の方が適切なケースはないか。

Bad: 継承の乱用
```
class FlyingCar extends Car implements Flyable { ... }
```
Good: 委譲
```
struct FlyingCar { car: Car, flight: FlightSystem }
fn drive(&self) { self.car.drive() }
fn fly(&self) { self.flight.take_off() }
```

### 9. デメテルの法則（最小知識の原則）
`a.b.c.d` のようなチェーンで他のオブジェクトの内部構造に依存していないか。

Bad: チェーンが長い
```
customer.address.city.postal_code
```
Good: 適切な委譲
```
customer.postal_code()
// Customer内部で address?.postal_code を返す
```

### 10. 高凝集・疎結合（GRASP）
関連する処理がまとまっているか（高凝集）。モジュール間の依存は最小か（疎結合）。

Bad: 低凝集（無関係な責務が混在）
```
class User {
  fn save_to_database() ...  // データ永続化
  fn send_email() ...        // メール
  fn calculate_tax() ...     // 税金計算
  fn format_address() ...    // 住所フォーマット
}
```
Good: 高凝集（ユーザー情報管理に集中）
```
class User { fn full_name(); fn age(); fn adult?(); }
class UserPersistence { fn save(user); }    // 永続化は別クラス
class UserNotifier { fn send_welcome(user); }  // 通知は別クラス
```

### 11. KISS（Keep It Simple, Stupid）
不必要に複雑な実装になっていないか。シンプルな方法で書けるのに遠回りしていないか。

Bad: 複雑な条件分岐のネスト
```
if user.premium? && product.category == 'electronics' &&
   ['winter','summer'].include?(season) && ['sat','sun'].include?(day)
  product.price * 0.2
elsif user.regular? && product.on_sale? && season == 'spring'
  product.price * 0.1
# ... さらに続く
```
Good: 戦略パターンで分離
```
strategies = find_applicable_strategies(user, product, context)
strategies.map(&:discount).max || 0
```

### 12. YAGNI（You Aren't Gonna Need It）
現時点で不要な機能・抽象化を先回りして実装していないか。

Bad: 「将来使うかも」で先回り実装
```
struct User {
    first_name: String, last_name: String, email: String,
    middle_name: String,           // 不要
    alternative_emails: Vec<String>, // 不要
    login_count: u32,              // 不要
    preferred_language: String,    // 不要
    timezone: String,              // 不要
}
```
Good: 現在必要な機能のみ
```
struct User { first_name: String, last_name: String, email: String }
// 必要になったら追加する
```

### 13. CQS（コマンドクエリ分離）
状態変更と値の取得を同時に行うメソッドがないか。

Bad: 状態変更と取得が混在
```
fn pop(&mut self) -> T {
    let value = self.stack.last().unwrap();  // 取得
    self.stack.pop();                         // 状態変更
    value
}
```
Good: 分離
```
fn peek(&self) -> &T { self.stack.last().unwrap() }  // クエリのみ
fn pop(&mut self) { self.stack.pop(); }               // コマンドのみ
```

### 14. 関心の分離（Separation of Concerns）
UI/ビジネスロジック/データアクセスが混在していないか。

Bad: 全部が1つのハンドラに混在
```
fn create_user(req) {
    // バリデーション + DB保存 + メール送信 が全部ここにある
    if email.is_empty() { return error; }
    db.save(user);
    mailer.send(user.email);
}
```
Good: 層ごとに分離
```
// Controller: リクエスト処理のみ
// Service: ビジネスロジック
// Repository: データアクセス
fn create_user(req) { service.create_user(req) }
```

### 15. IoC（制御の反転）
オブジェクト自身が依存関係を生成していないか。外部から注入すべき。

Bad: 内部で依存を生成
```
struct OrderService {
    fn new() -> Self {
        Self { repo: SqlRepository::new(), email: SmtpService::new() }
    }
}
```
Good: 外部から注入
```
struct OrderService { repo: Box<dyn Repository>, email: Box<dyn EmailService> }
fn new(repo: impl Repository, email: impl EmailService) -> Self { ... }
```

## 出力形式

- 🚨 原則違反（重大: 保守性・拡張性に直接影響）
- ⚠ 原則違反（軽微: 改善推奨）
- 💡 原則に基づく改善提案
- ✓ 主要原則に違反なし

各指摘には「どの原則（番号と名前）に違反しているか」を明記し、対象コードのBefore/After方向性を示すこと。"#;

/// Investigate prompt - cross-file investigation driven by a user question
pub const INVESTIGATE_PROMPT: &str = r#"以下のコードベースについて、ユーザーの疑問を調査してください。

## 調査対象の質問
{question}

## コードベース
{context}

## 調査方針
1. 質問に関連するデータフロー・型定義・API呼び出しを追跡
2. ファイル間の宣言と使用の不整合を特定
3. 期待される振る舞いと実際のコードの差異を報告

## 出力形式
### 調査結果
- 発見事項をファイル名:行番号付きで報告
### 結論
- 問題の根本原因を簡潔に述べる
### 推奨アクション
- 具体的な修正案（あれば）
"#;

/// Shared code discovery prompt - analyzes cross-project sharing opportunities
pub const FIND_SHARED_PROMPT: &str = r#"以下は2つのプロジェクト間の共有コード候補の分析結果です。

{context}

## 分析してほしいこと

各候補について以下を判断してください：

1. **共通化すべきか** - 両プロジェクトで同じロジック/データを持つべきでない場合
   - 共通ライブラリに切り出すべき（変更時に両方更新が必要になるリスク）
   - 設定ファイル（JSON等）として外部化して共有すべき
   - そのまま別々に持つのが適切（偶然の類似に過ぎない）

2. **優先度** - 高/中/低
   - 高: 頻繁に変更される or バグの温床になる重複
   - 中: たまに変更される or 一致させ忘れるリスク
   - 低: 安定していてほぼ変更されない

3. **具体的なアクション提案**
   - どのファイルをどう統合するか
   - 共通モジュールの配置場所

## 出力形式

### 共通化推奨
- 🔄 [高] 具体的な提案
- 🔄 [中] 具体的な提案

### 現状維持
- ✓ 理由

### 次のステップ
1. 最初にやるべきこと
2. 次にやるべきこと
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
2. 結合の均衡は取れているか（強度×距離×変動性で判断）
   - 強い結合（内部型・実装の共有）が近い距離（同モジュール内）にあるか → 高凝集＝良い
   - 遠い距離（別モジュール・別サービス）への結合はコントラクト結合（公開APIのみ）か → 疎結合＝良い
   - 遠いのに強い結合（他モジュールのDB直接参照、非公開型への依存）がないか → 大域的複雑性＝悪い
   - 変動性の高いコード（頻繁に変わるビジネスロジック）ほど結合を弱くすべき
3. 関連ファイル（一緒に変更されたファイル）との整合性は取れているか
4. 依存方向は適切か（循環依存がないか）
5. このファイルにあるべきコードか、別の場所が適切か
6. public APIは最小限か

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

/// Build a find-shared prompt with analysis context
pub fn build_find_shared_prompt(template: &str, context: &str) -> String {
    template.replace("{context}", context)
}

/// Build an investigate prompt with question and codebase context
pub fn build_investigate_prompt(template: &str, question: &str, context: &str) -> String {
    template
        .replace("{question}", question)
        .replace("{context}", context)
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
    /// Coding principles review (DRY, SOLID, GRASP, KISS, YAGNI, etc.)
    Principles,
    /// Investigate - cross-file investigation driven by a user question
    Investigate,
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
            PromptType::Principles => PRINCIPLES_REVIEW_PROMPT,
            PromptType::Discovery => DISCOVERY_PROMPT,
            PromptType::Analyze => ANALYZE_PROMPT,
            PromptType::Investigate => INVESTIGATE_PROMPT,
        }
    }

    /// Check if this prompt type requires a goal instead of file content
    pub fn requires_goal(&self) -> bool {
        matches!(self, PromptType::Discovery | PromptType::Investigate)
    }

    /// Check if this prompt type uses raw context (AI does the parsing)
    pub fn uses_raw_context(&self) -> bool {
        matches!(self, PromptType::Analyze | PromptType::Discovery | PromptType::Investigate)
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
        assert!(!PromptType::Principles.template().is_empty());
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
