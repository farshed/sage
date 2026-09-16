use anyhow::{anyhow, bail, Context, Result};
use sage_protocol::{read_async, DATA, DONE, ERROR, READY};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;

pub enum Event {
    Data(Vec<u8>),
    Done,
}

pub type Output = mpsc::Receiver<Result<Event>>;

struct Job {
    data: Vec<u8>,
    output: mpsc::Sender<Result<Event>>,
    cancel: CancellationToken,
}

#[derive(Clone)]
pub struct Worker {
    jobs: mpsc::Sender<Job>,
}

#[derive(Clone)]
pub struct WorkerConfig {
    pub name: String,
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub timeout: Duration,
}

struct Session {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
}

impl Session {
    async fn start(config: &WorkerConfig) -> Result<Self> {
        let mut child = Command::new(&config.executable)
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| {
                format!(
                    "Cannot start {} at {}",
                    config.name,
                    config.executable.display()
                )
            })?;
        let mut session = Self {
            input: child.stdin.take().context("Missing worker stdin")?,
            output: child.stdout.take().context("Missing worker stdout")?,
            child,
        };
        let ready = tokio::time::timeout(config.timeout, read_async(&mut session.output))
            .await
            .with_context(|| format!("{} startup timed out", config.name))??;
        if !matches!(ready, Some((READY, _))) {
            bail!("{} failed to become ready", config.name);
        }
        Ok(session)
    }

    async fn run(&mut self, job: &Job) -> Result<()> {
        self.input
            .write_all(&sage_protocol::encode(DATA, &job.data)?)
            .await?;
        self.input.flush().await?;
        while let Some((kind, bytes)) = read_async(&mut self.output).await? {
            let event = match kind {
                DATA => Ok(Event::Data(bytes)),
                DONE => Ok(Event::Done),
                ERROR => Err(anyhow!(String::from_utf8_lossy(&bytes).into_owned())),
                _ => bail!("Unexpected worker frame: {kind}"),
            };
            tokio::select! {
                biased;
                _ = job.cancel.cancelled() => {},
                _ = job.output.closed() => {},
                _ = job.output.send(event) => {},
            }
            if kind == DONE || kind == ERROR {
                return Ok(());
            }
        }
        bail!("Speech worker exited before completing its response")
    }

    async fn stop(mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

impl Worker {
    pub async fn start(config: WorkerConfig, shutdown: CancellationToken) -> Result<Self> {
        let initial = Session::start(&config).await?;
        let (jobs, mut queue) = mpsc::channel::<Job>(16);
        tokio::spawn(async move {
            let mut session = Some(initial);
            loop {
                let job = tokio::select! {
                    biased;
                    _ = shutdown.cancelled() => break,
                    job = queue.recv() => match job { Some(job) => job, None => break },
                };
                if job.cancel.is_cancelled() || job.output.is_closed() {
                    continue;
                }
                let result = tokio::select! {
                    biased;
                    _ = shutdown.cancelled() => break,
                    result = tokio::time::timeout(config.timeout, async {
                        if session.is_none() { session = Some(Session::start(&config).await?); }
                        session.as_mut().unwrap().run(&job).await
                    }) => result.unwrap_or_else(|_| Err(anyhow!("{} request timed out", config.name))),
                };
                if let Err(error) = result {
                    if let Some(active) = session.take() {
                        active.stop().await;
                    }
                    tokio::select! {
                        _ = shutdown.cancelled() => break,
                        _ = job.cancel.cancelled() => {},
                        _ = job.output.send(Err(error)) => {},
                    }
                }
            }
            if let Some(active) = session {
                active.stop().await;
            }
        });
        Ok(Self { jobs })
    }

    pub async fn request(&self, data: Vec<u8>, cancel: CancellationToken) -> Result<Output> {
        let (output, receiver) = mpsc::channel(2);
        tokio::select! {
            biased;
            _ = cancel.cancelled() => bail!("Request cancelled"),
            result = self.jobs.send(Job { data, output, cancel: cancel.clone() }) => result.context("Speech worker is closed")?,
        }
        Ok(receiver)
    }
}
