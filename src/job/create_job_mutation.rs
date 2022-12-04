use super::{Job, JobGraphQL, JobMap, JobQueue, JobStatus};
use async_graphql::{FieldResult, UploadValue};
use chrono::{Duration, Utc};
use eyre::Result;
use std::{os::unix::prelude::AsRawFd, path::PathBuf};
use tempfile::TempDir;
use tracing::instrument;

#[derive(Default)]
pub struct CreateJobMutation;

#[derive(async_graphql::InputObject)]
struct Vec3Input {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(async_graphql::InputObject)]
struct CreateJobInput {
    src: async_graphql::Upload,
    config: async_graphql::Upload,
    /// The engine to use to generate the GCode
    #[graphql(name = "engineURL")]
    engine_url: String,
}

fn move_upload_to_dir(upload: &UploadValue, temp_dir: &TempDir) -> Result<PathBuf> {
    // Get a path to the unlinked temp file
    let fd_path =
        Into::<PathBuf>::into("/proc/self/fd/").join(upload.content.as_raw_fd().to_string());

    // Create a new path to move the temp file to
    let named_file_path = temp_dir.path().join(&upload.filename);

    // Create a name in the file system for the temp file - it will automatically be cleaned up
    // by the OS when the File gets dropped.
    nix::unistd::linkat(
        None,
        &fd_path,
        None,
        &named_file_path,
        nix::unistd::LinkatFlags::SymlinkFollow,
    )?;

    Ok(named_file_path)
}

fn cleanup_old_jobs(jobs: &JobMap) -> Result<()> {
    // Delete jobs that errored or completed more than an hour ago
    let deletion_threshold = Utc::now() - Duration::hours(1);

    jobs.retain(|_, job| match job.status {
        JobStatus::Completed(completed_at) => completed_at > deletion_threshold,
        JobStatus::Errored((_, errored_at)) => errored_at > deletion_threshold,
        _ => true,
    });

    Ok(())
}

#[async_graphql::Object]
impl CreateJobMutation {
    /// Adds a job to the server's internal queue for processing into GCode.
    #[instrument(skip(self, input, ctx))]
    async fn create_job<'ctx>(
        &self,
        ctx: &'ctx async_graphql::Context<'_>,
        input: CreateJobInput,
    ) -> FieldResult<JobGraphQL> {
        let job_queue: &JobQueue = ctx.data()?;
        let jobs: &JobMap = ctx.data()?;

        cleanup_old_jobs(jobs)?;

        let src = input.src.value(&ctx)?;
        let config = input.config.value(&ctx)?;

        let temp_dir = tempfile::tempdir()?;

        let src_path = move_upload_to_dir(&src, &temp_dir)?;
        let config_path = move_upload_to_dir(&config, &temp_dir)?;

        let job = Job {
            id: nanoid::nanoid!().into(),
            temp_dir,
            src,
            src_path,
            config,
            config_path,
            engine_url: input.engine_url,
            status: JobStatus::Waiting,
            percent_complete: 0.0,
            created_at: Utc::now(),
        };

        // Insert the job and return a reference to it
        let entry_ref = jobs.entry(job.id.clone()).or_insert(job);
        let job = entry_ref.value();
        job_queue.send(job.id.clone())?;

        Ok(job.graphql())
    }
}
