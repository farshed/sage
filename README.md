# Sage

Converse with large language models using speech. Lightweight, local, and open-source.

## How it works

Sage uses state-of-the-art open-source speech processing models. Here's an overview of its pipeline.

<br/>

<picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://github.com/farshed/sage/blob/main/assets/architecture-dark.png?raw=true">
   <source media="(prefers-color-scheme: light)" srcset="https://github.com/farshed/sage/blob/main/assets/architecture-light.png?raw=true">
   <img alt="Sage architecture" src="https://github.com/farshed/sage/blob/main/assets/architecture-dark.png?raw=true">
</picture>

## Requirements

-  [Bun](https://bun.sh)
-  [Rust](https://www.rust-lang.org/tools/install)
-  [Ollama](https://ollama.com) (Alternatively, you can use a third-party provider)

## Run

1. Run `setup-unix.sh` or `setup-win.bat` depending on your platform. This will download the required model weights and compile the binaries needed for Sage.

2. For text generation, you can either self-host an LLM using Ollama, or opt for a third-party provider.

-  **If you're using Ollama**, add the `OLLAMA_MODEL` variable to the .env file to specify the model you'd like to use. (Example: `OLLAMA_MODEL=deepseek-r1:7b`)

-  **Among the third-party providers**, Sage supports the following out of the box:

   1. Deepseek
   2. OpenAI
   3. Anthropic
   4. Together.ai

-  To use a provider, add a `<PROVIDER>_API_KEY` variable to the .env file. (Example: `OPENAI_API_KEY=xxxxxxxxxxxxxxxxxxxxxxx`)
-  To choose which model should be used for a given provider, use the `<PROVIDER>_MODEL` variable. (Example: `DEEPSEEK_MODEL=deepseek-chat`)

3. Start the project with `bun start`. The first run on macOS is slow (~20 minutes on M1 Pro), since the ANE service compiles the Whisper CoreML model to a device-specific format. Next runs are faster.

## Future work

1. Optimize the pipeline.
2. Make it easier to run. (Dockerize?)
3. Release as a library?
