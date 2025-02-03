# Sage

Converse with large language models using speech. Local, lightweight, and open-source.

## Requirements

-  [Rust](https://www.rust-lang.org/tools/install)
-  [Bun](https://bun.sh)
-  [Ollama](https://ollama.com) (Alternatively, you can use a third-party provider)

## Run

1. Run `setup-unix.sh` or `setup-win.bat` depending on your platform. This will download the required model weights and compile the binaries needed for Sage.

2. For text generation, you can either self-host an LLM using Ollama, or opt for a third-party provider.

-  **If you're using Ollama**, add an `OLLAMA_MODEL` variable to the .env file to specify the model you're hosting. (Example: `OLLAMA_MODEL=deepseek-r1:671b`)

-  **Among the third-party providers**, Sage supports the following out of the box:

   1. Deepseek
   2. Anthropic
   3. OpenAI
   4. Together.ai

   -  To use a provider, add a `<PROVIDER>_API_KEY` variable to the .env file. (Example: `OPENAI_API_KEY=xxxxxxxxxxxxxxxxxxxxxxx`)
   -  To choose which model should be used for a given provider, use the `<PROVIDER>_MODEL` variable. (Example: `DEEPSEEK_MODEL=deepseek-chat`)

3. Start the project with `bun dev`.

## Future work

1. Optimize the pipeline.
2. Make it easier to run. (Dockerize?)
3. Release as a library?

Among the latter, only [together.ai](https://www.together.ai) is currently supported out of the box. It provides $1 in free credit which is good for ~800k tokens of Deepseek V3 usage. \[I'm not affiliated]
