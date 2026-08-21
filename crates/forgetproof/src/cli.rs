use crate::{evidence, model, runner};
use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process;

#[derive(Debug, Parser)]
#[command(
    name = "memoryproof",
    version,
    about = "Test what your AI remembers. Prove what it forgot."
)]
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
        #[arg(long, default_value_t = false, default_missing_value = "true", num_args = 0..=1)]
        allow_network: bool,
    },
    /// Execute a MemoryProof scenario and write an evidence bundle.
    Run {
        scenario: PathBuf,
        #[arg(long, default_value = ".memoryproof/runs")]
        output: PathBuf,
        #[arg(long, default_value_t = false, default_missing_value = "true", num_args = 0..=1)]
        allow_network: bool,
    },
    /// Verify an evidence bundle's checksums and format.
    Verify { bundle: PathBuf },
    /// Regenerate reports and refresh the evidence checksums.
    Report { bundle: PathBuf },
    /// Validate a generated static conformance matrix.
    Matrix {
        #[command(subcommand)]
        command: MatrixCommand,
    },
}

#[derive(Debug, Subcommand)]
enum AdapterCommand {
    List,
}

#[derive(Debug, Subcommand)]
enum MatrixCommand {
    Validate {
        #[arg(default_value = "site/matrix.json")]
        path: PathBuf,
    },
}

#[derive(Debug, Args)]
struct AdapterArgs {
    #[arg(long)]
    adapter: Option<String>,
    #[arg(long, default_value = "default")]
    mode: String,
    #[arg(long, default_value_t = false, default_missing_value = "true", num_args = 0..=1)]
    allow_network: bool,
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

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { path } => {
            runner::init_project(&path)?;
            println!("initialized MemoryProof project at {}", path.display());
        }
        Command::Doctor(args) => {
            let config = args.config.into_iter().collect::<BTreeMap<_, _>>();
            let names = args.adapter.map_or_else(
                || {
                    vec![
                        "reference-clean".to_owned(),
                        "reference-leaky".to_owned(),
                        "reference-overdelete".to_owned(),
                        "mem0".to_owned(),
                        "letta".to_owned(),
                        "zep".to_owned(),
                    ]
                },
                |name| vec![name],
            );
            for name in names {
                let capabilities =
                    runner::doctor_adapter(&name, &args.mode, &config, args.allow_network)?;
                println!("adapter: {}", capabilities.adapter);
                println!("backend: {}", capabilities.backend);
                println!("protocol: {}", capabilities.protocol);
                println!("version: {}", capabilities.version);
                println!("capabilities: {}", capabilities.capabilities.join(", "));
                if !capabilities.modes.is_empty() {
                    println!("modes: {}", capabilities.modes.join(", "));
                }
            }
        }
        Command::Adapters { command } => match command {
            AdapterCommand::List => {
                println!("NAME                   KIND       PROTOCOL");
                for name in [
                    "reference-clean",
                    "reference-leaky",
                    "reference-overdelete",
                    "reference-slow",
                    "reference-crash",
                    "reference-malformed",
                    "reference-wrong-id",
                    "reference-wrong-version",
                    "reference-stderr-noise",
                ] {
                    println!("{name:<22} built-in    {}", model::PROTOCOL_VERSION);
                }
                for name in ["mem0", "letta", "zep"] {
                    println!("{name:<22} python      {}", model::PROTOCOL_VERSION);
                }
            }
        },
        Command::Expand {
            input,
            output,
            allow_network,
        } => {
            runner::expand_scenario(&input, &output, allow_network)?;
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
            let bundle_hash = evidence::refresh_bundle(&bundle)?;
            println!(
                "regenerated report.html and junit.xml in {}",
                bundle.display()
            );
            println!("bundle hash: {bundle_hash}");
        }
        Command::Matrix { command } => match command {
            MatrixCommand::Validate { path } => {
                validate_matrix(&path)?;
                println!("valid matrix: {}", path.display());
            }
        },
    }
    Ok(())
}

fn validate_matrix(path: &PathBuf) -> Result<()> {
    let value: Value = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("invalid JSON in {}", path.display()))?;
    let rows = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("matrix results must be an array"))?;
    for row in rows {
        let status = row
            .get("status")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("matrix row is missing status"))?;
        if !["PASS", "FAIL", "SKIP", "UNKNOWN"].contains(&status) {
            bail!("unsupported matrix status '{status}'");
        }
        for field in ["adapter", "backend", "profile", "evidence"] {
            if row
                .get(field)
                .and_then(Value::as_str)
                .unwrap_or("")
                .is_empty()
            {
                bail!("matrix row is missing {field}");
            }
        }
    }
    Ok(())
}
