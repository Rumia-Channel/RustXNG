# rustxng

SearXNG を Rust で書き直したメタ検索エンジン（metasearch engine）です。ライセンスは上流と同じ **AGPL-3.0-or-later** で、帰属表示は [NOTICE](NOTICE)、移植の対応状況は [docs/PORTING.md](docs/PORTING.md) と [docs/COVERAGE.md](docs/COVERAGE.md)、速度・精度の比較は [docs/BENCHMARK.md](docs/BENCHMARK.md) にまとめています。

SearXNG（<https://github.com/searxng/searxng>）は AGPL-3.0-or-later で公開されているメタ検索エンジンです。本プロジェクトはこの振る舞い・設定形式・API・エンジン定義を Rust に移植した派生作品（derivative work）なので、同じライセンスを継承します。

## できること

- 108 エントリ / 66 種類のエンジンをカテゴリ（general / images / videos / news / map / music / it / science / files / social media）で横断検索
- エンジンの並列実行、重複排除、スコアリング（上流 `searx/results.py` と同じ計算式とグループ化）
- 検索構文: `!engine`（エンジン指定）、`!category`（カテゴリ指定）、`:lang`（言語指定）、`<3`（タイムアウト指定）、`!!`（先頭結果へリダイレクト）、`!!bang`（外部サービスへ委譲）
- **最新情報**: news カテゴリ（google news / bing news / hackernews）と、UI の時間範囲セレクタ（過去 1 日 / 1 週間 / 1 か月 / 1 年）。時間範囲は対応エンジンへネイティブに伝わります（Google News の `when:1d`、Bing の `qft`、Crossref の `from-pub-date`、GitHub の `pushed:>`、arXiv の `sortBy=submittedDate`、Stack Exchange の `fromdate`、Hacker News の `numericFilters`）
- 出力形式: HTML、JSON、CSV、RSS。`/autocompleter`、`/config`、`/stats`、`/stats/errors`、`/metrics`、`/preferences`、`/image_proxy`、`/opensearch.xml`、`/healthz`
- Cookie ベースのユーザー設定（言語、ロケール、セーフサーチ、時間範囲、カテゴリ、エンジン、プラグイン、画像プロキシ）
- プラグイン: `hostnames`、`tracker_url_remover`、`time_zone`、`calculator`、`hash_plugin`、`oa_doi_rewrite`（既定無効）、`self_info`（既定無効）
- 応答キャッシュ（TTL + LRU）、エンジンごとのタイムアウト・サスペンド（停止期間つき）・統計
- OpenMetrics / Prometheus 出力（`/metrics`）と、HMAC 署名つき画像プロキシ（`/image_proxy`、私設アドレスは拒否）
- 簡易レート制限（`server.limiter = true` で有効。上流の botdetection とは別物、[docs/PORTING.md](docs/PORTING.md) 参照）
- 上流と同じ `settings.yml` 風の設定と環境変数（`SEARXNG_*`）

## クイックスタート

```bash
# 開発時
cargo run -- --port 8888 --bind 127.0.0.1

# 配布用バイナリ
cargo build --release
./target/release/rustxng --port 8888
```

起動後、<http://127.0.0.1:8888/> を開くか、API を直接叩きます。

```bash
curl 'http://127.0.0.1:8888/search?q=rust&format=json'
curl 'http://127.0.0.1:8888/search?q=rust&time_range=day&format=json'   # 最新情報だけ
curl 'http://127.0.0.1:8888/search?q=%21wikipedia+rust&format=json'      # !wikipedia rust
curl 'http://127.0.0.1:8888/autocompleter?q=rust'
```

### オプション

| オプション | 説明 |
|---|---|
| `-c, --config <FILE>` | 設定ファイル（既定: `$SEARXNG_SETTINGS_PATH`） |
| `-b, --bind <ADDRESS>` | 待ち受けアドレス（既定: `server.bind_address`） |
| `-p, --port <PORT>` | 待ち受けポート（既定: `server.port`） |
| `-v, --version` / `-h, --help` | バージョン / ヘルプ |

環境変数: `SEARXNG_SETTINGS_PATH`、`SEARXNG_BIND_ADDRESS`、`SEARXNG_PORT`、`SEARXNG_SECRET`、`SEARXNG_BASE_URL`、`SEARXNG_DEBUG`、`SEARXNG_ENABLE_METRICS`、`RUST_LOG`。

## 設定

同梱の [settings.toml](settings.toml) が既定値です。上書きしたい項目だけを書いた TOML を `--config` で渡すと、既定値にマージされます（エンジンは **name 単位** でマージ。`use_default_settings = false` を指定するとエンジン一覧を置き換えます）。

```toml
use_default_settings = true

[search]
safe_search = 1
default_lang = "ja"

[server]
image_proxy = true      # /image_proxy を有効にする（secret_key で署名）
limiter = true          # 簡易レート制限（60 秒に 30 リクエスト）
limiter_window = 60
limiter_max = 30

[[engines]]
name = "mojeek"
engine = "mojeek"
disabled = true          # 既定のエンジンを無効化する

[[engines]]
name = "wikipedia (ja)"
engine = "wikipedia"
shortcut = "wja"
categories = ["general"]
base_url = "https://ja.wikipedia.org"

[[engines]]
name = "hacker news (newest)"
engine = "hackernews"
shortcut = "hnnew"
categories = ["news"]
search_by_date = true    # Algolia の日付順インデックスを使う
```

主な設定項目:

