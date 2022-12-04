use crate::auth::auth;
use crate::config::{directories, Config};
use crate::error::AppResult;
use crate::job::{self, JobMap, JobQueue};
use crate::mutation_root::Mutation;
use crate::query_root::QueryRoot;
use async_graphql::EmptySubscription;
use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    Schema,
};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::middleware;
use axum::response::Html;
use axum::{
    extract::Extension, extract::Path, http::StatusCode, response::IntoResponse, routing::get,
    Router,
};
use dashmap::DashMap;
use eyre::Result;
use eyre::{eyre, Context};
use hyper::server::accept;
use self_host_space::KeyManager;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::{fs, sync::mpsc::unbounded_channel};
use tracing::info;

pub struct SharedState {
    pub jobs: JobMap,
}

type AppSchema = Schema<QueryRoot, Mutation, EmptySubscription>;

pub async fn serve() -> Result<()> {
    info!("Starting Slicing Server");

    let dirs = directories()?;
    let config = Arc::new(Config::load().await?);

    let jobs: JobMap = Arc::new(DashMap::new());
    let (_job_queue_tx, job_queue_rx): (JobQueue, _) = unbounded_channel();

    // Start the job queue
    job::run_job_queue(jobs.clone(), job_queue_rx).await?;

    let shared_state = Arc::new(SharedState { jobs });

    // build the http server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);

    let server_keys = KeyManager::load_or_create(dirs.config_dir()).await?;
    let self_host_server = self_host_space::Server::new(server_keys);

    self_host_server
        .serve(move |async_wt_server| {
            let config = Arc::clone(&config);
            let shared_state = Arc::clone(&shared_state);

            async move {
                let schema: AppSchema =
                    Schema::new(QueryRoot, Mutation::default(), EmptySubscription);

                // build the server routes
                let routes = Router::new()
                    .route(
                        "/jobs/:job_id/gcode",
                        get({
                            let shared_state = Arc::clone(&shared_state);
                            move |path| get_job_gcode(path, shared_state)
                        }),
                    )
                    .route("/", get(graphql_playground).post(graphql_handler))
                    .layer(Extension(schema))
                    // The auth extractor will run before all routes
                    .route_layer(middleware::from_fn(move |req, next| {
                        auth(Arc::clone(&config), req, next)
                    }));
                let make_service = routes.into_make_service();

                // Start the http server. It will receive it's requests via the self-host.space Web Transport server to fasciliate
                // secure connections without a doman & signed certificate.
                let accept = accept::from_stream(async_wt_server.into_stream());
                axum::Server::builder(accept).serve(make_service);
            }
        })
        .await?;

    Ok(())
}

async fn graphql_playground() -> impl IntoResponse {
    Html(playground_source(GraphQLPlaygroundConfig::new("/")))
}

async fn graphql_handler(
    Extension(schema): Extension<AppSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let req = req.into_inner();
    // req = req.data(token);

    schema.execute(req).await.into()
}

async fn get_job_gcode(
    Path(job_id): Path<String>,
    shared_state: Arc<SharedState>,
) -> AppResult<impl IntoResponse> {
    let job = shared_state
        .jobs
        .get_mut(&job_id.into())
        .ok_or_else(|| eyre!("Job not found"))?;

    let gcode = fs::read(job.gcode_path())
        .await
        .wrap_err("Error reading GCode file")?;

    Ok((StatusCode::OK, gcode))
}
