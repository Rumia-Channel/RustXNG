# Changelog

このファイルの形式は [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に従います。バージョニング方針は本文末尾の「セマンティックバージョニング」を参照してください。

## [Unreleased]

## [0.2.0] - 2026-10-18

SearXNG クライアントから見た互換性を高めたリリースです。JSON/CSV/config の形が変わるため **破壊的変更** として 0.2.0 にしました。

### Changed (breaking)

- `format=json` は上流 `get_json_response` の形（7 フィールド）だけを返すように変更。計測値などは `full=1` で取得
- `format=csv` の列を上流と同じ `title,url,content,host,engine,score,type` に変更（answer / suggestion / correction 行も出力）
- `/config` を上流と同じフィールド構成（`brand` / `limiter` / `locales` / エンジンの `timeout` など）に変更
- 設定 Cookie のカテゴリ名を `category` に変更（旧 `categories` も読み取り可）

### Added

- 上流 SearXNG の `settings.yml`（YAML）をそのまま読み込み可能（TOML も可）
- `full=1` による詳細 JSON
- 互換性レポート [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md)
- README に速度比較の節

## [0.1.0] - 2026-10-18

最初の公開版です。SearXNG の振る舞いを Rust に移植し、単一バイナリで動くメタ検索エンジンとしてまとめました。

### Added

- 検索コア: `!engine` / `!category` / `:lang` / `<timeout` / `!!` / `!!bang` の構文解析、結果の重複排除・スコアリング・カテゴリ別グループ化（上流 `searx/query.py` / `searx/results.py` 準拠）
- エンジン: 113 設定エントリ / 71 エンジン型。宣言的エンジン（`xpath` / `json_engine` / `mediawiki`）と上流 `settings.yml` の自動取込
- 外部 bang: 13,561 件（上流の圧縮トライを平坦化）
- HTTP: `/search`（HTML/JSON/CSV/RSS）、`/autocomplete`、`/config`、`/preferences`、`/stats`、`/metrics`、`/image_proxy`、`/favicon_proxy`、`/opensearch.xml`、`/healthz`
- サブシステム: プラグイン 11 種、アンサーラー（statistics / random / weather）、Wikidata インフォボックス、i18n 11 ロケール
- 運用: TTL+LRU キャッシュ、エンジン単位のサスペンド、簡易レート制限、エンジン統計
- 開発: テスト 298 件、ベンチマーク example、上流との比較スクリプト、カバレッジ計測、CI

## セマンティックバージョニング

- バージョンは [SemVer 2.0.0](https://semver.org/lang/ja/) に従います（`MAJOR.MINOR.PATCH`）
- **公開 API** とみなすもの: クレートの公開アイテム、HTTP エンドポイントとそのパラメータ、JSON/CSV/RSS の出力形、設定キーと環境変数、CLI オプション
- 1.0.0 未満の間は、互換性を壊す変更を **MINOR** で上げます（`0.2.0` → `0.3.0`）。1.0.0 以降は **MAJOR** です
- 互換性を保つ追加は MINOR、バグ修正のみは PATCH です
- 破壊的変更は CHANGELOG の `Changed (breaking)` に明記します
