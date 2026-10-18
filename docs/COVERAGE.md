# 全エンジン移植の状況（カバレッジ）

SearXNG の `settings.yml` には **355 エントリ / 227 エンジン型**があります。本移植の現状は `tools/coverage.py` でいつでも再計測できます。

```bash
python3 tools/coverage.py /path/to/searxng settings.toml
```

## 現在の数値

| 指標 | 値 |
|---|---|
| 上流のエンジンエントリ | 355 |
| 上流のエンジン型 | 227 |
| 本移植の設定エントリ | 113 |
| 本移植のエンジン型 | 72 |
| 上流エントリのうち型を実装済み | 174（355 中） |

## 移植を支える仕組み

1. **宣言的エンジン**（`engine: xpath` / `json_engine` / `mediawiki`）を実装し、上流の `settings.yml` の定義をそのまま取り込めるようにしました。
   - `tools/import_engines.py` が上流設定から 44 エンジンを生成（xpath 24 / json_engine 8 / mediawiki 12）。
   - XPath は上流が使う構文のサブセット（`//tag`、`[@class="x"]`、`contains()`、`[1]`、`[last()]`、`(path)[last()]`、`/@attr`、`text()`、`concat()`、`substring-before/after`）を実装。
   - JSON パスは上流の `results_query` 記法（`/` 区切り、`$` で配列を降りる）に対応。
2. **コードエンジン**の追加移植（1 エンジン 1 ファイル、`#[cfg(test)]` のフィクスチャテスト付き）。
3. **外部 bang の全量移植**: 上流の圧縮トライ（13,561 件）を `tools/import_external_bangs.py` で平坦化し、完全な検索・補完を実装。

## まだ移植していないもの

`tools/coverage.py` の出力にある残りの型は、次のいずれかに分類できます。

| 分類 | 例 |
|---|---|
| API キー / 認証情報が必要 | google_cse, braveapi, kagi, exaapi, wolframalpha_api, flickr, pexels, pixabay, unsplash, spotify |
| 自前インスタンスの指定が必要 | lemmy, mastodon, invidious, piped, peertube, sepiasearch（※コードは移植済み。設定でインスタンスを指定して使う） |
| 海賊版・ファイル共有（方針として対象外） | 1337x, piratebay, nyaa, kickass, btdigg, solidtorrents ほか |
| データベース / 特殊バックエンドが必要 | elasticsearch, meilisearch, solr, postgresql, mysql_server, mongodb, recoll, sqlite |
| 天気・通貨・翻訳などのアンサー系 | open_meteo, currency_convert, deepl, libretranslate |
| その他の専用スクレイパー | 残りの HTML/JSON エンジン（1 件ずつ移植が必要） |

## サブシステム（SearXNG のエンジン以外）

| 上流 | 状態 |
|---|---|
| プラグイン 12 種 | 7 移植済み（hostnames / tracker_url_remover / time_zone / calculator / hash / oa_doi / self_info）。残り（unit_converter, ahmia_filter, tor_check, infinite_scroll）は移植作業中 |
| アンサーラー 3 種 + weather.py | 作業中 |
| favicons | 作業中 |
| i18n（61 ロケール） | `src/i18n.rs` を作業中（カタログ生成スクリプト付き） |
| wikidata インフォボックス | `src/wikidata_infobox.rs` を作業中 |
| valkey / redis キャッシュ | 未移植（メモリ内キャッシュで代替） |
| botdetection / limiter.toml | 部分（簡易レート制限のみ） |
