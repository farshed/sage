use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use std::path::Path;
use tokio::io::AsyncWriteExt;

const KOKORO_URL: &str = "https://huggingface.co/hexgrad/Kokoro-82M/resolve/f4e895d7c38097d813b7a6939f335200dcd9b21e/kokoro-v0_19.onnx";
const VOICES_URL: &str =
    "https://github.com/farshed/sage/releases/download/voices.json_v0.1/voices.json";
const PARAKEET_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2";
const PARAKEET_FILES: [&str; 4] = [
    "encoder.int8.onnx",
    "decoder.int8.onnx",
    "joiner.int8.onnx",
    "tokens.txt",
];

pub async fn models(directory: &Path, parakeet: &Path) -> Result<()> {
    tokio::fs::create_dir_all(directory).await?;
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()?;
    let kokoro = directory.join("kokoro-v0_19.onnx");
    let voices = directory.join("voices.json");
    tokio::try_join!(
        file(&client, KOKORO_URL, &kokoro),
        file(&client, VOICES_URL, &voices),
        parakeet_model(&client, directory, parakeet),
    )?;
    eprintln!("Speech models are ready in {}", directory.display());
    Ok(())
}

async fn file(client: &reqwest::Client, url: &str, target: &Path) -> Result<()> {
    if target.metadata().is_ok_and(|metadata| metadata.len() > 0) {
        return Ok(());
    }
    eprintln!("Downloading {}", target.display());
    let partial = target.with_extension(format!(
        "{}.part",
        target.extension().unwrap_or_default().to_string_lossy()
    ));
    let mut response = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes_stream();
    let mut output = tokio::fs::File::create(&partial).await?;
    while let Some(bytes) = response.next().await {
        output.write_all(&bytes?).await?;
    }
    output.flush().await?;
    drop(output);
    tokio::fs::rename(partial, target).await?;
    Ok(())
}

async fn parakeet_model(client: &reqwest::Client, directory: &Path, target: &Path) -> Result<()> {
    if PARAKEET_FILES
        .iter()
        .all(|name| target.join(name).is_file())
    {
        return Ok(());
    }
    let archive = directory.join(format!("{}.tar.bz2", crate::config::PARAKEET_MODEL));
    file(client, PARAKEET_URL, &archive).await?;
    let archive_path = archive.clone();
    let staging = directory.join("parakeet-extract.part");
    let output = staging.clone();
    let extracted = tokio::task::spawn_blocking(move || -> Result<()> {
        std::fs::create_dir_all(&output)?;
        let source = std::fs::File::open(archive_path)?;
        tar::Archive::new(bzip2::read::BzDecoder::new(source)).unpack(output)?;
        Ok(())
    })
    .await?;
    if let Err(error) = extracted {
        let _ = tokio::fs::remove_file(&archive).await;
        return Err(error.context("Cannot extract Parakeet; run download-models again"));
    }
    let extracted = staging.join(crate::config::PARAKEET_MODEL);
    if !PARAKEET_FILES
        .iter()
        .all(|name| extracted.join(name).is_file())
    {
        bail!("Parakeet archive is missing model files");
    }
    tokio::fs::create_dir_all(target).await?;
    for name in PARAKEET_FILES {
        tokio::fs::rename(extracted.join(name), target.join(name))
            .await
            .with_context(|| format!("Cannot install {name}"))?;
    }
    if extracted.join("test_wavs").is_dir() && !target.join("test_wavs").exists() {
        tokio::fs::rename(extracted.join("test_wavs"), target.join("test_wavs")).await?;
    }
    tokio::fs::remove_dir_all(staging).await?;
    tokio::fs::remove_file(archive).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[tokio::test]
    async fn downloads_atomically_replaces_partial_files_and_skips_cached_files() {
        let count = Arc::new(AtomicUsize::new(0));
        let requests = count.clone();
        let app = Router::new().route(
            "/model",
            get(move || {
                requests.fetch_add(1, Ordering::SeqCst);
                async { "model bytes" }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/model", listener.local_addr().unwrap());
        let shutdown = tokio_util::sync::CancellationToken::new();
        let _guard = shutdown.clone().drop_guard();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
                .unwrap();
        });
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("model.onnx");
        let partial = directory.path().join("model.onnx.part");
        tokio::fs::write(&partial, b"incomplete previous download")
            .await
            .unwrap();
        let client = reqwest::Client::new();
        file(&client, &url, &target).await.unwrap();
        assert_eq!(tokio::fs::read(&target).await.unwrap(), b"model bytes");
        assert!(!partial.exists());
        file(&client, &url, &target).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        let missing = directory.path().join("missing.onnx");
        assert!(file(&client, &url.replace("/model", "/missing"), &missing)
            .await
            .is_err());
        assert!(!missing.exists());
    }
}
