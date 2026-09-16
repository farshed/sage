use crate::config::SYSTEM_PROMPT;
use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Format {
    OpenAi,
    Anthropic,
    Ollama,
}

#[derive(Clone)]
pub struct Llm {
    client: reqwest::Client,
    format: Format,
    endpoint: String,
    model: String,
    key: Option<String>,
}

impl Llm {
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|key| std::env::var(key).ok().filter(|value| !value.is_empty()))
    }

    fn from_lookup(env: impl Fn(&str) -> Option<String>) -> Result<Self> {
        if let Some(model) = env("OLLAMA_MODEL") {
            return Self::new(
                Format::Ollama,
                env("OLLAMA_BASE_URL").unwrap_or("http://localhost:11434".into()),
                "/api/chat",
                model,
                None,
            );
        }
        for (prefix, base, path, default, format) in [
            (
                "DEEPSEEK",
                "https://api.deepseek.com",
                "/chat/completions",
                "deepseek-chat",
                Format::OpenAi,
            ),
            (
                "ANTHROPIC",
                "https://api.anthropic.com/v1",
                "/messages",
                "claude-3-5-haiku-20241022",
                Format::Anthropic,
            ),
            (
                "OPENAI",
                "https://api.openai.com/v1",
                "/chat/completions",
                "gpt-4-turbo",
                Format::OpenAi,
            ),
            (
                "TOGETHER_AI",
                "https://api.together.xyz/v1",
                "/chat/completions",
                "deepseek-ai/DeepSeek-V3",
                Format::OpenAi,
            ),
        ] {
            if let Some(key) = env(&format!("{prefix}_API_KEY")) {
                return Self::new(
                    format,
                    env(&format!("{prefix}_BASE_URL")).unwrap_or(base.into()),
                    path,
                    env(&format!("{prefix}_MODEL")).unwrap_or(default.into()),
                    Some(key),
                );
            }
        }
        bail!("No LLM provider configured. Set OLLAMA_MODEL or a supported provider's API key in .env.")
    }

    fn new(
        format: Format,
        base: String,
        path: &str,
        model: String,
        key: Option<String>,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            format,
            endpoint: format!("{}{path}", base.trim_end_matches('/')),
            model,
            key,
        })
    }

    pub fn ollama(base: String, model: String) -> Result<Self> {
        Self::new(Format::Ollama, base, "/api/chat", model, None)
    }

    fn payload(&self, messages: &[Message]) -> Value {
        if self.format == Format::Anthropic {
            json!({"model":self.model,"system":SYSTEM_PROMPT,"max_tokens":1024,"messages":messages,"stream":true})
        } else {
            let mut conversation = vec![json!({"role":"system","content":SYSTEM_PROMPT})];
            conversation.extend(messages.iter().map(|message| json!(message)));
            json!({"model":self.model,"messages":conversation,"stream":true})
        }
    }

    pub fn generate(
        &self,
        messages: Vec<Message>,
        cancel: CancellationToken,
    ) -> mpsc::Receiver<Result<String>> {
        let (sender, receiver) = mpsc::channel(8);
        let llm = self.clone();
        tokio::spawn(async move {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {},
                _ = sender.closed() => {},
                result = llm.stream(&messages, &sender) => {
                    if let Err(error) = result {
                        tokio::select! {
                            _ = cancel.cancelled() => {},
                            _ = sender.send(Err(error)) => {},
                        }
                    }
                },
            }
        });
        receiver
    }

    async fn stream(
        &self,
        messages: &[Message],
        sender: &mpsc::Sender<Result<String>>,
    ) -> Result<()> {
        let mut request = self
            .client
            .post(&self.endpoint)
            .json(&self.payload(messages));
        if let Some(key) = &self.key {
            request = if self.format == Format::Anthropic {
                request
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
            } else {
                request.bearer_auth(key)
            };
        }
        let response = request.send().await?.error_for_status()?;
        let mut stream = response.bytes_stream();
        let mut decoder = Decoder::new(self.format);
        while let Some(bytes) = stream.next().await {
            for event in decoder.push(&bytes?)? {
                match event {
                    Delta::Text(text) => sender
                        .send(Ok(text))
                        .await
                        .map_err(|_| anyhow!("Request cancelled"))?,
                    Delta::Done => return Ok(()),
                    Delta::Error(message) => bail!("{message}"),
                }
            }
        }
        for event in decoder.finish()? {
            match event {
                Delta::Text(text) => sender
                    .send(Ok(text))
                    .await
                    .map_err(|_| anyhow!("Request cancelled"))?,
                Delta::Done => return Ok(()),
                Delta::Error(message) => bail!("{message}"),
            }
        }
        bail!("LLM stream ended before the response was complete")
    }
}

