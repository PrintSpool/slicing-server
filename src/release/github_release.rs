use super::GithubReleaseConfig;
use crate::config::directories;
use crate::release::github_req_client::query_github_api;
use crate::release::github_req_client::req_client;
use eyre::eyre;
use eyre::Result;
use futures_util::StreamExt;
use std::fs::Permissions;
use std::os::unix::prelude::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use tokio::fs;
use tokio::fs::create_dir_all;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::info;
use tracing::trace;

#[derive(serde::Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub struct GithubRelease {
    pub config: GithubReleaseConfig,
    pub tag: String,
}

impl GithubRelease {
    pub fn bin_path(&self) -> Result<PathBuf> {
        let Self { config, tag, .. } = &self;
        let repo = &config.repo;

        let bin_path = directories()?
            .data_dir()
            .join("engines")
            .join(repo.replace("/", "-").to_lowercase())
            .join(format!("{repo}-{}", tag).replace("/", "-").to_lowercase())
            .with_extension("AppImage");

        Ok(bin_path)
    }

    pub fn bin_path_if_downloaded(&self) -> Result<PathBuf> {
        let Self { config, tag, .. } = &self;
        let repo = &config.repo;

        let bin_path = self.bin_path()?;

        if bin_path.exists() {
            Ok(bin_path)
        } else {
            Err(eyre!("{repo} {tag} is not installed"))
        }
    }

    pub async fn download(&self, no_cache: bool) -> Result<PathBuf> {
        let Self { config, tag, .. } = &self;
        let repo = &config.repo;

        let bin_path = self.bin_path()?;

        if !no_cache && bin_path.exists() {
            info!("{repo} {tag} is already installed. To force a reinstall use --force");
            return Ok(bin_path);
        }

        create_dir_all(
            &bin_path
                .parent()
                .ok_or_else(|| eyre!("Unable to get bin directory"))?,
        )
        .await?;

        // Get a list of assets associated with the release
        trace!("Querying Github API for release info");
        let mut json = query_github_api(&format!(
            "https://api.github.com/repos/{repo}/releases/tags/{tag}"
        ))
        .await?;

        // Find the App Image in the assets
        let asset = json
            .get_mut("assets")
            .ok_or_else(|| eyre!("Missing assets"))?
            .take()
            .as_array_mut()
            .ok_or_else(|| eyre!("Invalid assets array"))?
            .iter_mut()
            .map(|asset| Ok(serde_json::from_value::<GithubAsset>(asset.take())?))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .find(|asset| (self.config.asset_filter)(&asset.name))
            .ok_or_else(|| {
                eyre!("Binary not found in Github release assets for {repo} version {tag}",)
            })?;

        // Download the App Image
        info!("Downloading {repo} {tag} from Github");

        let app_image_req = req_client()?
            .get(asset.browser_download_url)
            .header("User-Agent", "slicing-server")
            .send()
            .await?
            .error_for_status()?;

        let len = app_image_req.content_length().unwrap_or(0);
        let mut byte_count = 0;
        let mut last_logged_at = SystemTime::now();
        let mut app_image_bytes_stream = app_image_req.bytes_stream();

        let download_path = bin_path.with_extension("partial");
        let mut file = File::create(&download_path).await?;

        while let Some(bytes) = app_image_bytes_stream.next().await {
            let bytes = bytes?;
            byte_count += bytes.len();
            let now = SystemTime::now();

            if last_logged_at + Duration::from_secs(1) < now {
                last_logged_at = now;
                info!(
                    "Downloading: {} MB / {} MB",
                    byte_count / 1_000_000,
                    len / 1_000_000,
                );
            }

            file.write_all(&bytes[..]).await?;
        }

        file.shutdown().await?;
        drop(file);

        fs::set_permissions(&download_path, Permissions::from_mode(0o755)).await?;
        fs::rename(&download_path, &bin_path).await?;

        info!("Download complete! Installed at:\n  {:?}", &bin_path);

        Ok(bin_path)
    }
}
