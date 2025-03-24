use color_eyre::eyre::eyre;
use tracing::debug;

use crate::Result;

const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 11_3_1 like Mac OS X) AppleWebKit/603.1.30 (KHTML, like Gecko) Version/10.0 Mobile/14E304 Safari/602.1";

pub fn fetch_pdf(doi: &str) -> Result<Vec<u8>> {
    let url = format!("https://sci-hub.ru/{}", doi);
    let response = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .build()?
        .get(&url)
        .send()?;
    let body = response.text()?;
    // println!("{}", body);

    let pdf_url = body
        .lines()
        .find(|line| line.contains(r#"embed type="application/pdf" src=""#))
        .ok_or_else(|| eyre!("Could not find embed src in the response from Sci-Hub"))?
        .split("src=\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    debug!(?pdf_url, "pdf url found");

    let pdf_url = if pdf_url.starts_with("/") {
        format!("https://sci-hub.ru{}", pdf_url)
    } else {
        pdf_url.to_string()
    };

    debug!(?pdf_url, "fetching pdf");
    let pdf_response = reqwest::blocking::get(pdf_url)?;
    Ok(pdf_response.bytes()?.to_vec())
}
