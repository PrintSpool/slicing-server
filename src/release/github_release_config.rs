use super::github_req_client::query_github_api;
use super::Release;
use super::ReleaseConfig;
use crate::execution_context::ExecutionContext;
use crate::release::GithubRelease;
use eyre::eyre;
use eyre::Result;
use futures_util::Future;
use futures_util::FutureExt;
use std::pin::Pin;
use tracing::warn;

pub type AssetFilter = &'static (dyn Fn(&str) -> bool + Sync);

#[derive(Clone)]
pub struct GithubReleaseConfig {
    pub repo: String,
    pub asset_filter: AssetFilter,
    pub generate_gcode_inner: &'static (dyn Fn(ExecutionContext<GithubRelease>) -> Pin<Box<dyn Future<Output = Result<()>>>>
                  + Sync),
}

impl ReleaseConfig for GithubReleaseConfig {
    fn parse<'a>(&self, url: &'a str) -> Result<Release> {
        let (_, release) = self.parse_inner(url).map_err(|err| {
            warn!("Github release URL parser error: {err:?}");
            eyre!("Invalid Github release URL: {url}")
        })?;

        Ok(Release::Github(release))
    }

    fn latest_release(&self) -> Pin<Box<dyn Future<Output = Result<Release>>>> {
        let config = self.clone();

        async move {
            let repo = &config.repo;

            // Use the github api to find the latest release
            let json = query_github_api(&format!(
                "https://api.github.com/repos/{repo}/releases/latest"
            ))
            .await?;

            let html_url = json
                .get("html_url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("Missing html_url in JSON"))?;

            config.parse(html_url)
        }
        .boxed()
    }
}

impl GithubReleaseConfig {
    fn parse_inner<'a>(&self, url: &'a str) -> nom::IResult<&'a str, GithubRelease> {
        use nom::bytes::complete::tag;
        use nom::character::complete::none_of;
        use nom::combinator::all_consuming;
        use nom::combinator::map;
        use nom::combinator::recognize;
        use nom::multi::many1;
        use nom::sequence::preceded;

        map(
            all_consuming(preceded(
                tag(&format!("https://github.com/{}/releases/tag/", self.repo)[..]),
                recognize(many1(none_of("/"))),
            )),
            |tag: &str| GithubRelease {
                config: self.clone(),
                tag: tag.to_owned(),
            },
        )(url)
    }
}
