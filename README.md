# Sage

Converse with large language models using speech. [DEMO](https://www.youtube.com/watch?v=aAl0SuJVm4g)

-  **Open**: Powered by state-of-the-art open-source speech processing models.
-  **Efficient**: Light enough to run on consumer hardware, with low latency.
-  **Self-hosted**: Entire pipeline runs offline, limited only by compute power.
-  **Modular**: Switching LLM providers is as simple as changing an environment variable.

## How it works

<br/>

<picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://github.com/farshed/sage/blob/main/assets/architecture-dark.png?raw=true">
   <source media="(prefers-color-scheme: light)" srcset="https://github.com/farshed/sage/blob/main/assets/architecture-light.png?raw=true">
   <img alt="Sage architecture" src="https://github.com/farshed/sage/blob/main/assets/architecture-dark.png?raw=true">
</picture>

## Run

1. For text generation, you can either self-host an LLM using Ollama, or opt for a third-party provider. This can be configured using a .env file in the project root.

   -  **If you're using Ollama**, add the `OLLAMA_MODEL` variable to the .env file to specify the model you'd like to use. (Example: `OLLAMA_MODEL=deepseek-r1:7b`)

   -  **Among the third-party providers**, Sage supports the following out of the box:

      1. Deepseek
      2. OpenAI
      3. Anthropic
      4. Together.ai

   -  To use a provider, add a `<PROVIDER>_API_KEY` variable to the .env file. (Example: `OPENAI_API_KEY=xxxxxxxxxxxxxxxxxxxxxxx`)
   -  To choose which model should be used for a given provider, use the `<PROVIDER>_MODEL` variable. (Example: `DEEPSEEK_MODEL=deepseek-chat`)

2. Next, you have two choices: Run Sage as a Docker container (the easy way) or natively (the hard way). _Running it with Docker has a performance penalty (Inference is 5-8x slower compared to running it natively)._

   -  **With Docker**: Install Docker and start the daemon. Download these files and place them inside a `models` directory at the project root. Run `bun docker-build` to build the image and then `bun docker-run` to spin a container. The UI is exposed at `http://localhost:3000`.
   -  **Without Docker**: Install [Bun](https://bun.sh), [Rust](https://www.rust-lang.org/tools/install), LLVM, Clang, and CMake. Make sure all of these are accessible via `$PATH`. Then, run `setup-unix.sh` or `setup-win.bat` depending on your platform. This will download the required model weights and compile the binaries needed for Sage. Once finished, start the project with `bun start`. _The first run on macOS is slow (~20 minutes on M1 Pro), since the ANE service compiles the Whisper CoreML model to a device-specific format. Next runs are faster._

## Future work

-  [x] Make it easier to run. (Dockerize?)
-  [ ] Optimize the pipeline
-  [ ] Release as a library?
