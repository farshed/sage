use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

pub const PARAKEET_MODEL: &str = "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8";
pub const SYSTEM_PROMPT: &str = "Respond in plain-text only. Keep your responses short and succinct. Ignore any unusual endings like \"Bye!\" and \"Thank you!\". Provide a response that is written to be read exactly and literally as it is written. For example, instead of writing '$10-50 million', write '10 to 50 million dollars'. Avoid using symbols or abbreviations that might be misinterpreted when read literally.";

#[derive(Parser)]
#[command(
    version,
    about = "Talk to language models using local speech processing"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
    #[arg(long, global = true, env = "SAGE_MODELS_DIR", default_value = "models")]
    pub models_dir: PathBuf,
    #[arg(long, global = true, env = "SAGE_WORKER_DIR")]
    pub worker_dir: Option<PathBuf>,
    #[arg(long, env = "SAGE_HOST", default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, env = "PORT", default_value_t = 3000)]
    pub port: u16,
}

#[derive(Subcommand)]
pub enum Command {
    DownloadModels,
    CheckSpeech,
}

impl Cli {
    pub fn worker_dir(&self) -> Result<PathBuf> {
        resolve_worker_dir(&std::env::current_exe()?, self.worker_dir.as_deref())
    }

    pub fn parakeet_dir(&self) -> PathBuf {
        std::env::var_os("PARAKEET_MODEL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.models_dir.join(PARAKEET_MODEL))
    }
}

fn resolve_worker_dir(executable: &Path, configured: Option<&Path>) -> Result<PathBuf> {
    let directory = executable
        .parent()
        .context("Cannot locate Sage's directory")?;
    let selected = configured.unwrap_or(directory);
    let complete = |path: &Path| {
        ["parakeet", "kokoro"].iter().all(|name| {
            path.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
                .is_file()
        })
    };
    if complete(selected) {
        return Ok(selected.to_path_buf());
    }
    if configured.is_none() && directory.file_name().is_some_and(|name| name == "debug") {
        if let Some(target) = directory.parent() {
            let release = target.join("release");
            if complete(&release) {
                return Ok(release);
            }
        }
    }
    let command = if directory.file_name().is_some_and(|name| name == "debug") {
        "cargo build --workspace"
    } else {
        "cargo build --release --workspace"
    };
    if configured.is_some() {
        bail!("Speech workers are missing from {}. Run {command}, then set --worker-dir or SAGE_WORKER_DIR to the directory containing both parakeet and kokoro.", selected.display());
    }
    bail!("Speech workers are missing from {}. Run {command} to build both parakeet and kokoro. cargo run only builds the Sage server.", selected.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workers(directory: &Path) {
        std::fs::create_dir_all(directory).unwrap();
        for name in ["parakeet", "kokoro"] {
            std::fs::write(
                directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
                [],
            )
            .unwrap();
        }
    }

    #[test]
    fn debug_server_finds_release_workers_and_prefers_complete_local_workers() {
        let target = tempfile::tempdir().unwrap();
        let debug = target.path().join("debug");
        let release = target.path().join("release");
        let executable = debug.join("sage");
        workers(&release);
        assert_eq!(resolve_worker_dir(&executable, None).unwrap(), release);
        workers(&debug);
        assert_eq!(resolve_worker_dir(&executable, None).unwrap(), debug);
        std::fs::remove_file(debug.join(format!("kokoro{}", std::env::consts::EXE_SUFFIX)))
            .unwrap();
        assert_eq!(resolve_worker_dir(&executable, None).unwrap(), release);
    }

    #[test]
    fn explicit_worker_directory_is_authoritative() {
        let target = tempfile::tempdir().unwrap();
        let debug = target.path().join("debug");
        let configured = target.path().join("custom");
        workers(&debug);
        workers(&target.path().join("release"));
        let executable = debug.join("sage");
        assert!(resolve_worker_dir(&executable, Some(&configured)).is_err());
        workers(&configured);
        assert_eq!(
            resolve_worker_dir(&executable, Some(&configured)).unwrap(),
            configured
        );
    }

    #[test]
    fn missing_workers_report_the_matching_build_profile() {
        let target = tempfile::tempdir().unwrap();
        let debug = target.path().join("debug");
        let release = target.path().join("release");
        assert!(resolve_worker_dir(&debug.join("sage"), None)
            .unwrap_err()
            .to_string()
            .contains("cargo build --workspace"));
        workers(&debug);
        assert!(resolve_worker_dir(&release.join("sage"), None)
            .unwrap_err()
            .to_string()
            .contains("cargo build --release --workspace"));
    }
}
