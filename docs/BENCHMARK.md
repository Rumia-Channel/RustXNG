# 速度・精度の比較（rustxng vs SearXNG）

このドキュメントは、同じマシン上で **rustxng** と **上流 SearXNG** を同時に動かし、同じクエリを投げて比べた記録です。測定に使ったスクリプトは `bench/` に入っているので、同じ手順を再現できます。

> 結論: カバレッジ（同じエンジンで同じ結果）はほぼ一致し、コールド時のレイテンシは同等、起動時間とメモリは大きく有利、キャッシュが効く反復クエリでは桁違いに速い。差が残るのは npm（索引が別）と、上流側のレート制限で欠測した crossref / openstreetmap。

## 測定環境

| 項目 | 値 |
|---|---|
| 日付 | 2026-10-18 |
| マシン | ローカル 1 台（loopback 通信、外部ネットワークは共通） |
| rustxng | 0.1.0、`cargo build --release` |
| SearXNG | master（shallow clone）、granian 2.8.4 + WSGI、JSON API を有効化（`/tmp/sxng-bench-settings.yml`） |
| 比較対象エンジン | hackernews, github, crates.io, npm, deezer, dailymotion, openstreetmap, photon, crossref, bing news |
| クエリ | `bench/queries.txt`（8 件） |
| 測定 | `bench/compare.py`（HTTP 越し、各エンジンへ `engines=<名前>` で指定） |

上流の既定設定では JSON API が無効なので、比較用に有効化しています。レート制限（limiter）は両者とも無効です。

## 1. 起動時間とメモリ

```bash
bash bench/resources.sh rustxng http://127.0.0.1:8899
bash bench/resources.sh granian      http://127.0.0.1:8001
```

| 指標 | rustxng | SearXNG (granian + Flask) |
|---|---|---|
| 起動（`/healthz` が 200 を返すまで） | **0.028 秒** | 0.722 秒 |
| 常駐メモリ（最大プロセスの RSS） | **13 MiB** | 118 MiB |
| スレッド数 | 26（tokio のワーカー + ブロッキングプール） | 5 |
| プロセス数 | 1 | 2（スーパーバイザ + ワーカー） |
| 配布物の大きさ | バイナリ 11.6 MB | venv 151 MB |

起動は約 26 倍、メモリは約 9 倍の差です。Rust 側は単一バイナリで、Python の venv や依存パッケージの導入が不要です。

## 2. エンジン別のコールド レイテンシと一致度

両サーバを再起動してキャッシュを空にしてから、同じクエリを 1 回ずつ投げた結果です（A=rustxng、B=SearXNG、単位 ms）。

| エンジン | A median | B median | A/B | overlap@10 | top1 一致 | A 取得成功 | B 取得成功 |
|---|---|---|---|---|---|---|---|
| hackernews | 351 | 476 | 0.74 | **1.00** | 1.00 | 1.00 | 1.00 |
| github | 962 | 810 | 1.19 | **1.00** | 1.00 | 1.00 | 1.00 |
| crates.io | 695 | 703 | 0.99 | **1.00** | 1.00 | 1.00 | 1.00 |
| npm | 218 | 287 | 0.76 | 0.01 | 0.00 | 1.00 | 0.50 |
| deezer | 288 | 293 | 0.98 | **1.00** | 0.62 | 0.62 | 0.62 |
| dailymotion | 823 | 673 | 1.22 | 0.90 | 0.88 | 1.00 | 1.00 |
| openstreetmap | 12 | 17 | 0.70 | **1.00** | 0.12 | 0.12 | 0.12 |
| photon | 478 | 489 | 0.98 | **1.00** | 0.88 | 0.88 | 0.88 |
| crossref | 292 | 5 | （欠測） | 0.43 | 0.00 | 0.62 | 0.12 |
| bing news | 151 | 157 | 0.96 | 0.90 | 0.88 | 0.88 | 0.88 |

読み方:

- **overlap@10** は、同じエンジン・同じクエリで得た上位 10 件の URL 集合の Jaccard 係数です（1.00 = 完全一致）。
- **top1 一致** は、1 位の URL が同じだったクエリの割合です。
- **取得成功** は、そのエンジンが 1 件以上返したクエリの割合です（API 側のレート制限や一時障害を含みます）。
- **crossref** は測定中に双方で 429 が発生し、A/B の比較が成立しないため欠測扱いにしています。**openstreetmap** も Nominatim のレート制限で 8 クエリ中 1 件しか返らず、こちらは一致度だけ参考値です。

## 3. キャッシュの効果（ウォーム）

同じクエリをもう一度投げたときの中央値です（`bench/results-warm.json`）。

| エンジン | rustxng | SearXNG |
|---|---|---|
| hackernews | 1 ms | 504 ms |
| github | 1 ms | 980 ms |
| crates.io | 1 ms | 257 ms |
| deezer | 1 ms | 274 ms |
| photon | 1 ms | 316 ms |
| bing news | 1 ms | 102 ms |

