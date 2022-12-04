use crate::{
    engine::InvertRotation,
    execution_context::ExecutionContext,
    release::{LocalRelease, LocalReleaseConfig},
};
use cgmath::Matrix4;
use eyre::{eyre, Context, Result};
use futures_util::FutureExt;
use tracing::info;

// beltEngine configs were previously stored in crate::paths::etc().join("CR30.cfg.ini")

pub const BELT_ENGINE_URL: &'static str = "https://github.com/Autodrop3d/BeltEngine";

pub fn engine() -> super::Engine {
    super::Engine {
        id: "belt_engine".into(),
        name: "Belt Engine",
        transform_mat4: Matrix4::from_nonuniform_scale(1.0, -1.0, 1.0),
        allows_positioning: false,
        invert_rotation: InvertRotation {
            x: false,
            y: false,
            z: true,
        },
        accepted_file_formats: vec![".stl", ".obj"],
        release_url: None,
        home_page: BELT_ENGINE_URL,
        release_config: Box::pin(LocalReleaseConfig {
            bin_path: std::env::var("BELT_ENGINE")
                .unwrap_or("beltengine".to_owned())
                .into(),
            release_url: BELT_ENGINE_URL.to_owned(),
            generate_gcode_inner: &|exec_ctx| generate_gcode(exec_ctx).boxed(),
        }),
    }
}

pub async fn generate_gcode(exec_ctx: ExecutionContext<LocalRelease>) -> Result<()> {
    let ExecutionContext {
        release,
        co,
        src_path,
        config_path,
        gcode_path,
    } = exec_ctx;

    let belt_engine_path = &release.bin_path_if_downloaded()?;

    // Run as slicing worker
    let mut cmd = tokio::process::Command::new("su");

    cmd.arg("-")
        .arg("slicing-worker")
        .arg("-c")
        // slicer
        .arg(&belt_engine_path)
        // slicing profile
        .arg("-c")
        .arg(&config_path)
        // gcode output
        .arg("-o")
        .arg(&gcode_path)
        // load model
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
