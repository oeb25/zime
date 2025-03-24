use color_eyre::eyre::eyre;

use crate::Result;

const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 11_3_1 like Mac OS X) AppleWebKit/603.1.30 (KHTML, like Gecko) Version/10.0 Mobile/14E304 Safari/602.1";

pub fn is_arxiv(doi: &str) -> bool {
    doi.contains("/ARXIV.")
}

/// Fetches a PDF from arXiv given a DOI.
///
/// For example, `fetch_pdf("10.48550/ARXIV.2207.0282")` will fetch the PDF from `https://arxiv.org/pdf/2103.03230.pdf`.
pub fn fetch_pdf(doi: &str) -> Result<Vec<u8>> {
    let id = doi
        .split_once("/ARXIV.")
        .map(|(_, id)| id)
        .ok_or_else(|| eyre!("Invalid arXiv DOI"))?;
    let url = format!("https://arxiv.org/pdf/{id}.pdf");
    let response = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .build()?
        .get(&url)
        .send()?;
    let body = response.bytes()?;
    Ok(body.to_vec())
}
