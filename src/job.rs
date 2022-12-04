use async_graphql::futures_util::StreamExt;
use async_graphql::ID;
use chrono::DateTime;
use chrono::Utc;
use dashmap::DashMap;
use eyre::eyre;
use eyre::ContextCompat;
use eyre::Result;
use std::{path::PathBuf, sync::Arc};
use tempfile::TempDir;
use tracing::info;
use tracing::warn;

use crate::engine::ENGINES;

pub mod create_job_mutation;

pub struct Job {
    pub id: ID,
    pub temp_dir: TempDir,
    pub src: async_graphql::UploadValue,
    pub src_path: PathBuf,
    pub config: async_graphql::UploadValue,
    pub config_path: PathBuf,
    pub engine_url: String,
    pub status: JobStatus,
    pub percent_complete: f32,
    pub created_at: DateTime<Utc>,
}

#[derive(PartialEq)]
pub enum JobStatus {
    Waiting,
    Started,
    Completed(DateTime<Utc>),
    Errored((String, DateTime<Utc>)),
}

pub type JobMap = Arc<DashMap<ID, Job>>;
pub type JobQueue = tokio::sync::mpsc::UnboundedSender<ID>;

#[derive(async_graphql::SimpleObject)]
#[graphql(name = "Job")]
pub struct JobGraphQL {
    id: ID,
    is_done: bool,
    error: Option<JobError>,
    engine_url: String,
    percent_complete: f32,
    gcode_url: String,
}

#[derive(async_graphql::SimpleObject)]
pub struct JobError {
    message: String,
}

impl Job {
    pub fn gcode_path(&self) -> PathBuf {
        self.src_path.with_extension(".gcode")
    }

    pub fn graphql(&self) -> JobGraphQL {
        let error = if let JobStatus::Errored((message, _)) = &self.status {
            Some(JobError {
                message: message.clone(),
            })
        } else {
            None
        };

        JobGraphQL {
            id: self.id.clone(),
            is_done: matches!(self.status, JobStatus::Completed(_)),
            error,
            engine_url: self.engine_url.clone(),
            percent_complete: self.percent_complete,
            gcode_url: format!("/job/{}/gcode", &self.id.0),
        }
    }

    pub async fn run(jobs: &JobMap, job_id: &ID) -> Result<()> {
        let mut job = jobs.get_mut(job_id).wrap_err("Unable to find job")?;
        job.status = JobStatus::Started;

        let job = jobs.get(&job_id).wrap_err("Unable to find job")?;

        let src_path = job.src_path.clone();
        let config_path = job.config_path.clone();
        let gcode_path = job.gcode_path();

        let release = ENGINES
            .values()
            .filter_map(|engine| engine.release_config.parse(&job.engine_url).ok())
            .next()
            .ok_or_else(|| eyre!("Engine not found for release url: {:?}", &job.engine_url))?;

        drop(job);

        let mut job_stream = release.generate_gcode(src_path, config_path, gcode_path);

        while let Some(precent_complete) = job_stream.next().await {
            let mut job = jobs.get_mut(job_id).wrap_err("Unable to find job")?;
            job.percent_complete = precent_complete?;
        }

        let mut job = jobs.get_mut(job_id).wrap_err("Unable to find job")?;
        job.status = JobStatus::Completed(Utc::now());
        info!("Slicing... [DONE]");

        Ok(())
    }
}

pub async fn run_job_queue(
    jobs: JobMap,
    mut job_queue_rx: tokio::sync::mpsc::UnboundedReceiver<ID>,
) -> Result<()> {
    while let Some(job_id) = job_queue_rx.recv().await {
        if let Err(err) = Job::run(&jobs, &job_id).await {
            warn!("Slicing Failure: {:?}", err);
            let mut job = jobs.get_mut(&job_id).wrap_err("Unable to find job")?;
            job.status = JobStatus::Errored((err.to_string(), Utc::now()));
        }
    }

    Ok(())
}
