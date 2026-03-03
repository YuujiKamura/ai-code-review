# Review Instructions

`ai-code-review` の CLI を使うときは、レビュー対象を必ず明示する。

## Rule

- Repoスコープのモードでは `--target <path>` を必須とする。
- 対象未指定での実行は禁止。

対象必須モード:

- `--diff`
- `--discover`
- `--hook`
- `--hook-install`

## Canonical Commands

```bash
# Diff review
review --diff --target C:/work/your-repo

# Discovery
review --discover --goal "..." --target C:/work/your-repo

# Hook run
review --hook --target C:/work/your-repo

# Hook install
review --hook-install --target C:/work/your-repo
```

## Anti-pattern

```bash
# NG: targetが無く、暗黙cwdに依存する呼び方
review --diff
```

## Preflight

実行時に `[target] <path>` が表示されることを確認する。  
表示されない場合は対象指定が誤っている可能性がある。
