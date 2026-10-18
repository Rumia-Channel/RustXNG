//! CSV, RSS and OpenSearch responses, ported from `searx/webutils.py` and
//! `searx/templates/opensearch_response_rss.xml`.

use axum::http::header;
use axum::response::{IntoResponse, Response};

use super::AppState;
use crate::aggregator::SearchResponse;
use crate::engine::util::encode;

/// `format=csv`: one row per result, like upstream's `write_csv_response`.
pub fn csv_response(response: &SearchResponse) -> Response {
    let mut body =
        String::from("title,url,content,engine,engines,score,category,template,publishedDate\n");
    for result in &response.results {
        body.push_str(&format!(
            "{},{},{},{},{},{:.2},{},{},{}\n",
            csv_field(&result.title),
            csv_field(&result.url),
            csv_field(&result.content),
            csv_field(&result.engine),
            csv_field(&result.engines.join(", ")),
            result.score,
            csv_field(&result.category),
            csv_field(result.template.as_str()),
            csv_field(result.published_date.as_deref().unwrap_or_default()),
        ));
    }
    let disposition = format!(
        "attachment;Filename=rustxng-{}.csv",
        response.search_query.replace(['"', '\r', '\n'], "")
    );
    (
        [
            (
                header::CONTENT_TYPE,
                "application/csv; charset=utf-8".to_string(),
            ),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        body,
    )
        .into_response()
}

fn csv_field(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// `format=rss`: an RSS 2.0 feed with one item per result.
pub fn rss_response(state: &AppState, response: &SearchResponse, raw_query: &str) -> Response {
    let base = state.base_url();
    let instance = &state.settings().general.instance_name;
    let search_url = format!("{base}/search?q={}", encode(raw_query));
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\" xmlns:opensearch=\"http://a9.com/-/spec/opensearch/1.1/\">\n<channel>\n",
    );
    xml.push_str(&format!("<title>{}</title>\n", escape(instance)));
    xml.push_str(&format!(
        "<link>{}</link>\n<description>Search results for \"{}\" - {}</description>\n",
        escape(&search_url),
        escape(raw_query),
        escape(instance)
    ));
    xml.push_str(&format!(
        "<opensearch:totalResults>{}</opensearch:totalResults>\n<opensearch:startIndex>1</opensearch:startIndex>\n<opensearch:itemsPerPage>{}</opensearch:itemsPerPage>\n",
        response.number_of_results,
        response.results.len()
    ));
    for result in &response.results {
        xml.push_str("<item>\n");
        xml.push_str(&format!("<title>{}</title>\n", escape(&result.title)));
        xml.push_str(&format!("<link>{}</link>\n", escape(&result.url)));
        xml.push_str(&format!(
            "<description>{}</description>\n",
            escape(&result.content)
        ));
        xml.push_str(&format!(
            "<guid isPermaLink=\"true\">{}</guid>\n",
            escape(&result.url)
        ));
        if let Some(date) = &result.published_date {
            xml.push_str(&format!("<pubDate>{}</pubDate>\n", escape(date)));
        }
        xml.push_str(&format!(
            "<category>{}</category>\n",
            escape(&result.category)
        ));
        xml.push_str("</item>\n");
    }
    xml.push_str("</channel>\n</rss>\n");
    ([(header::CONTENT_TYPE, "text/xml; charset=utf-8")], xml).into_response()
}

/// `/opensearch.xml`: the browser search plugin description.
pub fn opensearch_response(state: &AppState) -> Response {
    let base = state.base_url();
    let instance = &state.settings().general.instance_name;
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<OpenSearchDescription xmlns=\"http://a9.com/-/spec/opensearch/1.1/\">\n<ShortName>{short}</ShortName>\n<Description>Search {name}</Description>\n<InputEncoding>UTF-8</InputEncoding>\n<Url type=\"text/html\" method=\"GET\" template=\"{base}/search?q={{searchTerms}}\" />\n<Url type=\"application/x-suggestions+json\" method=\"GET\" template=\"{base}/autocompleter?q={{searchTerms}}\" />\n</OpenSearchDescription>\n",
        short = escape(
            &instance
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        ),
        name = escape(instance),
        base = escape(&base),
    );
    (
        [(
            header::CONTENT_TYPE,
            "application/opensearchdescription+xml",
        )],
        xml,
    )
        .into_response()
}

/// Minimal XML escaping for the feed bodies.
pub fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_fields_are_quoted() {
        assert_eq!(csv_field("a \"b\" c"), "\"a \"\"b\"\" c\"");
    }

    #[test]
    fn xml_is_escaped() {
        assert_eq!(escape("<a & b>"), "&lt;a &amp; b&gt;");
    }
}
