use anyhow::{bail, Context, Result};
use url::Url;

pub fn fetch_and_convert(raw_url: &str) -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!(
            "Mozilla/5.0 (compatible; ",
            env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),
            "; +", env!("CARGO_PKG_REPOSITORY"), ")"
        ))
        .build()
        .context("failed to build HTTP client")?;
    let resp = client
        .get(raw_url)
        .send()
        .with_context(|| format!("failed to fetch {raw_url}"))?;
    let bytes = resp.bytes().context("failed to read response body")?;
    if bytes.is_empty() {
        bail!("no content returned from {raw_url} (site may be blocking automated requests)");
    }
    let body = decode_bytes(&bytes);
    convert(&body, raw_url)
}

pub fn convert(html: &str, base_url: &str) -> Result<String> {
    let url = Url::parse(base_url).unwrap_or_else(|_| Url::parse("https://example.com").unwrap());
    let article = readability::extractor::extract(&mut html.as_bytes(), &url)
        .context("readability extraction failed")?;
    if article.content.trim().is_empty() {
        bail!("no article content extracted (page may require JavaScript or a login)");
    }
    htmd::convert(&article.content).context("htmd conversion failed")
}

fn decode_bytes(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}
