use eyre::Result;

pub fn req_client() -> Result<reqwest::Client> {
    let req_client = reqwest::ClientBuilder::new().https_only(true).build()?;
    Ok(req_client)
}

pub async fn query_github_api(url: &str) -> Result<serde_json::Value> {
    let json: serde_json::Value = req_client()?
        .get(url)
        .header("User-Agent", "slicing-server")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(json)
}
