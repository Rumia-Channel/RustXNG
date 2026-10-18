# Changelog

このファイルの形式は [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に従い、バージョンは [Semantic Versioning](https://semver.org/lang/ja/) を意識しています。

## [Unreleased]

## [0.1.0] - 2026-10-18

最初の公開版です。SearXNG の振る舞いを Rust に移植し、単一バイナリで動くメタ検索エンジンとしてまとめました。

### Added

- 検索コア: `!engine` / `!category` / `:lang` / `<timeout` / `!!` / `!!bang` の構文解析、結果の重複排除・スコアリング・カテゴリ別グループ化（上流 `searx/query.py` / `searx/results.py` 準拠）
- エンジン: 113 設定エントリ / 71 エンジン型。宣言的エンジン（`xpath` / `json_engine` / `mediawiki`）と上流 `settings.yml` の自動取込
- 外部 bang: 13,561 件（上流の圧縮トライを平坦化）
- HTTP: `/search`（HTML/JSON/CSV/RSS）、`/autocomplete`、`/config`、`/preferences`、`/stats`、`/stats/errors`、`/metrics`（OpenMetrics）、`/image_proxy`、`/favicon_proxy`、`/opensearch.xml`、`/healthz`、`/about`
- サブシステム: プラグイン 11 種、アンサーラー（statistics / random / weather）、Wikidata インフォボックス、i18n 11 ロケール
- 運用: TTL+LRU キャッシュ、エンジン単位のサスペンド（`suspended_times` 準拠）、簡易レート制限、エンジン統計
- 最新情報: news カテゴリと時間範囲フィルタ（エンジンごとのネイティブ日付フィルタへ変換）
- 開発: テスト 295 件（ユニット 281 + 統合 14）、ベンチマーク example、上流 SearXNG との比較スクリプト（`bench/compare.py`）、カバレッジ計測（`tools/coverage.py`）、CI（fmt / test / example build）
- ドキュメント: `docs/PORTING.md`、`docs/COVERAGE.md`、`docs/BENCHMARK.md`

### Known limitations

- 上流 227 エンジン型のうち 156 型が未移植（API キー必須・自前インスタンス・torrent 系は方針として対象外・特殊 DB バックエンド）
- valkey / redis キャッシュ、botdetection 本体、i18n のテンプレート全面適用は未対応