#[derive(Debug, PartialEq)]
enum Delta {
    Text(String),
    Done,
    Error(String),
}

struct Decoder {
    format: Format,
    pending: Vec<u8>,
    data: String,
}

impl Decoder {
    fn new(format: Format) -> Self {
        Self {
            format,
            pending: Vec::new(),
            data: String::new(),
        }
    }

    fn push(&mut self, bytes: &[u8]) -> Result<Vec<Delta>> {
        self.pending.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(end) = self.pending.iter().position(|&byte| byte == b'\n') {
            if end > 1024 * 1024 {
                bail!("LLM stream record is too large");
            }
            let line = String::from_utf8(self.pending.drain(..=end).collect())?;
            self.line(line.trim_end_matches(['\r', '\n']), &mut events)?;
        }
        if self.pending.len() > 1024 * 1024 {
            bail!("LLM stream record is too large");
        }
        Ok(events)
    }

    fn line(&mut self, line: &str, events: &mut Vec<Delta>) -> Result<()> {
        if self.format == Format::Ollama {
            if !line.trim().is_empty() {
                events.extend(self.parse(line)?);
            }
        } else if line.is_empty() {
            if !self.data.is_empty() {
                events.extend(self.parse(self.data.trim_end_matches('\n'))?);
                self.data.clear();
            }
        } else if let Some(data) = line.strip_prefix("data:") {
            self.data.push_str(data.strip_prefix(' ').unwrap_or(data));
            self.data.push('\n');
            if self.data.len() > 1024 * 1024 {
                bail!("LLM stream event is too large");
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<Vec<Delta>> {
        let mut events = Vec::new();
        let pending = String::from_utf8(std::mem::take(&mut self.pending))?;
        self.line(pending.trim_end_matches('\r'), &mut events)?;
        self.line("", &mut events)?;
        Ok(events)
    }

    fn parse(&self, data: &str) -> Result<Vec<Delta>> {
        if data == "[DONE]" {
            return Ok(vec![Delta::Done]);
        }
        let value: Value = serde_json::from_str(data).context("Invalid LLM stream data")?;
        if let Some(error) = value.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| error.as_str())
                .unwrap_or("LLM request failed");
            return Ok(vec![Delta::Error(message.to_string())]);
        }
        let text = match self.format {
            Format::OpenAi => value.pointer("/choices/0/delta/content"),
            Format::Anthropic => value.pointer("/delta/text"),
            Format::Ollama => value.pointer("/message/content"),
        }
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty());
        let mut events = text
            .map(|text| vec![Delta::Text(text.to_string())])
            .unwrap_or_default();
        if value.get("type").and_then(Value::as_str) == Some("message_stop")
            || value.get("done").and_then(Value::as_bool) == Some(true)
        {
            events.push(Delta::Done);
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_provider_precedence_and_system_prompt() {
        let provider = Llm::from_lookup(|key| match key {
            "OLLAMA_MODEL" => Some("local".into()),
            "OPENAI_API_KEY" => Some("secret".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(provider.endpoint, "http://localhost:11434/api/chat");
        assert_eq!(provider.payload(&[])["messages"][0]["role"], "system");
        let provider =
            Llm::from_lookup(|key| (key == "ANTHROPIC_API_KEY").then(|| "secret".into())).unwrap();
        assert_eq!(provider.payload(&[])["system"], SYSTEM_PROMPT);
        assert!(Llm::from_lookup(|_| None).is_err());
    }

    #[test]
    fn decodes_fragmented_sse_and_unicode() {
        let bytes = "event: delta\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"héllo\"}}]}\r\n\r\ndata: [DONE]\n\n";
        let mut decoder = Decoder::new(Format::OpenAi);
        let mut events = Vec::new();
        for byte in bytes.as_bytes() {
            events.extend(decoder.push(&[*byte]).unwrap());
        }
        assert_eq!(events, [Delta::Text("héllo".into()), Delta::Done]);
    }

    #[test]
    fn decodes_ollama_final_record_and_anthropic_errors() {
        let mut decoder = Decoder::new(Format::Ollama);
        assert!(decoder
            .push(br#"{"message":{"content":"Hi"},"done":true}"#)
            .unwrap()
            .is_empty());
        assert_eq!(
            decoder.finish().unwrap(),
            [Delta::Text("Hi".into()), Delta::Done]
        );
        let mut decoder = Decoder::new(Format::Anthropic);
        assert_eq!(
            decoder
                .push(b"data: {\"type\":\"error\",\"error\":{\"message\":\"busy\"}}\n\n")
                .unwrap(),
            [Delta::Error("busy".into())]
        );
        assert!(decoder.push(&vec![b'x'; 1024 * 1024 + 1]).is_err());
    }
}
