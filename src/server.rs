use crate::{
    chunks::SpeechChunks,
    llm::{Llm, Message, Role},
    speech::Speech,
    worker::Event,
};
use anyhow::{bail, Context, Result};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use sage_protocol::{DATA, DONE, ERROR, METADATA};
use serde_json::json;
use std::{
    convert::Infallible,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct Services {
    speech: Speech,
    llm: Llm,
    shutdown: CancellationToken,
}

pub fn router(speech: Speech, llm: Llm, shutdown: CancellationToken) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    include_str!("../public/ui.html"),
                )
            }),
        )
        .route("/public/{file}", get(asset))
        .route("/chat", post(chat))
        .layer(DefaultBodyLimit::max(17 * 1024 * 1024))
        .with_state(Arc::new(Services {
            speech,
            llm,
            shutdown,
        }))
}

async fn asset(Path(file): Path<String>) -> Response {
    let content = match file.as_str() {
        "chat.js" => include_str!("../public/chat.js"),
        "protocol.js" => include_str!("../public/protocol.js"),
        "audio-buffer.js" => include_str!("../public/audio-buffer.js"),
        "audio-worklet.js" => include_str!("../public/audio-worklet.js"),
        "player.js" => include_str!("../public/player.js"),
        "wav-worker.js" => include_str!("../public/wav-worker.js"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        content,
    )
        .into_response()
}

async fn upload(mut form: Multipart) -> Result<(Vec<u8>, Vec<Message>)> {
    let mut wav = None;
    let mut messages = Vec::new();
    while let Some(field) = form.next_field().await? {
        match field.name() {
            Some("file") => {
                if field.file_name().is_none() {
                    bail!("Send a WAV recording smaller than 16 MB.");
                }
                let data = field.bytes().await?;
                if data.is_empty() || data.len() > 16 * 1024 * 1024 {
                    bail!("Send a WAV recording smaller than 16 MB.");
                }
                wav = Some(data.to_vec());
            }
            Some("messages") => {
                let data = field.bytes().await?;
                if data.len() > 1024 * 1024 {
                    bail!("Conversation history is too large.");
                }
                messages =
                    serde_json::from_slice(&data).context("Invalid conversation history.")?;
            }
            _ => {}
        }
    }
    Ok((
        wav.context("Send a WAV recording smaller than 16 MB.")?,
        messages,
    ))
}

async fn chat(State(services): State<Arc<Services>>, form: Multipart) -> Response {
    let (wav, history) = match upload(form).await {
        Ok(input) => input,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":error.to_string()})),
            )
                .into_response()
        }
    };
    let cancel = services.shutdown.child_token();
    let guard = cancel.clone().drop_guard();
    let (sender, mut receiver) = mpsc::channel::<Bytes>(4);
    tokio::spawn(async move {
        let generation = cancel.child_token();
        let _generation_guard = generation.clone().drop_guard();
        let result = tokio::select! {
            biased;
            _ = cancel.cancelled() => return,
            result = tokio::time::timeout(Duration::from_secs(300), turn(&services, wav, history, &sender, generation.clone())) => {
                result.unwrap_or_else(|_| Err(anyhow::anyhow!("Voice response timed out")))
            },
        };
        generation.cancel();
        if let Err(error) = result {
            eprintln!("Voice response failed: {error:#}");
            tokio::select! {
                _ = cancel.cancelled() => {},
                _ = send_json(&sender, ERROR, json!({"error":error.to_string()})) => {},
            }
        }
    });
    let stream = async_stream::stream! {
        let _guard = guard;
        while let Some(bytes) = receiver.recv().await { yield Ok::<_, Infallible>(bytes); }
    };
    (
        [
            (header::CONTENT_TYPE, "application/x-sage-audio"),
            (header::CACHE_CONTROL, "no-store"),
            (header::HeaderName::from_static("x-accel-buffering"), "no"),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

async fn turn(
    services: &Services,
    wav: Vec<u8>,
    mut messages: Vec<Message>,
    sender: &mpsc::Sender<Bytes>,
    cancel: CancellationToken,
) -> Result<()> {
    let started = Instant::now();
    let transcript = services.speech.transcribe(wav, cancel.clone()).await?;
    let transcription_ms = started.elapsed().as_millis();
    if transcript.is_empty() {
        bail!("No speech detected.");
    }
    messages.push(Message {
        role: Role::User,
        content: transcript,
    });
    send_json(
        sender,
        METADATA,
        json!({"sampleRate":24000,"channels":1,"encoding":"f32le"}),
    )
    .await?;
    let mut text = services.llm.generate(messages.clone(), cancel.clone());
    let mut chunks = SpeechChunks::default();
    let mut reply = String::new();
    let mut first_audio_ms = None;
    while let Some(delta) = text.recv().await {
        let delta = delta?;
        reply.push_str(&delta);
        if reply.len() > 1024 * 1024 {
            bail!("LLM response is too large");
        }
        for chunk in chunks.push(&delta, false) {
            speak(
                services,
                chunk,
                sender,
                cancel.clone(),
                started,
                &mut first_audio_ms,
            )
            .await?;
        }
    }
    for chunk in chunks.push("", true) {
        speak(
            services,
            chunk,
            sender,
            cancel.clone(),
            started,
            &mut first_audio_ms,
        )
        .await?;
    }
    let first_audio_ms = first_audio_ms.context("No speech was generated.")?;
    let timings = json!({"transcriptionMs":transcription_ms,"firstAudioMs":first_audio_ms,"totalMs":started.elapsed().as_millis()});
    eprintln!("Voice response: {timings}");
    messages.push(Message {
        role: Role::Assistant,
        content: reply,
    });
    send_json(sender, DONE, json!({"messages":messages,"timings":timings})).await
}

async fn speak(
    services: &Services,
    text: String,
    sender: &mpsc::Sender<Bytes>,
    cancel: CancellationToken,
    started: Instant,
    first_audio_ms: &mut Option<u128>,
) -> Result<()> {
    let mut audio = services.speech.synthesize(text, cancel).await?;
    while let Some(event) = audio.recv().await {
        match event? {
            Event::Data(bytes) => {
                if !bytes.len().is_multiple_of(4) {
                    bail!("Invalid speech audio.");
                }
                if !bytes.is_empty() {
                    first_audio_ms.get_or_insert_with(|| started.elapsed().as_millis());
                    send(sender, DATA, &bytes).await?;
                }
            }
            Event::Done => return Ok(()),
        }
    }
    bail!("Speech synthesis ended unexpectedly")
}

async fn send(sender: &mpsc::Sender<Bytes>, kind: u8, data: &[u8]) -> Result<()> {
    sender
        .send(Bytes::from(sage_protocol::encode(kind, data)?))
        .await
        .context("Request cancelled")
}

async fn send_json(sender: &mpsc::Sender<Bytes>, kind: u8, value: serde_json::Value) -> Result<()> {
    send(sender, kind, &serde_json::to_vec(&value)?).await
}

pub async fn check_speech(
    speech: Speech,
    model: &std::path::Path,
    shutdown: CancellationToken,
) -> Result<()> {
    use futures_util::StreamExt;
    let wav = tokio::fs::read(model.join("test_wavs/en.wav"))
        .await
        .context(
            "Missing Parakeet test_wavs/en.wav. Run download-models with a fresh model directory.",
        )?;
    for run in 1..=2 {
        let start = Instant::now();
        let transcript = speech
            .transcribe(wav.clone(), shutdown.child_token())
            .await?;
        if transcript.is_empty() {
            bail!("Parakeet returned an empty transcript");
        }
        eprintln!(
            "Transcription {run}: {} ms, {transcript}",
            start.elapsed().as_millis()
        );
    }
    if speech
        .transcribe(b"invalid wav".to_vec(), shutdown.child_token())
        .await
        .is_ok()
    {
        bail!("Invalid WAV was accepted");
    }
    let first_audio = Arc::new(tokio::sync::Notify::new());
    let notify = first_audio.clone();
    let mock = Router::new().route("/api/chat", post(move || {
        let notify = notify.clone();
        async move {
            Body::from_stream(async_stream::stream! {
                yield Ok::<_, Infallible>(Bytes::from_static(b"{\"message\":{\"content\":\"Hello there! \"},\"done\":false}\n"));
                notify.notified().await;
                yield Ok(Bytes::from_static(b"{\"message\":{\"content\":\"The Rust speech pipeline works.\"},\"done\":true}\n"));
            })
        }
    }));
    let mock_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let llm = Llm::ollama(
        format!("http://{}", mock_listener.local_addr()?),
        "test".into(),
    )?;
    let stop = shutdown.clone();
    let mock_task = tokio::spawn(async move {
        axum::serve(mock_listener, mock)
            .with_graceful_shutdown(stop.cancelled_owned())
            .await
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}/chat", listener.local_addr()?);
    let app = router(speech, llm, shutdown.clone());
    let stop = shutdown.clone();
    let server_task = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(stop.cancelled_owned())
            .await
    });
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(wav).file_name("input.wav"),
        )
        .text("messages", "[]");
    let response = reqwest::Client::new()
        .post(url)
        .multipart(form)
        .timeout(Duration::from_secs(120))
        .send()
        .await?
        .error_for_status()?;
    let mut stream = response.bytes_stream();
    let mut pending = Vec::new();
    let mut audio_bytes = 0;
    let mut complete = false;
    let started = Instant::now();
    while let Some(bytes) = stream.next().await {
        pending.extend_from_slice(&bytes?);
        while pending.len() >= 5 {
            let length = u32::from_le_bytes(pending[1..5].try_into()?) as usize;
            if length > sage_protocol::MAX_FRAME_BYTES {
                bail!("Invalid HTTP audio frame");
            }
            if pending.len() < 5 + length {
                break;
            }
            let kind = pending[0];
            let data = &pending[5..5 + length];
            match kind {
                DATA => {
                    if !length.is_multiple_of(4) {
                        bail!("Invalid PCM audio");
                    }
                    if audio_bytes == 0 {
                        eprintln!(
                            "First HTTP audio: {} ms, before LLM completion",
                            started.elapsed().as_millis()
                        );
                        first_audio.notify_one();
                    }
                    audio_bytes += length;
                }
                DONE => {
                    complete = true;
                    eprintln!("{}", String::from_utf8_lossy(data));
                }
                ERROR => bail!("{}", String::from_utf8_lossy(data)),
                METADATA => {
                    let metadata: serde_json::Value = serde_json::from_slice(data)?;
                    if metadata["sampleRate"] != 24000 {
                        bail!("Invalid sample rate");
                    }
                }
                _ => bail!("Invalid HTTP frame kind"),
            }
            pending.drain(..5 + length);
        }
    }
    if !complete || audio_bytes == 0 || !pending.is_empty() {
        bail!("Incomplete speech response");
    }
    shutdown.cancel();
    server_task.await??;
    mock_task.await??;
    eprintln!("Speech check passed: {audio_bytes} PCM bytes over HTTP. No LLM service was called.");
    Ok(())
}
