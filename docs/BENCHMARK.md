# ベンチマーク

RustXNG と上流 SearXNG（Python）を同じマシンで並べて測った結果です。

## 方法

- 2 つのインスタンスを同じホストで同時に起動（RustXNG: `cargo run --release`、SearXNG: granian + WSGI）
- 同じクエリを同じ順序で送信し、JSON API の応答時間と結果を比較
- コールド（初回）とウォーム（キャッシュ・サスペンド後）を分けて記録
- 再現: `cargo run --release --example benchmark` / `python3 bench/compare.py`

## レイテンシ

169 エンジンを既定で有効にした状態で、5 クエリ × 3 回の測定です。

| 指標 | 値 |
| --- | ---: |
| 平均 | 774 ms |
| 中央値（ウォーム） | **276 ms** |
| p95（初回クエリ） | 3,011 ms |
| キャッシュ効果 | 初回 1,227 ms → 2 回目以降 547 ms（-55%） |

- 定常状態では 270 ms 前後で、46 エンジンが応答しています
- 初回が遅いのは、応答しないエンジンが `search.timeout_limit`（3 秒）まで待つためです。2 回目以降は自動サスペンドでスキップされます

## 改善の履歴

| 変更 | 効果 |
| --- | ---: |
| タイムアウトを再試行しない（上流も接続エラーのみ再試行） | 中央値 6,259 ms → 3,003 ms |
| 再試行を含めた全体デッドラインを守る | p95 のばらつきを抑制 |
| エンジンのサスペンドをクエリ間で維持 | ウォーム時 276 ms |

## 起動とメモリ

| 指標 | SearXNG | RustXNG |
| --- | ---: | ---: |
| 起動（`/healthz` が 200 を返すまで） | 0.72 秒 | **0.03 秒** |
| 常駐メモリ（RSS） | 118 MiB | **13 MiB** |
| バイナリサイズ | venv 151 MB | **17 MB**（DB エンジン込みで 27 MB） |

## エンジンごとの応答時間

```bash
cargo run --release --example benchmark -- --repeats 3
```

出力の `per engine` 表に、エンジンごとの中央値と最大値が出ます。多くのエンジンは 0〜300 ms、遅いものでも 1〜2 秒です。

## 結果の一致度

上流と RustXNG で同じクエリを投げ、上位 10 件の URL の一致率（overlap@10）を測ります。

| エンジン | overlap@10 |
| --- | ---: |
| hackernews / github / crates.io / deezer / photon | 1.00 |
| dailymotion / bing news | 0.90 |
| google / bing / duckduckgo | 0.70〜0.85（ページネーションや広告枠の差） |

`python3 bench/compare.py --help` で再現できます。
