use crate::execution_context::ExecutionContext;
use eyre::eyre;
use eyre::Result;
use futures_util::future;
use futures_util::Future;
use futures_util::FutureExt;
use std::path::PathBuf;
use std::pin::Pin;

use super::Release;
use super::ReleaseConfig;

#[derive(Clone)]
pub struct LocalReleaseConfig {
    pub release_url: String,
    pub bin_path: PathBuf,
    pub generate_gcode_inner: &'static (dyn Fn(ExecutionContext<LocalRelease>) -> Pin<Box<dyn Future<Output = Result<()>>>>
                  + Sync),
}

pub struct LocalRelease {
    pub config: LocalReleaseConfig,
}

impl ReleaseConfig for LocalReleaseConfig {
    fn parse(&self, url: &str) -> Result<Release> {
        if self.release_url == url {
            Ok(Release::Local(LocalRelease {
                config: self.clone(),
            }))
        } else {
            Err(eyre!("Incorrect release URL for locally installed engine"))
        }
    }

    fn latest_release(&self) -> Pin<Box<dyn Future<Output = Result<Release>>>> {
        future::ok(Release::Local(LocalRelease {
            config: self.clone(),
        }))
        .boxed()
    }
}

impl LocalRelease {
    pub fn bin_path_if_downloaded(&self) -> Result<PathBuf> {
        if self.config.bin_path.exists() {
            Ok(self.config.bin_path.clone())
        } else {
            Err(eyre!("Belt engine is not installed"))
        }
    }
}
