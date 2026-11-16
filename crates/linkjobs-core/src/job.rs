//! The one Apalis job type: fetch a shared link and pull out the page title.

use std::time::Duration;

use apalis::prelude::{BoxDynError, Data};
use serde::{Deserialize, Serialize};

/// Read at most this much of a page; `<title>` lives in `<head>`.
const MAX_BODY_BYTES: usize = 512 * 1024;
/// Titles longer than this are cut so they fit in a notification.
const MAX_TITLE_CHARS: usize = 200;

/// Job arguments, stored as JSON in the SQLite `Jobs.job` column.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FetchTitle {
    pub url: String,
}

/// Job result, stored as JSON in the SQLite `Jobs.last_result` column.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageInfo {
    pub url: String,
    pub final_url: String,
    pub http_status: u16,
    pub title: Option<String>,
    pub bytes_read: usize,
}

/// Build the HTTP client the worker shares between jobs.
pub fn http_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("linkjobs/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
}

/// The Apalis handler. Errors are retried by the worker's retry layer.
pub async fn fetch_title(
    job: FetchTitle,
    client: Data<reqwest::Client>,
) -> Result<PageInfo, BoxDynError> {
    tracing::info!(url = %job.url, "fetching");
    let mut response = client.get(&job.url).send().await?;
    let status = response.status();
    if status.is_server_error() {
        // 5xx is often temporary: fail so Apalis retries it.
        return Err(format!("server returned {status}").into());
    }
    let final_url = response.url().to_string();

    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        body.extend_from_slice(&chunk);
        if body.len() >= MAX_BODY_BYTES {
            break;
        }
    }
    let html = String::from_utf8_lossy(&body);
    let info = PageInfo {
        url: job.url,
        final_url,
        http_status: status.as_u16(),
        title: extract_title(&html),
        bytes_read: body.len(),
    };
    tracing::info!(title = ?info.title, status = info.http_status, "fetched");
    Ok(info)
}

/// Find the first `http(s)://` link in shared text (share sheets often add extra words).
pub fn extract_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .filter_map(|word| {
            let start = word.find("https://").or_else(|| word.find("http://"))?;
            let candidate = word[start..]
                .trim_end_matches(['.', ',', ';', ':', ')', ']', '>', '"', '\'', '!', '?']);
            let url = reqwest::Url::parse(candidate).ok()?;
            url.host_str().is_some().then(|| url.to_string())
        })
        .next()
}

/// Extract and clean the text of the first `<title>` element.
pub fn extract_title(html: &str) -> Option<String> {
    // ASCII lowercasing keeps byte offsets identical, so indexes map back to `html`.
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open_end = start + lower[start..].find('>')? + 1;
    let close = open_end + lower[open_end..].find("</title")?;
    let text = decode_entities(&html[open_end..close])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        return None;
    }
    Some(match text.char_indices().nth(MAX_TITLE_CHARS) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text,
    })
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let decoded = rest.find(';').filter(|&end| end <= 10).and_then(|end| {
            let name = &rest[1..end];
            let ch = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                    .and_then(char::from_u32),
            }?;
            Some((ch, end))
        });
        match decoded {
            Some((ch, end)) => {
                out.push(ch);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_title() {
        let html =
            "<html><HEAD><Title data-x=1>\n  Rust &amp; Friends &#8212; Home\n</TITLE></head>";
        assert_eq!(
            extract_title(html).as_deref(),
            Some("Rust & Friends — Home")
        );
    }

    #[test]
    fn missing_or_empty_title() {
        assert_eq!(extract_title("<html><body>hi</body></html>"), None);
        assert_eq!(extract_title("<title>   </title>"), None);
    }

    #[test]
    fn keeps_unknown_entities() {
        assert_eq!(decode_entities("a &bogus; b & c"), "a &bogus; b & c");
    }

    #[test]
    fn long_titles_are_cut() {
        let html = format!("<title>{}</title>", "é".repeat(500));
        let title = extract_title(&html).unwrap();
        assert_eq!(title.chars().count(), MAX_TITLE_CHARS + 1);
    }

    #[test]
    fn finds_url_in_shared_text() {
        assert_eq!(
            extract_url("Check this out: https://example.com/a?b=1).").as_deref(),
            Some("https://example.com/a?b=1")
        );
        assert_eq!(extract_url("no links here"), None);
        assert_eq!(extract_url("ftp://example.com"), None);
    }
}
