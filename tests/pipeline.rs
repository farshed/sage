use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use sage::{
    llm::Llm,
    server,
    speech::Speech,
    worker::{Event, Output, Worker, WorkerConfig},
};
use std::{path::PathBuf, sync::OnceLock, time::Duration};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

fn executable() -> PathBuf {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY
        .get_or_init(|| {
            let directory = tempfile::tempdir().unwrap().into_path();
            let binary = directory.join(format!("worker{}", std::env::consts::EXE_SUFFIX));
            let status = std::process::Command::new("rustc")
                .args(["--edition=2021", "tests/fixtures/worker.rs", "-o"])
                .arg(&binary)
                .status()
                .unwrap();
            assert!(status.success());
            binary
        })
        .clone()
}

async fn worker(mode: &str, timeout: Duration, shutdown: CancellationToken) -> Worker {
    Worker::start(
        WorkerConfig {
            name: mode.into(),
            executable: executable(),
            args: vec![mode.into()],
            timeout,
        },
        shutdown,
    )
    .await
    .unwrap()
}

async fn collect(mut output: Output) -> anyhow::Result<Vec<Vec<u8>>> {
    let mut data = Vec::new();
    while let Some(event) = output.recv().await {
        match event? {
            Event::Data(bytes) => data.push(bytes),
            Event::Done => return Ok(data),
        }
    }
    anyhow::bail!("Missing completion")
}

#[tokio::test]
async fn workers_reuse_processes_serialize_requests_and_recover() {
    let shutdown = CancellationToken::new();
    let _guard = shutdown.clone().drop_guard();
    let worker = worker("echo", Duration::from_secs(1), shutdown).await;
    let request = |data: &'static [u8]| worker.request(data.to_vec(), CancellationToken::new());
    let first = request(b"slow").await.unwrap();
    let second = request(b"second").await.unwrap();
    let first = collect(first).await.unwrap();
    let second = collect(second).await.unwrap();
    assert_eq!(first[0], second[0]);
    assert_eq!(first[1], b"slow");
    assert_eq!(second[1], b"second");
    assert!(collect(request(b"bad").await.unwrap()).await.is_err());
    assert_eq!(
        collect(request(b"after-error").await.unwrap())
            .await
            .unwrap()[0],
        first[0]
    );
    assert!(collect(request(b"crash").await.unwrap()).await.is_err());
    let restarted = collect(request(b"after-crash").await.unwrap())
        .await
        .unwrap();
    assert_ne!(restarted[0], first[0]);
    assert!(collect(request(b"hang").await.unwrap()).await.is_err());
    assert_ne!(
        collect(request(b"after-timeout").await.unwrap())
            .await
            .unwrap()[0],
        restarted[0]
    );
}

