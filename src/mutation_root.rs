use async_graphql::MergedObject;

use crate::job::create_job_mutation::CreateJobMutation;

#[derive(MergedObject, Default)]
pub struct Mutation(CreateJobMutation);
