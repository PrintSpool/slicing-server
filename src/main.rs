use clap::Parser;
use config::directories;
use config::Config;
use engine::ENGINES;
use eyre::Result;
use release::Release;
use self_host_space::KeyManager;
use std::io::Write;
use tracing::error;
use tracing_subscriber::{prelude::*, EnvFilter};

mod auth;
mod config;
mod engine;
mod error;
mod execution_context;
mod job;
mod mutation_root;
mod query_root;
mod release;
mod server;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    Engines(EngineArgs),
    Keys(KeyArgs),
    /// Run the slicing server
    Serve,
}

// Engines
// =========================

/// Manage slicing engines
#[derive(Parser, Debug)]
struct EngineArgs {
    #[command(subcommand)]
    action: EngineAction,
}

#[derive(clap::Subcommand, Debug)]
enum EngineAction {
    Install(InstallEngineArgs),
    /// List the slicing engines
    List,
}

/// Install a slicing engine
#[derive(Parser, Debug)]
struct InstallEngineArgs {
    /// Engine name
    #[arg(index = 1)]
    engine: String,

    /// Install a specific github release of the engine, eg. https://github.com/prusa3d/PrusaSlicer/releases/tag/version_2.5.0
    #[arg(long)]
    release_url: Option<String>,

    /// Force the engine to be re-installed if it is already installed
    #[arg(short, long)]
    force: bool,
}

// Keys
// =========================

/// Manage keys and client authorization
#[derive(Parser, Debug)]
struct KeyArgs {
    #[command(subcommand)]
    action: KeyAction,
}

#[derive(clap::Subcommand, Debug)]
enum KeyAction {
    Add(AddKeyArgs),
    Remove(RemoveKeyArgs),
    /// List the authorized client keys
    List,
}

/// Authorizes a new key and returns its invite token
#[derive(Parser, Debug)]
struct AddKeyArgs {
    /// Choose a unique label in order to distinguish this key from your other ones
    #[arg(index = 1)]
    label: String,
}

/// De-authorize an existing key
#[derive(Parser, Debug)]
struct RemoveKeyArgs {
    id: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(tracing::metadata::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let args = Args::parse();
    let dirs = directories()?;
    let mut config = Config::load().await?;

    match args.action {
        Action::Engines(EngineArgs { action }) => match action {
            EngineAction::Install(install_args) => {
                install(install_args).await?;
            }
            EngineAction::List => print_engines(),
        },
        Action::Keys(KeyArgs { action }) => match action {
            KeyAction::Add(add_args) => {
                let server_keys = KeyManager::load_or_create(dirs.config_dir()).await?;

                let (invite_token, _) = config.add_key(&server_keys, add_args.label.clone())?;
                config.save().await?;

                println!("Key created with label \"{}\"\n", add_args.label);
                println!(
                    "Add the invite token bellow to your PrintSpool slicing settings to connect:\n\n{}\n",
                    invite_token
                )
            }
            KeyAction::Remove(rm_args) => {
                config.authorized_keys.remove(&rm_args.id);
                config.save().await?;

                println!("Key (id: \"{}\") removed", rm_args.id);
            }
            KeyAction::List => {
                if config.authorized_keys.is_empty() {
                    println!("No authorized client keys. Use slicing-server keys add [label] to authorize a slicing client.");
                    return Ok(());
                }

                println!("Authorized client keys:\n");
                for key in config.authorized_keys.values() {
                    println!("  - {} (id: \"{}\")", key.label, key.id)
                }
            }
        },
        Action::Serve => server::serve().await?,
    };

    Ok(())
}

fn print_engines() {
    println!(
        "Available Slicing Engines:\n\n{}",
        ENGINES
            .keys()
            .map(|engine_id| format!("  - {}", engine_id.0))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

async fn install(args: InstallEngineArgs) -> Result<()> {
    let engine = match ENGINES.get(&(&args.engine).into()) {
        Some(engine) => engine,
        None => {
            error!("Unknown engine: {}", &args.engine);
            print_engines();
            let _ = std::io::stdout().flush();
            std::process::exit(1);
        }
    };

    let release = if let Some(release_url) = args.release_url {
        engine.release_config.parse(&release_url)?
    } else {
        (&*engine.release_config).latest_release().await?
    };

    if let Release::Local(_) = &release {
        error!("{} must be manually installed", &args.engine);
        let _ = std::io::stdout().flush();
        std::process::exit(1);
    };

    release.download(args.force).await?;

    Ok(())
}
