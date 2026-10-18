# rustxng

[![CI](https://github.com/sahenjp/rustxng/actions/workflows/ci.yml/badge.svg)](https://github.com/sahenjp/rustxng/actions/workflows/ci.yml)
[![License: AGPL-3.0-or-later](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](Cargo.toml)

rustxng は、プライバシー重視のメタ検索エンジン [SearXNG](https://github.com/searxng/searxng) の振る舞いを Rust に移植した派生作品です。上流の検索構文・結果の統合ロジック・設定形式・API を保ちながら、単一バイナリで動く軽量な検索基盤としてセルフホストできます。ライセンスは上流と同じ **AGPL-3.0-or-later** で、帰属は [NOTICE](NOTICE) に記載しています。

> 移植の範囲と残課題は [docs/PORTING.md](docs/PORTING.md) と [docs/COVERAGE.md](docs/COVERAGE.md)、上流 SearXNG との実測比較は [docs/BENCHMARK.md](docs/BENCHMARK.md) にまとめています。

## 主な特徴

- **検索構文の互換性**: `!engine` / `!shortcut` / `!category` / `:lang` / `<timeout` / `!!`（先頭結果へ）/ `!!bang`（外部サービスへ委譲）
- **上流準拠の集計ロジック**: 結果の重複排除（トラッキングパラメータ除去込み）・スコアリング・カテゴリ別グループ化は上流 `searx/results.py` と同じ計算式
- **エンジン**: 113 設定エントリ / 71 エンジン型。宣言的エンジン（`xpath` / `json_engine` / `mediawiki`）を実装し、上流 `settings.yml` の定義を `tools/import_engines.py` でそのまま取り込めます
- **外部 bang**: 13,561 件（上流の圧縮トライを `tools/import_external_bangs.py` で平坦化）
- **出力とエンドポイント**: HTML / JSON / CSV / RSS、`/autocomplete`、`/config`、`/preferences`、`/stats`、`/metrics`（OpenMetrics）、`/image_proxy`、`/favicon_proxy`、`/opensearch.xml`、`/healthz`
- **サブシステム**: プラグイン 11 種、アンサーラー（statistics / random / weather）、Wikidata インフォボックス、i18n 11 ロケール
- **運用まわり**: TTL+LRU キャッシュ、エンジン単位のサスペンド（`suspended_times` 準拠）、簡易レート制限、HMAC 署名つき画像/favicon プロキシ（SSRF ガード）
- **最新情報**: news カテゴリ（Google News / Bing News / Hacker News）と UI の時間範囲セレクタ。時間範囲は各エンジンのネイティブな日付フィルタへ変換されます

## クイックスタート

### Cargo で実行

```bash
git clone https://github.com/sahenjp/rustxng.git
cd rustxng
cargo run --release -- --port 8888 --bind 127.0.0.1
```

起動後 <http://127.0.0.1:8888/> を開きます。API はそのまま叩けます。

```bash
curl 'http://127.0.0.1:8888/search?q=rust&format=json'
curl 'http://127.0.0.1:8888/search?q=rust&time_range=day&format=json'   # 最新情報だけ
curl 'http://127.0.0.1:8888/search?q=%21wikipedia+rust&format=json'      # !wikipedia rust
```

### Docker で実行

```bash
docker build -t rustxng .
docker run --rm -p 8888:8888 rustxng
```

### 設定

同梱の [settings.toml](settings.toml) が既定値です。上書きしたい項目だけを書いた TOML を `--config` で渡すと、既定値にマージされます（エンジンは name 単位。`use_default_settings = false` で置き換え）。`SEARXNG_PORT` / `SEARXNG_BIND_ADDRESS` / `SEARXNG_SECRET` などの環境変数も上流と同じ名前で受け付けます。

```toml
[server]
port = 8888
bind_address = "127.0.0.1"
image_proxy = true      # /image_proxy を有効化（secret_key で署名）

[search]
safe_search = 1
default_lang = "ja"

[[engines]]
name = "wikipedia (ja)"
engine = "wikipedia"
shortcut = "wja"
categories = ["general"]
base_url = "https://ja.wikipedia.org"
```

## アーキテクチャ

上流 SearXNG のコンポーネントと本リポジトリの実装の対応です。

| SearXNG | rustxng | 役割 |
| :--- | :--- | :--- |
| `searx/query.py` | [src/query.rs](src/query.rs) | 検索構文の解析（bang / 言語 / タイムアウト） |
| `searx/results.py` | [src/results.rs](src/results.rs) | 重複排除・スコア計算・カテゴリ別グループ化 |
| `searx/search/` | [src/aggregator.rs](src/aggregator.rs) | エンジン選択・並列実行・マージ |
| `searx/engines/` | [src/engine/](src/engine/), [src/engines/](src/engines/) | エンジンフレームワークと各エンジン |
| `searx/settings_defaults.py` | [src/settings.rs](src/settings.rs) | 設定の読み込みとマージ・検証 |
| `searx/network/` | [src/network.rs](src/network.rs) | HTTP クライアント（UA ローテーション・リトライ・キャッシュ） |
| `searx/plugins/` | [src/plugins/](src/plugins/) | プラグイン 11 種 |
| `searx/answerers/` | [src/answerers/](src/answerers/) | 即時回答（統計・乱数・天気） |
| `searx/webapp.py` | [src/web/](src/web/) | HTTP ルート・テンプレート |

## エンジンカバレッジ

上流 `settings.yml` の 355 エントリ / 227 型に対する現状です。`python3 tools/coverage.py /path/to/searxng` で再計測できます。

| 指標 | 値 |
| :--- | ---: |
| 設定エントリ | 113 / 355 |
| エンジン型（実装） | 71 / 227 |
| 上流エントリのうち型を実装済み | 174 / 355 |
| 外部 bang | 13,561 |

未実装の内訳（API キー必須・自前インスタンス・torrent 系（方針として対象外）・特殊 DB バックエンド・専用スクレイパー）は [docs/COVERAGE.md](docs/COVERAGE.md) を参照してください。

## ベンチマーク

同一マシンで上流 SearXNG と比較した実測値です（詳細と再現手順は [docs/BENCHMARK.md](docs/BENCHMARK.md)）。

| 指標 | SearXNG | rustxng |
| :--- | ---: | ---: |
| 起動（`/healthz` が 200 を返すまで） | 0.722 秒 | **0.028 秒** |
| 常駐メモリ（RSS） | 118 MiB | **13 MiB** |
| エンジン別コールド レイテンシ | 基準 | 同等（A/B 0.74〜1.22） |
| 反復クエリ（キャッシュ） | 100〜1000 ms | **1 ms 級** |
| 結果一致度（overlap@10） | 基準 | 10 エンジン中 6 エンジンで 1.00 |

## 開発

```bash
cargo fmt --all --check
cargo clippy --all-targets
cargo test                       # ユニット 281 + 統合 14（外部ネットワーク不要）
cargo run --release --example benchmark -- --repeats 2 --engines wikipedia,hackernews
python3 bench/compare.py --help  # 参照 SearXNG との速度・一致度比較
```

CI（GitHub Actions）はフォーマット・全テスト・ベンチマーク example のビルドを検証します。開発の進め方は [CONTRIBUTING.md](CONTRIBUTING.md)、脆弱性の報告は [SECURITY.md](SECURITY.md) を参照してください。

## ライセンス

本プロジェクトは **AGPL-3.0-or-later**（[LICENSE](LICENSE)）です。SearXNG をはじめとする上流の成果物に基づく派生作品であり、著作権表示と帰属は [NOTICE](NOTICE) に記載しています。AGPL 第 13 条により、改変版をネットワークサービスとして公開する場合は、利用者に対応するソースコードを提供する必要があります。
