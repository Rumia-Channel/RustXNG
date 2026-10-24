# 全エンジン移植の状況（カバレッジ）

SearXNG の `settings.yml` には **352 エントリ / 226 エンジン型**があります。本移植の現状は `tools/coverage.py` でいつでも再計測できます。

```bash
python3 tools/coverage.py /path/to/searxng settings.toml
```

## 現在の数値

| 指標 | 値 |
|---|---:|
| 上流のエンジンエントリ | 352 |
| 上流のエンジン型 | 226 |
| 本移植のエンジン型 | 198 |
| 上流エントリのうち型を実装済み | 316（352 中） |
| 本移植の設定エントリ | 243 |
| 既定で有効なエントリ | 約 200（上流と同じく、認証情報や自前インスタンスが必要なものは無効） |
| テスト | ユニット 635 + 統合 14 |

## 移植を支える仕組み

1. **宣言的エンジン**（`engine: xpath` / `json_engine` / `mediawiki`）で上流 `settings.yml` の定義をそのまま取り込めます。
   - `tools/import_engines.py` が上流設定からエンジンを生成（xpath / json_engine / mediawiki）。
   - XPath は上流が使う構文のサブセット（`//tag`、`[@class="x"]`、`contains()`、`[1]`、`[last()]`、`(path)[last()]`、`/@attr`、`text()`、`concat()`、`substring-before/after`）を実装。
   - JSON パスは上流の `results_query` 記法（`/` 区切り、`$` で配列を降りる）に対応。
2. **コードエンジン**の追加移植（1 エンジン 1 ファイル、`#[cfg(test)]` のフィクスチャテスト付き、外部ネットワーク不要）。
3. **外部 bang の全量移植**: 上流の圧縮トライ（13,561 件）を `tools/import_external_bangs.py` で平坦化。
4. **登録の自動化**: `tools/register_engines.py` が `// engine-id:` 行を読み、`mod.rs` の宣言・ディスパッチと `settings.toml` のエントリを同期します。

## エンジン型の内訳

| 分類 | 数 | 状態 |
|---|---:|---|
| 実装済み（スクレイパー / API） | 198 | 既定で有効、または認証情報を設定すれば有効 |
| 認証情報・自前インスタンスが必要 | — | 実装済みだが `disabled = true`（上流と同じ扱い） |
| データベース直結（未移植） | 11 | elasticsearch / meilisearch / solr / postgresql / mysql_server / mariadb_server / mongodb / sqlite / valkey_server / recoll / opensemantic |
| アシスタント型・デモ用（対象外） | 8 | `__init__` などのヘルパーモジュール、`dummy` / `demo_*` デモエンジン |
| その他 | 9 | 移植中または上流でも特殊（後述） |

### データベース直結エンジンについて

`elasticsearch` / `solr` / `meilisearch` / `postgresql` などは、SearXNG が DB ドライバを直接使ってインデックスを検索するエンジンです。Rust 版では DB クライアントの依存を増やさない方針のため対象外とし、代わりに `tools/import_engines.py` で生成した宣言的エンジン（HTTP 経由）を使えます。

### 実装済みでも既定で無効なもの

上流と同じく、次は認証情報や自前インスタンスの設定が必要なため `disabled = true` にしています。

- API キー: `braveapi`, `google_cse`, `kagi`, `exaapi`, `wolframalpha_api`, `springer`, `flickr`, `youtube_api`, `giphy`, `soundcloud`, `marginalia`, `jina`, `ollama` など
- 自前インスタンス: `lemmy`, `mastodon`, `invidious`, `piped`, `peertube`, `sepiasearch`, `discourse`, `torznab`, `annas_archive`, `searx 系`
- 翻訳系: `deepl`, `libretranslate`, `lingva`, `mozhi`, `translated`

## 移植時の意図的な差分

- **2 リクエスト必要なエンジン**（PubMed の esearch→efetch、Open-Meteo のジオコーディング、Ahmia のトークン取得など）は、単一リクエストで取得できる代替エンドポイントを使うか、トークン等を設定値から受け取ります。各ファイル冒頭に明記しています。
- **JavaScript 実行が必要なページ**は対象外、または JSON エンドポイントに切り替えています。
- **アシスタント型エンジン**（`calculator` など）はプラグイン / アンサーラーとして実装しています。

## サブシステム（エンジン以外）

| 上流 | 状態 |
|---|---|
| プラグイン 12 種 | 11 移植済み（hostnames / tracker_url_remover / time_zone / calculator / hash / oa_doi / self_info / unit_converter / ahmia_filter / tor_check / infinite_scroll） |
| アンサーラー | statistics / random / weather を実装 |
| favicons | 実装（`/favicon_proxy`、キャッシュ付き） |
| i18n | 11 ロケール（`tools/import_translations.py` で生成） |
| wikidata インフォボックス | `src/wikidata_infobox.rs` に実装 |
| valkey / redis キャッシュ | 未移植（TTL + LRU のメモリ内キャッシュで代替） |
| botdetection / limiter.toml | 部分（クライアント単位の簡易レート制限のみ） |
