use async_graphql::{Context, FieldResult, Object, ID};

use crate::job::{JobGraphQL, JobMap};
use eyre::eyre;

pub struct QueryRoot;

#[derive(async_graphql::InputObject)]
struct JobInput {
    id: ID,
}

#[Object]
impl QueryRoot {
    async fn job<'a>(&self, ctx: &'a Context<'_>, input: JobInput) -> FieldResult<JobGraphQL> {
        let jobs: &JobMap = ctx.data()?;
        let job = jobs.get(&input.id).ok_or_else(|| eyre!("Job not found"))?;

        Ok(job.graphql())
    }
}
