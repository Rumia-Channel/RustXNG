# RustXNG

[![CI](https://github.com/sahenjp/RustXNG/actions/workflows/ci.yml/badge.svg)](https://github.com/sahenjp/RustXNG/actions/workflows/ci.yml)
[![License: AGPL-3.0-or-later](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](Cargo.toml)

SearXNG（プライバシー重視のメタ検索エンジン）の振る舞いを Rust に移植した派生プロジェクトです。検索構文・結果統合ロジック・API パラメータは上流に合わせ、単一バイナリで動く軽量な検索基盤としてセルフホストできます。ライセンスは上流と同じ **AGPL-3.0-or-later**。

## 特長

- **上流互換の検索構文** — `!engine` / `!category` / `:lang` / `<timeout` / `!!` / `!!bang`（外部 bang 13,561 件）
- **同じ集計ロジック** — 重複排除・スコア計算・カテゴリ別グループ化は `searx/results.py` と同一の式
- **エンジン 113 設定 / 71 型** — 宣言的エンジン（`xpath` / `json_engine` / `mediawiki`）で上流設定をそのまま取込
- **API 一式** — HTML / JSON / CSV / RSS、`/autocomplete`、`/config`、`/preferences`、`/stats`、`/metrics`、`/image_proxy`、`/favicon_proxy`、`/opensearch.xml`
- **軽い** — 起動 0.03 秒 / 常駐 13 MiB（上流 SearXNG は 0.72 秒 / 118 MiB、同一マシン実測）

## 動かす

```bash
cargo run --release -- --port 8888
# または
docker build -t rustxng . && docker run --rm -p 8888:8888 rustxng
```

`http://127.0.0.1:8888/` を開くか、そのまま API を叩けます。

```bash
curl 'http://127.0.0.1:8888/search?q=rust&format=json'
curl 'http://127.0.0.1:8888/search?q=rust&time_range=day&format=json'   # 最新情報だけ
curl 'http://127.0.0.1:8888/search?q=%21wikipedia+rust&format=json'      # !wikipedia rust
```

設定は [settings.toml](settings.toml) が既定です。`--config` で差分だけ上書きでき、`SEARXNG_*` 環境変数も受け付けます。詳しくは [docs/PORTING.md](docs/PORTING.md#設定) を参照してください。

## 速度

同一マシンで上流 SearXNG と比較した実測値です（再現手順: [docs/BENCHMARK.md](docs/BENCHMARK.md)）。

| 指標 | SearXNG | RustXNG | 倍率 |
| :--- | ---: | ---: | ---: |
| 起動（`/healthz` が 200 まで） | 0.722 秒 | **0.028 秒** | 約 26 倍高速 |
| 常駐メモリ（RSS） | 118 MiB | **13 MiB** | 約 9 倍小さい |
| エンジン別コールド検索 | 基準 | 0.74〜1.22 | ほぼ同等 |
| 反復クエリ（キャッシュ） | 100〜1000 ms | **1 ms 級** | 100〜1000 倍高速 |
| 結果一致度（overlap@10） | 基準 | 10 エンジン中 6 エンジンが 1.00 | — |

自分の環境で測る場合:

```bash
cargo run --release --example benchmark      # レイテンシとキャッシュ効果
cargo run --release --example benchmark -- --json | jq .   # 詳細を JSON で
```

## ドキュメント

| ドキュメント | 内容 |
| :--- | :--- |
| [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) | **SearXNG との互換範囲と差分**（「完全互換か」の答えはここ） |
| [docs/COVERAGE.md](docs/COVERAGE.md) | エンジン移植の現状（113/355 エントリ、71/227 型） |
| [docs/PORTING.md](docs/PORTING.md) | 上流ファイルとの対応表、意図的な逸脱 |
| [docs/BENCHMARK.md](docs/BENCHMARK.md) | 上流との速度・精度の実測と再現手順 |

## 開発

```bash
cargo fmt --all --check
cargo clippy --all-targets
cargo test          # ユニット 281 + 統合 14（外部ネットワーク不要）
cargo run --release --example benchmark -- --repeats 2 --engines wikipedia,hackernews
```

エンジンの追加手順は [CONTRIBUTING.md](CONTRIBUTING.md)、脆弱性の報告は [SECURITY.md](SECURITY.md) を参照してください。CI はフォーマット・全テスト・ベンチマーク example のビルドを検証します。

## ライセンス

**AGPL-3.0-or-later**（[LICENSE](LICENSE)）。SearXNG をはじめとする上流の成果物に基づく派生作品で、帰属は [NOTICE](NOTICE) に記載しています。AGPL 第 13 条により、改変版をネットワークサービスとして公開する場合は対応するソースコードの提供が必要です。
