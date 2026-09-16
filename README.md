# Sage

Talk to language models using local speech recognition and speech generation. [Demo](https://www.youtube.com/watch?v=aAl0SuJVm4g).

Sage is a Rust Cargo workspace. Bun and Node.js are not needed to build or run it. Browser JavaScript is embedded in the server and runs directly in your browser.

## Run

Install [Rust](https://www.rust-lang.org/tools/install) 1.91 or newer, a C/C++ toolchain, OpenSSL, LLVM/Clang, and CMake. The first build downloads native inference libraries, and model setup downloads about 1 GB of weights.

On macOS or Linux:

```sh
bash setup-unix.sh
```

On Windows, run `setup-win.bat` with the Rust MSVC toolchain and the native build tools available in your terminal. Setup keeps your existing `.env` and model files.

Configure a language model in `.env`. For example, with a running Ollama server and an installed model:

```dotenv
OLLAMA_MODEL=deepseek-r1:7b
```

Then start Sage:

```sh
./target/release/sage
```

On Windows, use `target\release\sage.exe`. Open `http://localhost:3000`. Microphone access needs localhost or HTTPS.

The equivalent manual setup is:

```sh
cargo build --release --locked --workspace
./target/release/sage download-models
./target/release/sage
```

After setup, `cargo run` also starts Sage. It looks for speech workers in `target/debug`, then uses the workers from `target/release` if needed. `cargo run` only builds the server; use `cargo build --workspace` to build debug workers or `cargo build --release --workspace` to build release workers. An explicit `--worker-dir` or `SAGE_WORKER_DIR` overrides this lookup.

`cargo run --release -p sage` starts the release server. Keep `sage`, `parakeet`, and `kokoro` together when distributing the application. Moving them to another machine also requires the matching native runtime files, including `espeak-ng-data` from `target/release/build/espeak-rs-sys-*/out/share/`. Docker packages these files for you.

## Configuration

Sage reads `.env` in the current directory. Existing environment variables take precedence.

| Provider | Required setting | Optional model setting | Default model |
| --- | --- | --- | --- |
| Ollama | `OLLAMA_MODEL` | Already specified | None |
| DeepSeek | `DEEPSEEK_API_KEY` | `DEEPSEEK_MODEL` | `deepseek-chat` |
| Anthropic | `ANTHROPIC_API_KEY` | `ANTHROPIC_MODEL` | `claude-3-5-haiku-20241022` |
| OpenAI | `OPENAI_API_KEY` | `OPENAI_MODEL` | `gpt-4-turbo` |
| Together.ai | `TOGETHER_AI_API_KEY` | `TOGETHER_AI_MODEL` | `deepseek-ai/DeepSeek-V3` |

If several providers are configured, the first one in this table wins. You can select another model offered by your provider. Each provider supports a matching `_BASE_URL` setting. `OLLAMA_BASE_URL` defaults to `http://localhost:11434`; cloud provider base URLs include any API version prefix, such as `https://api.openai.com/v1`.

| Setting | Default |
| --- | --- |
| `SAGE_HOST` or `--host` | `127.0.0.1` |
| `PORT` or `--port` | `3000` |
| `SAGE_MODELS_DIR` or `--models-dir` | `models` in the current directory |
| `PARAKEET_MODEL_DIR` | Parakeet's model folder inside the models directory |
| `SAGE_WORKER_DIR` or `--worker-dir` | Directory containing Sage; debug builds also check the sibling release directory |

`download-models` honors the model directory settings. Old Whisper weights are unused and can be removed manually.

## Workspace and pipeline

| Package | Purpose |
| --- | --- |
| `sage` in `src/` | HTTP server, LLM streaming, model downloads, worker management |
| `crates/parakeet` | Parakeet TDT speech recognition through sherpa-onnx |
| `crates/kokoro` | Kokoro speech generation |
| `crates/speech-protocol` | Shared binary framing for the speech workers |

1. The browser records a WAV file and sends it to the Rust server.
2. Parakeet TDT 0.6B v3 transcribes it locally using INT8 ONNX models on the CPU.
3. The LLM streams text. Short sentences and clauses go to Kokoro while more text is arriving.
4. Kokoro returns mono 24 kHz audio. The server streams it to an AudioWorklet for playback.

Both speech models load once and stay in memory. Separate worker processes keep their native inference runtimes isolated and allow restart after a crash or timeout. WAV, text, and PCM data travel through process pipes and the HTTP response without temporary files. See [the protocol](docs/protocol.md).

Queues are bounded so slow playback does not grow memory without limit. Clicking during thinking or speaking cancels playback and the LLM request. A speech worker finishes its active chunk, discards its output, and skips cancelled queued work. Conversation history updates only after a complete response. The server logs transcription, first audio, and total response times.

Speech processing is local. Cloud LLM providers receive conversation text. Ollama can keep text generation local too. The page currently loads Tailwind from a CDN. Parakeet supports multiple languages; the Kokoro voice is configured for US English.

## Docker

```sh
docker build -t sage .
docker run --rm -v "$PWD/models:/app/models" sage sage download-models
docker run --rm -p 3000:3000 --env-file .env -v "$PWD/models:/app/models" sage
```

For Ollama running on a Docker Desktop host, set `OLLAMA_BASE_URL=http://host.docker.internal:11434`. On Linux, also pass `--add-host=host.docker.internal:host-gateway`. Use `SAGE_HOST=0.0.0.0` if your `.env` overrides the container's listening address.

## Checks

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy -p sage -p sage-protocol -p parakeet --all-targets --locked -- -D warnings
./target/release/sage check-speech
```

Rust tests cover framing, audio conversion, text chunking, provider stream decoding, worker reuse, cancellation, crash and timeout recovery, upload validation, and streamed responses. `check-speech` uses the actual models and a local mock LLM. It checks repeated transcription, invalid audio recovery, and audio arriving over HTTP before the mock LLM finishes. No provider key is needed.

The optional browser logic tests remain in `tests/browser` and can be run with `bun test tests/browser`. They use mocked browser APIs and are separate from building, testing, or running the Rust application.