#[tokio::test]
async fn cancelled_and_abandoned_requests_do_not_corrupt_the_next_turn() {
    let shutdown = CancellationToken::new();
    let _guard = shutdown.clone().drop_guard();
    let worker = worker("echo", Duration::from_secs(2), shutdown).await;
    let cancel = CancellationToken::new();
    let first = worker
        .request(b"slow".to_vec(), cancel.clone())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    cancel.cancel();
    drop(first);
    let next = worker
        .request(b"second".to_vec(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(collect(next).await.unwrap()[1], b"second");
    let abandoned = worker
        .request(b"slow".to_vec(), CancellationToken::new())
        .await
        .unwrap();
    drop(abandoned);
    let next = worker
        .request(b"third".to_vec(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(collect(next).await.unwrap()[1], b"third");
}

async fn app(shutdown: CancellationToken, response: &'static str) -> Router {
    let asr = worker("asr", Duration::from_secs(2), shutdown.clone()).await;
    let tts = worker("tts", Duration::from_secs(2), shutdown.clone()).await;
    let mock = Router::new().route(
        "/api/chat",
        post(
            move |axum::Json(body): axum::Json<serde_json::Value>| async move {
                assert_eq!(body["messages"][0]["role"], "system");
                assert_eq!(body["messages"][1]["content"], "Hello");
                assert_eq!(body["stream"], true);
                response
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let llm = Llm::ollama(
        format!("http://{}", listener.local_addr().unwrap()),
        "test".into(),
    )
    .unwrap();
    let stop = shutdown.clone();
    tokio::spawn(async move {
        axum::serve(listener, mock)
            .with_graceful_shutdown(stop.cancelled_owned())
            .await
            .unwrap();
    });
    server::router(Speech::new(asr, tts), llm, shutdown)
}

fn request(history: &str, file: bool) -> Request<Body> {
    let mut data =
        format!("--sage\r\nContent-Disposition: form-data; name=\"messages\"\r\n\r\n{history}\r\n");
    if file {
        data.push_str("--sage\r\nContent-Disposition: form-data; name=\"file\"; filename=\"input.wav\"\r\nContent-Type: audio/wav\r\n\r\nRIFF\r\n");
    }
    data.push_str("--sage--\r\n");
    Request::post("/chat")
        .header("content-type", "multipart/form-data; boundary=sage")
        .body(Body::from(data))
        .unwrap()
}

#[tokio::test]
async fn validates_uploads_and_embeds_browser_assets() {
    let shutdown = CancellationToken::new();
    let _guard = shutdown.clone().drop_guard();
    let app = app(shutdown, "").await;
    for (history, file) in [
        ("[]", false),
        ("null", true),
        (r#"[{"role":"system","content":"bad"}]"#, true),
    ] {
        let response = app.clone().oneshot(request(history, file)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    for path in [
        "/",
        "/public/chat.js",
        "/public/audio-worklet.js",
        "/public/wav-worker.js",
    ] {
        let response = app
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .is_empty());
    }
    assert_eq!(
        app.oneshot(Request::get("/public/.env").body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn streams_pcm_and_commits_history_only_after_completion() {
    let shutdown = CancellationToken::new();
    let _guard = shutdown.clone().drop_guard();
    let app = app(
        shutdown,
        "{\"message\":{\"content\":\"Hello there! Another sentence.\"},\"done\":true}\n",
    )
    .await;
    let response = app.oneshot(request("[]", true)).await.unwrap();
    assert_eq!(
        response.headers()["content-type"],
        "application/x-sage-audio"
    );
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let mut reader = bytes.as_ref();
    let mut frames = Vec::new();
    while let Some(frame) = sage_protocol::read_async(&mut reader).await.unwrap() {
        frames.push(frame);
    }
    assert_eq!(frames.first().unwrap().0, sage_protocol::METADATA);
    assert_eq!(
        frames
            .iter()
            .filter(|frame| frame.0 == sage_protocol::DATA)
            .count(),
        40
    );
    let last = frames.last().unwrap();
    assert_eq!(last.0, sage_protocol::DONE);
    let result: serde_json::Value = serde_json::from_slice(&last.1).unwrap();
    assert_eq!(
        result["messages"][1]["content"],
        "Hello there! Another sentence."
    );
}

#[tokio::test]
async fn incomplete_and_failed_llm_streams_never_commit_history() {
    for data in [
        "{\"message\":{\"content\":\"Partial\"},\"done\":false}\n",
        "{\"error\":\"Provider unavailable\"}\n",
    ] {
        let shutdown = CancellationToken::new();
        let _guard = shutdown.clone().drop_guard();
        let app = app(shutdown, data).await;
        let response = app.oneshot(request("[]", true)).await.unwrap();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let mut reader = bytes.as_ref();
        let mut kinds = Vec::new();
        while let Some((kind, _)) = sage_protocol::read_async(&mut reader).await.unwrap() {
            kinds.push(kind);
        }
        assert_eq!(kinds, [sage_protocol::METADATA, sage_protocol::ERROR]);
    }
}

#[tokio::test]
async fn dropping_http_body_cancels_upstream_generation() {
    let shutdown = CancellationToken::new();
    let _guard = shutdown.clone().drop_guard();
    let entered = std::sync::Arc::new(tokio::sync::Notify::new());
    let closed = CancellationToken::new();
    let started = entered.clone();
    let disconnected = closed.clone();
    let mock = Router::new().route("/api/chat", post(move || {
        let started = started.clone();
        let guard = disconnected.clone().drop_guard();
        async move {
            Body::from_stream(async_stream::stream! {
                let _guard = guard;
                started.notify_one();
                yield Ok::<_, std::convert::Infallible>("{\"message\":{\"content\":\"Hello there! \"},\"done\":false}\n");
                std::future::pending::<()>().await;
            })
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let llm = Llm::ollama(
        format!("http://{}", listener.local_addr().unwrap()),
        "test".into(),
    )
    .unwrap();
    let stop = shutdown.clone();
    tokio::spawn(async move {
        axum::serve(listener, mock)
            .with_graceful_shutdown(stop.cancelled_owned())
            .await
            .unwrap();
    });
    let speech = Speech::new(
        worker("asr", Duration::from_secs(2), shutdown.clone()).await,
        worker("tts", Duration::from_secs(2), shutdown.clone()).await,
    );
    let app = server::router(speech, llm, shutdown);
    let response = app.oneshot(request("[]", true)).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), entered.notified())
        .await
        .unwrap();
    drop(response);
    tokio::time::timeout(Duration::from_secs(2), closed.cancelled())
        .await
        .unwrap();
}
