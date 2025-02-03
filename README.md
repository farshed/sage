# Sage

Converse with large language models using speech. Local, lightweight, and open-source.

## Requirements

-  [Rust](https://www.rust-lang.org/tools/install)
-  [Bun](https://bun.sh)
-  [Ollama](https://ollama.com) (Alternatively, you can use a third-party provider)

## Run

1. Run `setup-unix.sh` or `setup-win.bat` depending on your platform. This will download the required model weights and compile the binaries needed for Sage.

2. For text generation, you can either self-host an LLM using Ollama, or opt for a third-party provider. Among the latter, only [together.ai](https://www.together.ai) is currently supported out of the box. It provides $1 in free credit which is good for ~800k tokens of Deepseek V3 usage. \[I'm not affiliated]

-  **If you're using Ollama**, add an `OLLAMA_MODEL` variable to the .env file. (Example: `OLLAMA_MODEL=deepseek-r1:671b`)

-  **If you're using together.ai**, add `TOGETHER_API_KEY` variable to the .env file. (API keys can be obtained at https://api.together.ai). The model defaults to Deepseek V3 but can be changed using the `TOGETHER_MODEL` variable.

3. Start the project with `bun dev`.

## Future work

1. Optimize the pipeline.
2. Make it easier to run. (Dockerize?)
3. Release as a library?
