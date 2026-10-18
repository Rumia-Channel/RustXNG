# SearXNG 互換性の範囲

**結論: 「完全互換」とは言えません。** 検索構文・API パラメータ・JSON の基本形は上流に合わせていますが、エンジンの網羅率・サブシステム・UI・設定ファイル形式が一致しないためです。このページは「何が互換で、何が違うか」を検証可能な形でまとめたものです。

## 互換と言える範囲

| 領域 | 状態 | 根拠 |
| :--- | :--- | :--- |
| 検索構文（`!engine` / `!category` / `:lang` / `<timeout` / `!!` / `!!bang`） | **互換** | 上流 `searx/query.py` と同じ判定順・同じ正規化。ユニットテストで検証 |
| 結果の統合（重複排除・スコア・カテゴリ別グループ化） | **互換** | `searx/results.py` の `calculate_score` / `get_ordered_results` と同じ式を移植 |
| `/search` のパラメータ名（`q`, `category`, `engines`, `language`, `pageno`, `safesearch`, `time_range`, `format`, `timeout_limit`, `disabled_engines`, `enabled_plugins` ...） | **互換** | 実装・統合テストで確認 |
| 出力形式（html / json / csv / rss） | **互換** | 同じ `format=` で切り替え |
| JSON の基本形 | **一部互換** | `results` / `answers` / `corrections` / `infoboxes` / `suggestions` / `unresponsive_engines` の形は上流準拠（下記）。ただし追加フィールドあり |
| 設定キー（`general` / `server` / `search` / `ui` / `outgoing` / `engines[]`） | **一部互換** | 名前と意味は同じ。形式は TOML（上流は YAML）、一部の値の既定が違う |
| 環境変数（`SEARXNG_PORT`, `SEARXNG_BIND_ADDRESS`, `SEARXNG_SECRET`, `SEARXNG_SETTINGS_PATH` ...） | **互換** | 同じ名前を受け付け |
| 外部 bang | **データ互換** | 上流の 13,561 件を同一データから生成 |
| OpenMetrics のエンジン指標名 | **互換** | `searxng_engines_*` を使用 |

## JSON API の差分

上流（`searx/webutils.py` の `get_json_response`）と同じ形にした部分:

- `answers`: `[{"answer": "..."}]`
- `unresponsive_engines`: `[["engine", "Translated error"]]`（engine 名でソート、停止中は `"Suspended: ..."`）
- `results[].template`: `"default.html"` / `"images.html"` など
- `results[].metadata`: `"key: value | key: value"` の文字列
- `results[].priority`: 既定は空文字

上流に対して**追加**しているフィールド（上流のクライアントは無視できますが、厳密比較では差分になります）:

- トップレベル: `search_query`, `number_of_results`, `timings`, `engines`, `pageno`, `paging`, `engine_data`, `redirect_url`, `error`
- 結果: `pretty_url`

上流が出力するが本実装が出力しないフィールド: `parsed_url`, `pubdate`, `iframe_src` / `audio_src`（メタデータ側に格納）, `length` / `views`（メタデータ側に格納）。

## 互換でない領域

| 領域 | 差分 |
| :--- | :--- |
| **エンジンの網羅率** | 上流 355 エントリ / 227 型に対し、本実装は 113 エントリ / 71 型（型を実装済みの上流エントリは 174）。未移植の内訳は [COVERAGE.md](COVERAGE.md) |
| **UI** | 単一テーマ（simple 相当）のみ。上流のテンプレート・テーマ資産とマークアップは非互換 |
| **i18n** | 11 ロケール × 41 文字列のみ。上流は 61 ロケールで UI 全体を翻訳 |
| **`/config`** | 返す JSON の構造が異なる（上流は `brand` や `categories_as_tabs` などを含む） |
| **`/stats`** | 表示形式が異なる（HTML の内容・`/stats/errors` の形） |
| **CSV 出力** | 列構成が上流と同一ではない |
| **設定ファイル** | TOML（上流は YAML）。YAML の汎用 `xpath` エンジン定義は `tools/import_engines.py` で取り込む方式 |
| **サブシステム** | プラグイン 11 / 上流 12、アンサーラーは 3 種 + weather、valkey / redis キャッシュ・botdetection・favicon 解決戦略（上流は第三者 resolver を使用）は未移植 |
| **レート制限** | 上流は botdetection（link token / Redis / IP レンジ判定）。本実装はクライアント単位の固定窓のみ |
| **Cookie 名** | 一部異なる（例: カテゴリは `categories`、上流は `category`） |

## 検証の方法

- 構文・統合ロジック・JSON 形: `cargo test`（ユニット 281 + 統合 14）
- 上流との実測比較（速度・結果一致度）: [BENCHMARK.md](BENCHMARK.md) と `bench/compare.py`
- エンジン網羅率: `python3 tools/coverage.py /path/to/searxng`

## 完全互換に近づけるには

1. 残り 156 エンジン型の移植（API キー必須・自前インスタンス・torrent 系は方針で除外）
2. `/config` / `/stats` / CSV を上流の形に合わせる（クライアント互換のため）
3. i18n のテンプレート全面適用とロケール拡大
4. valkey / redis キャッシュ、botdetection、UI テーマ資産の移植