- `server`: `bind_address`、`port`、`base_url`、`method`（GET/POST）、`secret_key`、`image_proxy`、`limiter` / `limiter_window` / `limiter_max`
- `search`: `safe_search`、`autocomplete`、`default_lang`、`languages`、`timeout_limit`、`formats`、`max_page`、`suspended_times`
- `outgoing`: `request_timeout`、`max_request_timeout`、`retries`、`proxies`、`useragent_suffix`、`verify`
- `plugins`: `enabled` / `disabled`

### 最新情報を集めるコツ

- カテゴリ `news` のタブ、または `!news` / `!gnews` / `!bnews` / `!hn` を使う
- `time_range`（UI の時間範囲セレクタ）を `day` にする。対応エンジンは上記のとおりネイティブな日付フィルタを使います
- 日付順で読みたい場合は `search_by_date = true` を付けた Hacker News エンジンを追加する

## HTTP API

| エンドポイント | 説明 |
|---|---|
| `GET /` | 検索フォーム（`q` 付きなら検索結果） |
| `GET/POST /search` | 検索本体。`format=html` / `json` / `csv` / `rss` |
| `GET /autocompleter` | サジェスト（`backend=duckduckgo,google,wikipedia,mwmbl,none`） |
| `GET /config` | エンジン・カテゴリ・プラグイン・ロケール一覧 |
| `GET/POST /preferences` | ユーザー設定の表示と保存（Cookie） |
| `GET /stats`、`/stats/errors` | 統計（メモリ上のカウンタとエンジン状態） |
| `GET /metrics` | OpenMetrics / Prometheus 形式（`general.enable_metrics` で無効化可） |
| `GET /image_proxy` | 署名つき画像プロキシ（`url` と `h` が必要） |
| `GET /healthz` | ヘルスチェック |
| `GET /opensearch.xml` | ブラウザ検索プラグイン |
| `GET /about`、`/robots.txt`、`/static/style.css` | 補助ページ |

`/search` のパラメータは上流と同じです: `q`、`category`/`categories`、`engine`/`engines`、`language`、`locale`、`pageno`、`safesearch`、`time_range`、`timeout_limit`、`format`、`enabled_plugins`、`disabled_plugins`、`disabled_engines`。

## 検索構文

| 構文 | 例 | 意味 |
|---|---|---|
| `!engine` / `!shortcut` | `!wikipedia rust`、`!wp rust` | 指定エンジンだけを使う |
| `!category` | `!images cats` | カテゴリ内の全エンジンを使う |
| `:lang` | `:ja 猫`、`:english cat` | 言語を指定 |
| `<n` | `rust <3` | タイムアウト（100 未満は秒、100 以上はミリ秒） |
| `!!` | `!! rust` | 先頭の結果へリダイレクト |
| `!!bang` | `!!yt cats` | 外部サービスへ委譲 |

## 実装済みエンジン

| カテゴリ | エンジン |
|---|---|
| general | duckduckgo, google, bing, brave, mojeek, mwmbl, wikipedia, wikidata, openlibrary |
| news | google news, bing news, hackernews |
| it | hackernews, github, stack exchange, crates.io, npm, pypi, docker hub |
| science | arxiv, crossref, semantic scholar |
| map | openstreetmap, photon |
| videos | youtube, dailymotion |
| music | deezer |
| images | openverse, wikicommons |

上流の全エンジン（253 モジュール）のうち、API キー不要で動くものを優先して移植しています。未移植・部分移植の一覧は [docs/PORTING.md](docs/PORTING.md) を参照してください。

## ベンチマークと比較

```bash
# 自分の環境でレイテンシを測る（アプリを内蔵起動して HTTP で叩く）
cargo run --release --example benchmark -- --repeats 2 --engines wikipedia,hackernews

# 最新情報（news + 時間範囲）を測る
cargo run --release --example benchmark -- --time-range week \
    --engines 'google news,bing news,hackernews' --queries 'rust release,ai research'

# 参照 SearXNG インスタンスと速度・結果一致度を比べる
python3 bench/compare.py --a http://127.0.0.1:8899 --b http://127.0.0.1:8001 \
    --engines 'hackernews,github,npm,deezer' --repeats 2 --json bench/results.json

# 起動時間とメモリ
bash bench/resources.sh http://127.0.0.1:8899 rustxng rustxng
```

実測値と測定方法、注意点は [docs/BENCHMARK.md](docs/BENCHMARK.md) にまとめています。要点だけ:

| 観点 | 結果（同じマシン・同条件の実測） |
|---|---|
| 起動時間 | rustxng 0.028 秒 / SearXNG 0.722 秒（約 26 倍） |
| 常駐メモリ | 13 MiB / 118 MiB（約 9 倍小さい） |
| エンジン別コールド レイテンシ | ほぼ同等（8 エンジンで A/B 0.74〜1.22） |
| キャッシュが効く反復クエリ | 1 ms 級（SearXNG は 100〜1000 ms） |
| 結果の一致度 | 10 エンジン中 6 エンジンが overlap@10 = 1.00、2 エンジンが 0.90、npm のみ 0.01（索引差） |

## テストと検証

```bash
cargo test          # ユニットテスト + HTTP 統合テスト（ローカルの擬似エンジンを使う）
cargo clippy --all-targets
cargo fmt --all
```

統合テストは外部ネットワークに接続しません。擬似エンジンをローカルで立てて、検索パイプラインと HTML / JSON / CSV / RSS の各出力を検証します。

## ライセンス

- 本プロジェクト: **AGPL-3.0-or-later**（[LICENSE](LICENSE)）
- 上流 SearXNG: AGPL-3.0-or-later、Copyright は SearXNG の作者の方々
- 帰属と派生関係: [NOTICE](NOTICE)

AGPL 第 13 条により、改変版をネットワークサービスとして公開する場合は、利用者に対応するソースコードを提供する必要があります。
