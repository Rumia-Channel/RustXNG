# Security Policy

## サポート対象

セキュリティ修正は `main` ブランチに対して行います。リリースタグを切っている場合は最新のマイナー系列が対象です。

## 脆弱性の報告

**公開 Issue を作らないでください。** 次のいずれかで非公開に報告してください。

1. GitHub の [Security Advisories](https://github.com/sahenjp/rustxng/security/advisories/new)（推奨）
2. リポジトリのメンテナへの直接連絡

報告に含めてほしい情報:

- 影響を受けるバージョン（コミットハッシュ可）と設定
- 再現手順（リクエスト例、クエリ文字列、環境）
- 想定される影響（SSRF、XSS、DoS、情報漏えいなど）
- 可能なら修正案

初回の応答は 7 日以内を目安にします。修正が公開されるまで、詳細の公開は控えてください。

## このプロジェクトの防御機構

- **画像・favicon プロキシ**: HMAC-SHA256 署名（`server.secret_key`）を検証し、私設アドレス・非 http(s)・非画像レスポンスを拒否します（SSRF ガード）
- **テンプレート**: askama による自動エスケープ。ユーザー入力は HTML として解釈されません
- **レート制限**: `server.limiter = true` でクライアント単位の固定窓制限（既定は無効）
- **外部 URL**: 検索結果の URL は http(s) のみを扱います

## 既知の注意点

- `server.secret_key` の既定値（`ultrasecretkey`）は必ず変更してください。画像プロキシの署名と Cookie の整合性に使われます
- 公開インスタンスでは `server.limiter` とリバースプロキシ側の制限を併用してください
- 本プロジェクトは上流 SearXNG の botdetection（link token / Redis カウンタ）を完全には移植していません
