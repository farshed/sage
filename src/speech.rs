use crate::{
    config::Cli,
    worker::{Event, Output, Worker, WorkerConfig},
};
use anyhow::{bail, Context, Result};
use std::{path::Path, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Speech {
    parakeet: Worker,
    kokoro: Worker,
}

impl Speech {
    pub fn new(parakeet: Worker, kokoro: Worker) -> Self {
        Self { parakeet, kokoro }
    }

    pub async fn load(cli: &Cli, shutdown: CancellationToken) -> Result<Self> {
        let directory = cli.worker_dir()?;
        let model = cli.models_dir.join("kokoro-v0_19.onnx");
        for path in [
            model.clone(),
            cli.models_dir.join("voices.json"),
            cli.parakeet_dir().join("tokens.txt"),
        ] {
            if !path.is_file() {
                bail!(
                    "Missing {}. Run sage download-models first.",
                    path.display()
                );
            }
        }
        let executable =
            |name: &str| directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        let config = |name: &str, args| WorkerConfig {
            name: name.to_string(),
            executable: executable(name),
            args,
            timeout: Duration::from_secs(120),
        };
        let loaded = tokio::try_join!(
            Worker::start(
                config(
                    "parakeet",
                    vec!["--worker".into(), absolute(&cli.parakeet_dir())?]
                ),
                shutdown.clone()
            ),
            Worker::start(
                config(
                    "kokoro",
                    vec!["--worker".into(), "--model".into(), absolute(&model)?]
                ),
                shutdown.clone()
            ),
        );
        match loaded {
            Ok((parakeet, kokoro)) => Ok(Self { parakeet, kokoro }),
            Err(error) => {
                shutdown.cancel();
                Err(error)
            }
        }
    }

    pub async fn transcribe(&self, wav: Vec<u8>, cancel: CancellationToken) -> Result<String> {
        let mut output = self.parakeet.request(wav, cancel.clone()).await?;
        let mut text = Vec::new();
        loop {
            let next = tokio::select! {
                _ = cancel.cancelled() => bail!("Request cancelled"),
                next = output.recv() => next.context("Transcription ended unexpectedly")??,
            };
            match next {
                Event::Data(bytes) => text.extend(bytes),
                Event::Done => {
                    return Ok(String::from_utf8(text)?
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "))
                }
            }
        }
    }

    pub async fn synthesize(&self, text: String, cancel: CancellationToken) -> Result<Output> {
        self.kokoro.request(text.into_bytes(), cancel).await
    }
}

fn absolute(path: &Path) -> Result<String> {
    Ok(path.canonicalize()?.to_string_lossy().into_owned())
}
