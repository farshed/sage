use anyhow::Result;
use clap::Parser;
use sage::{
    config::{Cli, Command},
    download,
    llm::Llm,
    server,
    speech::Speech,
};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    if matches!(cli.command, Some(Command::DownloadModels)) {
        return download::models(&cli.models_dir, &cli.parakeet_dir()).await;
    }
    let shutdown = CancellationToken::new();
    let _guard = shutdown.clone().drop_guard();
    let llm = if cli.command.is_none() {
        Some(Llm::from_env()?)
    } else {
        None
    };
    eprintln!("Loading speech models...");
    let speech = Speech::load(&cli, shutdown.clone()).await?;
    eprintln!("Speech models are ready.");
    if matches!(cli.command, Some(Command::CheckSpeech)) {
        return server::check_speech(speech, &cli.parakeet_dir(), shutdown).await;
    }
    let listener = tokio::net::TcpListener::bind((cli.host.as_str(), cli.port)).await?;
    eprintln!("Open http://{} in your browser", listener.local_addr()?);
    let app = server::router(speech, llm.unwrap(), shutdown.clone());
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            stop_signal().await;
            shutdown.cancel();
        })
        .await?;
    Ok(())
}

async fn stop_signal() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Cannot register SIGTERM");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = term.recv() => {} }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
