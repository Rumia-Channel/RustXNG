# RustXNG

[![CI](https://github.com/sahenjp/RustXNG/actions/workflows/ci.yml/badge.svg)](https://github.com/sahenjp/RustXNG/actions/workflows/ci.yml)
[![License: AGPL-3.0-or-later](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](Cargo.toml)
[![Release](https://img.shields.io/github/v/release/sahenjp/RustXNG?sort=semver)](https://github.com/sahenjp/RustXNG/releases/latest)

Google・Bing・Wikipedia・GitHub など **200 以上の検索サイトをまとめて検索できる、自分でホストする検索エンジン**です。検索語は各サイトへ直接送られ、広告や追跡のない結果だけが表示されます。Python 製の [SearXNG](https://github.com/searxng/searxng) を Rust で書き直したもので、同じ使い勝手のまま起動が速く、メモリ消費が小さいのが特徴です。

配布物は単一のバイナリです。Python もデータベースも不要で、設定ファイル 1 枚で動きます。

## できること

- **まとめ検索** — 一般・ニュース・画像・動画・音楽・書籍・論文・パッケージ・地図など、200 以上のサイトを横断
- **結果の統合** — 重複を除き、関連度順に並べ替え、落ちているサイトがあっても残りで結果を返す
- **いろいろな出力** — ブラウザ用の画面のほか、JSON / CSV / RSS でも取得できる
- **プライバシー** — 画像や favicon はプロキシ経由で表示し、利用者の IP を相手サイトに見せない
- **運用向けの機能** — 検索候補、設定画面、統計画面、ヘルスチェック、メトリクス出力、Valkey / Redis 共有キャッシュ、bot 対策のレート制限
- **自分のデータも検索** — PostgreSQL / MySQL / MariaDB / MongoDB / SQLite / Valkey / Elasticsearch / Solr / Meilisearch を検索対象にできる

## 使い方

### バイナリを入手する

リリースページに、タグとターゲット名を含む名前でビルド済みバイナリを置いています。

| プラットフォーム | 資産名の例 | 備考 |
| :--- | :--- | :--- |
| Linux x86_64（glibc） | `rustxng-v0.4.0-x86_64-unknown-linux-gnu` | glibc 2.35 以上 |
| Linux x86_64（静的） | `rustxng-v0.4.0-x86_64-unknown-linux-musl` | Alpine などでも動作 |
| macOS arm64 | `rustxng-v0.4.0-aarch64-apple-darwin` | Apple Silicon |
| macOS x86_64 | `rustxng-v0.4.0-x86_64-apple-darwin` | Intel Mac |
| Windows x86_64 | `rustxng-v0.4.0-x86_64-pc-windows-msvc.exe` | `rustxng.exe` として保存 |

各資産には `.sha256` を併置しています。取得後は次のように確認してから配置してください。

```bash
gh release download --repo sahenjp/RustXNG --pattern 'rustxng-*-x86_64-unknown-linux-gnu*'
sha256sum -c rustxng-v0.4.0-x86_64-unknown-linux-gnu.sha256
install -m755 rustxng-v0.4.0-x86_64-unknown-linux-gnu ~/.local/bin/rustxng
```

`rdsh rustxng install`（rustdsh 同梱のコマンド）は、この命名規則で最新リリースの資産を選んで自動で導入します。

### 起動する

```bash
cargo run --release -- --port 8888
```

<http://127.0.0.1:8888/> を開くと検索できます。Docker を使う場合:

```bash
docker build -t rustxng .
docker run --rm -p 8888:8888 rustxng
```

### プログラムから使う

```bash
# JSON で結果を取得
curl 'http://127.0.0.1:8888/search?q=rust&format=json'

# 1 日以内に公開された情報だけ
curl 'http://127.0.0.1:8888/search?q=rust&time_range=day&format=json'
```

検索語の先頭に `!サイト名` を付けると、そのサイトだけを検索できます。

```bash
curl 'http://127.0.0.1:8888/search?q=%21wikipedia+rust&format=json'   # Wikipedia のみ
curl 'http://127.0.0.1:8888/search?q=%21github+tokio&format=json'      # GitHub のみ
```

## 対応している検索サイト

SearXNG が標準で用意している **226 種類すべて**を移植済みです（352 の設定エントリ）。

| 分類 | 例 |
| :--- | :--- |
| ウェブ検索 | Google, Bing, DuckDuckGo, Brave, Yahoo, Yandex, Qwant, Startpage |
| ニュース | Google News, Bing News, Reuters, Yahoo News, 地域ニュース各社 |
| 画像・動画・音楽 | Google Images, YouTube, Vimeo, Bilibili, Spotify, SoundCloud |
| 学術・論文 | arXiv, PubMed, Crossref, OpenAlex, Semantic Scholar, Springer |
| 開発・パッケージ | GitHub, GitLab, npm, PyPI, crates.io, Docker Hub, Hex, Go |
| 百科事典・辞書 | Wikipedia, Wikidata, Wiktionary, 各種辞書・翻訳 |
| その他 | 天気, 通貨換算, 地図, torrent, ファイル共有など |

既定で有効なのは約 170 サイトです。API キーや自前サーバーが必要なサイトは無効にしてあるので、[settings.toml](settings.toml) で有効化してください。全リストは [docs/COVERAGE.md](docs/COVERAGE.md)。

## 速さ

同じマシンで SearXNG（Python 版）と比較した実測値です。詳しい条件と再現手順は [docs/BENCHMARK.md](docs/BENCHMARK.md)。

| 指標 | SearXNG | RustXNG |
| :--- | ---: | ---: |
| 起動時間 | 0.72 秒 | **0.03 秒** |
| メモリ使用量 | 118 MiB | **13 MiB** |
| 既定エンジン全件（169）の検索 | 1〜3 秒 | **0.27 秒**（ウォーム時・中央値） |
| 単一サイトの検索 | 100〜400 ms | **1〜50 ms** |
| 検索結果の一致度 | — | hackernews / arxiv は完全一致、bing は 88% |

自分の環境で測る場合:

```bash
cargo run --release --example benchmark
```

## 設定

同梱の [settings.toml](settings.toml) が既定値です。変えたい項目だけを書いた TOML を `--config` で渡すと、既定値に上書きマージされます。

```toml
[server]
port = 8888
image_proxy = true

[search]
safe_search = 1
default_lang = "ja"
```

SearXNG を使っていた場合は、その `settings.yml`（YAML）をそのまま `--config` に指定できます。環境変数も SearXNG と同じ名前（`SEARXNG_PORT` など）を受け付けます。詳細は [docs/PORTING.md](docs/PORTING.md)。

## SearXNG との違い

検索の構文・パラメータ名・API の返し方・設定の考え方は SearXNG に合わせています。違うのは次の点です。

- 画面のデザインは独自（SearXNG のテーマは移植していません）
- 画面の翻訳は 57 言語（SearXNG は 60 言語以上）。検索する言語は 110 以上から選べます
- プラグインは 11 / 12 種

互換性の詳細は [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md)。

## 開発

```bash
cargo fmt --all --check
cargo clippy --all-targets
cargo test          # 735 件、外部ネットワーク不要
```

エンジンの追加方法は [CONTRIBUTING.md](CONTRIBUTING.md)、脆弱性の報告は [SECURITY.md](SECURITY.md) を参照してください。CI は Linux・macOS・Windows でフォーマット・clippy・テストを回し、`--no-default-features` の軽量ビルドと Docker イメージの起動まで検証します。

## ドキュメント

| ドキュメント | 内容 |
| :--- | :--- |
| [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) | SearXNG とどこが同じでどこが違うか |
| [docs/COVERAGE.md](docs/COVERAGE.md) | 対応している検索サイトの一覧 |
| [docs/PORTING.md](docs/PORTING.md) | 内部構造と設定の詳細 |
| [docs/BENCHMARK.md](docs/BENCHMARK.md) | 速度の測り方と実測値 |

## ライセンス

**AGPL-3.0-or-later**（[LICENSE](LICENSE)）。SearXNG をはじめとする上流プロジェクトに基づく派生作品で、帰属情報は [NOTICE](NOTICE) に記載しています。AGPL 第 13 条により、改変版をネットワークサービスとして公開する場合は、対応するソースコードを提供する必要があります。
