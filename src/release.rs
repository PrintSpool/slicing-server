use crate::execution_context::ExecutionContext;
use eyre::eyre;
use eyre::Result;
use futures_util::Future;
use futures_util::Stream;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

mod github_release;
mod github_release_config;
mod github_req_client;
mod local_release;

pub use self::local_release::LocalRelease;
pub use self::local_release::LocalReleaseConfig;
pub use github_release::GithubRelease;
pub use github_release_config::GithubReleaseConfig;

pub trait ReleaseConfig {
    fn parse<'a>(&self, url: &'a str) -> Result<Release>;
    fn latest_release(&self) -> Pin<Box<dyn Future<Output = Result<Release>>>>;
}

pub enum Release {
    Github(GithubRelease),
    Local(LocalRelease),
}

impl Release {
    pub async fn download(&self, no_cache: bool) -> Result<PathBuf> {
        match &self {
            Release::Github(github_release) => github_release.download(no_cache).await,
            Release::Local(_) => Err(eyre!("Engine must be manually installed")),
        }
    }

    pub fn generate_gcode(
        self,
        src_path: PathBuf,
        config_path: PathBuf,
        gcode_path: PathBuf,
    ) -> impl Stream<Item = Result<f32>> {
        genawaiter::sync::Gen::new(move |co| async move {
            let co = Arc::new(co);
            let result = match self {
                Release::Github(release) => {
                    (release.config.generate_gcode_inner)(ExecutionContext {
                        release,
                        co: Arc::clone(&co),
                        src_path,
                        config_path,
                        gcode_path,
                    })
                    .await
                }
                Release::Local(release) => {
                    (release.config.generate_gcode_inner)(ExecutionContext {
                        release,
                        co: Arc::clone(&co),
                        src_path,
                        config_path,
                        gcode_path,
                    })
                    .await
                }
            };
            if let Err(err) = result {
                co.yield_(Err(err.into())).await;
            }
        })
    }
}