rustxng は TTL 300 秒・上限 8 MiB の LRU キャッシュを内蔵し、同一リクエストをそのまま返します。SearXNG のキャッシュは Valkey 前提で、この測定環境には Valkey サーバが無いため効いていません（同じ環境条件での比較です。上流も Valkey を用意すれば同等の効果が得られます）。

## 4. 最新情報クエリの実測

news カテゴリと時間範囲（`time_range=week`）を組み合わせた実測です（`examples/benchmark.rs`、アプリを内蔵起動して HTTP で計測）。

```bash
cargo run --release --example benchmark -- \
  --queries 'rust release,latest ai research' \
  --engines 'google news,bing news,hackernews,wikipedia' \
  --time-range week --repeats 2
```

| エンジン | 中央値 | 最大 |
|---|---|---|
| bing news | 176 ms | 198 ms |
| google news | 440 ms | 500 ms |
| hackernews | 380 ms | 678 ms |
| wikipedia | 538 ms | 733 ms |

2 クエリ × 4 エンジンで 84〜99 件が返り、時間範囲は各エンジンのネイティブなフィルタへ変換されています（Google News の `when:7d` 相当、Bing の `qft=interval="7"`、Hacker News の `numericFilters` など）。

## 5. 精度の解釈

- **一致度が 1.00 のエンジン**（hackernews, github, crates.io, deezer, photon）は、同じ API を同じパラメータで叩き、同じ並びになることを意味します。
- **hackernews は当初 0.01 でした。** 原因は上流の Algolia チューニング（`restrictSearchableAttributes` など）と、上流が HN のコメント URL を結果 URL に使う設計を移植していなかったことです。ベンチマークで検出して修正し、現在は 1.00 です。この修正は本比較の成果の 1 つです。
- **npm は 0.01 です。** 上流は npms.io、本実装は npm レジストリの検索 API を使っており、索引そのものが違います（互換性より可用性を優先した意図的な差）。
- **結果件数** は上流に合わせました（github 30 件、npm 25 件、deezer 25 件、openverse 20 件、crates.io 10 件）。上流はエンジンごとにページサイズが異なります。
- **crossref / openstreetmap** の低い一致度は、API 側のレート制限で片側または両側が空になったことによる欠測で、実装差ではありません。

## 6. 再現方法

```bash
# 1. rustxng（release）を起動
cargo build --release
./target/release/rustxng --port 8899

# 2. 参照 SearXNG を用意（venv + granian、JSON 有効化）
python3 -m venv /tmp/sxng-venv
/tmp/sxng-venv/bin/pip install -r requirements.txt -r requirements-server.txt
SEARXNG_SETTINGS_PATH=/tmp/sxng-bench-settings.yml \
  /tmp/sxng-venv/bin/granian --interface wsgi --host 127.0.0.1 --port 8001 searx.webapp:app

# 3. 比較（コールド / ウォーム）
python3 bench/compare.py --a http://127.0.0.1:8899 --b http://127.0.0.1:8001 \
  --engines 'hackernews,github,crates.io,npm,deezer,dailymotion,openstreetmap,photon,crossref,bing news' \
  --queries bench/queries.txt --repeats 1 --json bench/results-cold.json

# 4. 起動時間とメモリ
bash bench/resources.sh rustxng http://127.0.0.1:8899
bash bench/resources.sh granian http://127.0.0.1:8001

# 5. 自分の環境だけを測る（アプリ内蔵）
cargo run --release --example benchmark -- --repeats 2 --engines wikipedia,hackernews
```

## 7. 注意点

- 1 台・同時刻・同一回線の測定で、外部 API の遅延やレート制限、IP の評判に左右されます。値は環境ごとに変わります。
- レイテンシは「外部 API の応答時間 + 実装のオーバーヘッド」です。エンジン API が支配的なため、コールド時の差は小さく出ます。実装差がはっきり出るのは起動・メモリ・キャッシュと、リクエスト数が多い場面です。
- 上流のキャッシュは Valkey が無いと効かないため、ウォーム比較は「既定構成での差」であり、上流の構成次第で変わります。
- 本比較は可用性の高い 10 エンジンに限っています。全 253 エンジンの比較は行っていません。

## 8. 結論

| 観点 | 結果 |
|---|---|
| 起動時間 | rustxng が約 26 倍速い（0.028 秒 vs 0.722 秒） |
| メモリ | rustxng が約 9 倍小さい（13 MiB vs 118 MiB） |
| コールド レイテンシ | 同等（8 エンジンで A/B 0.74〜1.22） |
| ウォーム（キャッシュ） | rustxng が 2〜3 桁速い（1 ms 級） |
| 結果の一致度 | 9 エンジン中 6 エンジンが overlap@10 = 1.00、2 エンジンが 0.90、npm のみ 0.01（索引差） |
| 最新情報 | news エンジンと時間範囲フィルタで 4 エンジン 84〜99 件、176〜538 ms |
