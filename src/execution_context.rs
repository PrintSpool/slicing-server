use eyre::Result;
use std::{path::PathBuf, sync::Arc};

pub struct ExecutionContext<R> {
    pub release: R,
    pub co: Arc<genawaiter::sync::Co<Result<f32>, ()>>,
    pub src_path: PathBuf,
    pub config_path: PathBuf,
    pub gcode_path: PathBuf,
}
