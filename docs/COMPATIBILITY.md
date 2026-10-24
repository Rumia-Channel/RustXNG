# SearXNG 互換性の範囲

**結論: エンジン（352 エントリ / 226 型）とプロトコル（HTTP API・設定・データ形式）は上流と一致しています。** 残る差分は UI のマークアップと i18n のロケール数（57 / 60）です。このページは「何が互換で、何が違うか」を検証可能な形でまとめたものです。

## 互換と言える範囲

| 領域 | 状態 | 根拠 |
| :--- | :--- | :--- |
| 検索構文（`!engine` / `!category` / `:lang` / `<timeout` / `!!` / `!!bang`） | **互換** | 上流 `searx/query.py` と同じ判定順・同じ正規化 |
| 結果の統合（重複排除・スコア・カテゴリ別グループ化） | **互換** | `searx/results.py` の `calculate_score` / `get_ordered_results` と同じ式 |
| `/search` のパラメータ名・出力形式（html/json/csv/rss） | **互換** | 統合テストで検証 |
| JSON API の形 | **互換（既定）** | 上流 `get_json_response` の 7 フィールドのみを出力。詳細版は `full=1` |
| JSON の各要素の形 | **互換** | `answers` = `[{"answer":...}]`、`unresponsive_engines` = `[[engine, "Translated error"]]`、`template` = `"default.html"`、`metadata` = 文字列、`priority` 既定は空文字 |
| CSV 出力 | **互換** | 上流 `write_csv_response` と同じ列（`title,url,content,host,engine,score,type`）と同じ行種別（result / answer / suggestion / correction） |
| `/config` | **互換** | 上流と同じフィールド（`brand`、`limiter`、`locales`、`engines[].timeout` などを含む） |
| 設定ファイル | **互換（YAML をそのまま読める）** | `--config settings.yml` / `SEARXNG_SETTINGS_PATH` に上流の YAML をそのまま指定可能。TOML も可 |
| 環境変数（`SEARXNG_*`） | **互換** | 同じ名前を受け付け |
| 外部 bang | **データ互換** | 上流の 13,561 件を同一データから生成 |
| Cookie 名 | **互換** | `category` / `language` / `locale` / `safesearch` / `theme` / `disabled_engines` など上流と同じ（旧名 `categories` も読み取り可能） |
| OpenMetrics のエンジン指標名 | **互換** | `searxng_engines_*` |

## JSON API

既定の `format=json` は上流の形だけを返します。

`length` / `iframe_src` / `audio_src` / `views` など本実装が持たないフィールドは、エンジン側の `metadata` 文字列に含まれます（上流と同じ「メタデータ」表現）。

計測値を含む詳細版が必要な場合は `format=json&full=1` を使います（`number_of_results` / `timings` / `engines` / `engine_data` / `paging` / `redirect_url` / `error` が追加されます）。

## 互換でない領域

| 領域 | 差分 |
| :--- | :--- |
| **エンジンの網羅率** | **上流 352 エントリ / 226 型をすべて実装済み（100%）**。既定で有効なのは約 170（認証情報・自前インスタンスが必要なものは上流と同じく無効）。内訳は [COVERAGE.md](COVERAGE.md) |
| **UI** | 単一テーマ（simple 相当）のみ。上流のテンプレート・テーマ資産とマークアップは非互換 |
| **i18n** | 57 ロケール（上流は 60） |
| **`/stats`** | 表示形式が上流と異なる（HTML の内容・`/stats/errors` の形） |
| **サブシステム** | プラグイン 11 / 上流 12、favicon は上流の resolver 方式ではない |
| **キャッシュ** | メモリ内（TTL + LRU）と Valkey / Redis の両方に対応 |
| **botdetection** | 信頼プロキシの XFF 解決、IP 制限のスライディング窓、リンクトークン、429 応答 |
| **DB 直結エンジン** | 11 種すべて実装済み（db-engines フィーチャー、既定で有効） |

## 検証の方法

- 構文・統合ロジック・JSON/CSV の形: `cargo test`（ユニット 750 + 統合 14）
- 上流との実測比較（速度・結果一致度）: [BENCHMARK.md](BENCHMARK.md) と `bench/compare.py`
- エンジン網羅率: `python3 tools/coverage.py /path/to/searxng`

## 完全互換に近づけるには

1. ~~残りエンジン型の移植~~ → **完了（226/226）**。DB ドライバ直結の 8 種は依存を増やさない方針でスタブ
2. `/stats` と UI テンプレートを上流資産に合わせる
3. i18n のテンプレート全面適用とロケール拡大
4. UI テンプレートのマークアップを上流に合わせる
