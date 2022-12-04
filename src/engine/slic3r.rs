use super::Engine;
use crate::{
    execution_context::ExecutionContext,
    release::{GithubRelease, GithubReleaseConfig},
};
use cgmath::Matrix4;
use eyre::{eyre, Context, Result};
use futures_util::FutureExt;
use tracing::info;

pub fn engines() -> Vec<Engine> {
    vec![
        Engine {
            id: "slic3r".into(),
            name: "Slic3r".into(),
            transform_mat4: Matrix4::from_scale(1.0),
            allows_positioning: true,
            invert_rotation: Default::default(),
            accepted_file_formats: vec![".stl", ".obj", ".amf", ".3mf"],
            release_url: Some("https://github.com/slic3r/Slic3r/releases"),
            home_page: "https://github.com/slic3r/Slic3r",
            release_config: Box::pin(GithubReleaseConfig {
                repo: "slic3r/Slic3r".to_owned(),
                asset_filter: &|asset: &str| {
                    // Only X64 support for now
                    asset.contains("-x86_64") && asset.ends_with(".AppImage")
                },
                generate_gcode_inner: &|exec_ctx| generate_gcode(exec_ctx).boxed(),
            }),
        },
        Engine {
            id: "prusa_slicer".into(),
            name: "Prusa Slicer".into(),
            transform_mat4: Matrix4::from_scale(1.0),
            allows_positioning: true,
            invert_rotation: Default::default(),
            accepted_file_formats: vec![".stl", ".obj", ".amf", ".3mf"],
            release_url: Some("https://github.com/prusa3d/PrusaSlicer/releases"),
            home_page: "https://github.com/prusa3d/PrusaSlicer",
            release_config: Box::pin(GithubReleaseConfig {
                repo: "prusa3d/PrusaSlicer".to_owned(),
                asset_filter: &|asset: &str| {
                    // Only X64 support for now
                    asset.contains("-x64-GTK3") && asset.ends_with(".AppImage")
                },
                generate_gcode_inner: &|exec_ctx| generate_gcode(exec_ctx).boxed(),
            }),
        },
        Engine {
            id: "super_slicer".into(),
            name: "Super Slicer".into(),
            transform_mat4: Matrix4::from_scale(1.0),
            allows_positioning: true,
            invert_rotation: Default::default(),
            accepted_file_formats: vec![".stl", ".obj", ".amf", ".3mf"],
            release_url: Some("https://github.com/supermerill/SuperSlicer/releases"),
            home_page: "https://github.com/supermerill/SuperSlicer",
            release_config: Box::pin(GithubReleaseConfig {
                repo: "supermerill/SuperSlicer".to_owned(),
                asset_filter: &|asset: &str| {
                    // Only X64 support for now
                    asset.contains("-ubuntu_18.04-") && asset.ends_with(".AppImage")
                },
                generate_gcode_inner: &|exec_ctx| generate_gcode(exec_ctx).boxed(),
            }),
        },
    ]
}

pub async fn generate_gcode(exec_ctx: ExecutionContext<GithubRelease>) -> Result<()> {
    let ExecutionContext {
        release,
        co,
        src_path,
        config_path,
        gcode_path,
    } = exec_ctx;

    let belt_engine_path = release.bin_path_if_downloaded()?;

    // Run as slicing worker
    let mut cmd = tokio::process::Command::new("su");

    cmd.arg("-")
        .arg("slicing-worker")
        .arg("-c")
        // slicer
        .arg(&belt_engine_path)
        // Set slicing profile
        .arg("--load")
        .arg(&config_path)
        // Set gcode output
        .arg("--output")
        .arg(&gcode_path)
        // Run the slicer
        .arg("--slice")
        .arg(&src_path);

    info!("Slicer command: {:?}", cmd);

    let output = cmd.output().await.wrap_err("Slicer error")?;

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let result = if output.status.success() {
        info!("{}", stderr);
        Ok(100.0)
    } else {
        Err(eyre!(stderr))
    };

    co.yield_(result).await;

    Ok(())
}
