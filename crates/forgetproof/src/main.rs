mod evidence;
mod model;
mod protocol;
mod runner;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process;

#[derive(Debug, Parser)]
#[command(name = "forgetproof", version, about = "Prove your AI forgot.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create local configuration and reference scenarios.
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Inspect an adapter without mutating a backend.
    Doctor(AdapterArgs),
    /// Manage registered adapters.
    Adapters {
        #[command(subcommand)]
        command: AdapterCommand,
    },
    /// Freeze deterministic lexical and semantic probe variants.
    Expand {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Execute an erasure scenario and write an evidence bundle.
    Run {
        scenario: PathBuf,
        #[arg(long, default_value = ".forgetproof/runs")]
        output: PathBuf,
        #[arg(long)]
        allow_network: bool,
    },
    /// Verify an evidence bundle's checksums.
    Verify { bundle: PathBuf },
    /// Regenerate HTML and JUnit reports from results.json.
    Report { bundle: PathBuf },
}

#[derive(Debug, Subcommand)]
enum AdapterCommand {
    List,
}

#[derive(Debug, Args)]
struct AdapterArgs {
    #[arg(long)]
    adapter: Option<String>,
    #[arg(long, default_value = "default")]
    mode: String,
    /// Repeat as --config key=value. Values are passed to the adapter process.
    #[arg(long = "config", value_parser = parse_key_value)]
    config: Vec<(String, String)>,
}

fn parse_key_value(value: &str) -> Result<(String, String), String> {
    value
        .split_once('=')
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .ok_or_else(|| "expected key=value".to_owned())
}

fn main() {
    if let Err(error) = run_cli() {
        eprintln!("error: {error:#}");
        process::exit(2);
    }
}

fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { path } => {
            runner::init_project(&path)?;
            println!("initialized ForgetProof project at {}", path.display());
        }
        Command::Doctor(args) => {
            let config = args.config.into_iter().collect::<BTreeMap<_, _>>();
            let names = args.adapter.map_or_else(
                || {
                    vec![
                        "reference-clean".to_owned(),
                        "reference-leaky".to_owned(),
                        "mem0".to_owned(),
                        "letta".to_owned(),
                        "zep".to_owned(),
                    ]
                },
                |name| vec![name],
            );
            for name in names {
                let capabilities = runner::doctor_adapter(&name, &args.mode, &config)?;
                println!("adapter: {}", capabilities.adapter);
                println!("backend: {}", capabilities.backend);
                println!("protocol: {}", capabilities.protocol);
                println!("version: {}", capabilities.version);
                println!("capabilities: {}", capabilities.capabilities.join(", "));
            }
        }
        Command::Adapters { command } => match command {
            AdapterCommand::List => {
                println!("NAME              KIND       PROTOCOL");
                println!("reference-clean   built-in    {}", model::PROTOCOL_VERSION);
                println!("reference-leaky   built-in    {}", model::PROTOCOL_VERSION);
                println!("mem0              python      {}", model::PROTOCOL_VERSION);
                println!("letta             python      {}", model::PROTOCOL_VERSION);
                println!("zep               python      {}", model::PROTOCOL_VERSION);
            }
        },
        Command::Expand { input, output } => {
            runner::expand_scenario(&input, &output)?;
            println!("wrote frozen scenario to {}", output.display());
        }
        Command::Run {
            scenario,
            output,
            allow_network,
        } => {
            let outcome = runner::run_scenario(&scenario, &output, allow_network)?;
            println!("status: {}", outcome.result.status);
            println!("exit code: {}", outcome.result.exit_code);
            println!("bundle: {}", outcome.directory.display());
            println!("bundle hash: {}", outcome.bundle_hash);
            if !outcome.result.warnings.is_empty() {
                println!("warnings:");
                for warning in outcome.result.warnings {
                    println!("  - {warning}");
                }
            }
            process::exit(outcome.result.exit_code);
        }
        Command::Verify { bundle } => {
            let verification = evidence::verify_bundle(&bundle)?;
            if verification.valid {
                println!("valid bundle: {}", verification.bundle_hash);
            } else {
                println!("invalid bundle: {}", verification.bundle_hash);
                for error in verification.errors {
                    println!("  - {error}");
                }
                process::exit(1);
            }
        }
        Command::Report { bundle } => {
            let result = evidence::load_result(&bundle)?;
            std::fs::write(bundle.join("report.html"), evidence::render_html(&result))?;
            std::fs::write(bundle.join("junit.xml"), evidence::render_junit(&result))?;
            println!(
                "regenerated report.html and junit.xml in {}",
                bundle.display()
            );
        }
    }
    Ok(())
}
