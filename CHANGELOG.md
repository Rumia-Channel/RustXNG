# Changelog

このファイルの形式は [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に従います。バージョニング方針は本文末尾の「セマンティックバージョニング」を参照してください。

## [Unreleased]

### Added

- ビルド済みバイナリの配布: Linux（glibc / 静的 musl）、macOS（arm64 / x86_64）、
  Windows（x86_64）をタグ push で自動ビルドし、`.sha256` とともに Release に添付
  （`.github/workflows/release.yml`）。資産名は `rustxng-<tag>-<target>` で、
  rustdsh の `rdsh rustxng install` がこの名前で選ぶ
- CI を Linux・macOS・Windows の matrix に拡張し、clippy `-D warnings`、
  `--no-default-features` ビルド、Docker イメージの `/healthz` 起動確認を追加

## [0.4.0] - 2026-10-24

前回の「未移植」項目をすべて埋め、実測でボトルネックを潰したリリースです（後方互換）。

### Added

- **DB 直結エンジン 8 種を実装**: postgresql, mysql_server, mariadb_server, mongodb, sqlite, valkey_server, recoll, opensemantic
  - 検索バックエンド 3 種（elasticsearch / solr / meilisearch）と合わせ、上流の DB 系 11 種がすべて動作
  - `db-engines` Cargo フィーチャー（既定で有効）。`--no-default-features` でバイナリを 27 MB → 17 MB に削減可能
  - 結果は key/value 形式（`keyvalue` テンプレート）で表示
- **Valkey / Redis キャッシュ**: `[cache] backend = "valkey"` で複数インスタンスがキャッシュを共有
- **botdetection 相当のレート制限**: 信頼プロキシの X-Forwarded-For 解決、IPv4/IPv6 CIDR、IP 制限のスライディング窓、リンクトークン、429 応答
- **i18n を 57 ロケールに拡張**（11 → 57。上流は 60）

### Changed

- エンジン名が大文字小文字を区別せずに一致するように
- 未知のエンジン名を指定した場合は SearXNG と同じく既定のエンジン群で検索（従来は 400 エラー）
- ベンチマーク example がエンジン別の応答時間を表示

### Performance

- タイムアウトした要求を再試行しない（上流も接続エラーのみ再試行）: 全エンジン検索の中央値 6.26 秒 → 3.00 秒
- 再試行を含めた全体デッドラインを厳守
- ウォーム時（キャッシュ + エンジンサスペンド）: 中央値 276 ms、単一サイト検索は 1〜50 ms

### Tests

- ユニット 750 + 統合 14

## [0.3.0] - 2026-10-24

全エンジンの移植を完了したリリースです（後方互換）。

### Added

- **エンジン 226 型 / 上流 352 エントリをすべて実装（100%）**
  - 検索系: yandex, yahoo, seznam, marginalia, yep, swisscows, quark, sogou, baidu 系, naver ほか
  - 動画/音楽: vimeo, rumble, odysee, niconico, youtube_api, bilibili, iqiyi, acfun, yandex_music, spotify
  - 画像: google_images, flickr, pinterest, pixiv, flaticon, iconify, lucide, material_icons, tineye, unsplash 系
  - 学術: pubmed, europepmc, openalex, springer, astrophysics_data_system, pdbe, nvd, chatnoir, scanr_structures
  - パッケージ/開発: github_code, gitlab, sourcehut, gitea, pkg_go_dev, hex, metacpan, lib_rs, nvd, repology, fdroid
  - ショッピング: ebay, geizhals, steam, shopify_stock, apkmirror, apple_app_store, google_play
  - 地域/ニュース: tagesschau, reuters, yahoo_news, swisscows_news, il_post, ansa, gmx, www1x, neosearch, xprivo ほか
  - torrent/ファイル: piratebay, nyaa, kickass, solidtorrents, torznab, tokyotoshokan, btdigg, bt4g, annas_archive, zlibrary
  - 検索バックエンド: elasticsearch, solr, meilisearch（HTTP 完全実装）
  - 翻訳/天気/その他: deepl, libretranslate, lingva, mozhi, translated, open_meteo, wttr, duckduckgo_weather, currency_convert
- エンジン初期化に失敗しても起動を継続する挙動（上流と同じ。失敗理由は警告ログに記録）
- レビュー補助: `tools/register_engines.py`（エンジンの登録を自動同期）、`tools/verify_boot.py`

### Changed

- 既定で有効なエンジンを約 170 に拡大（認証情報・自前インスタンスが必要なものは上流と同じく無効）
- ドキュメント: COVERAGE / COMPATIBILITY / README を最新の数値に更新

### Tests

- ユニット 722 + 統合 14（外部ネットワーク不要）

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
