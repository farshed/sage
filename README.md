# sage

Converse with large language models using speech. Local, lightweight, and open-source.

## Models

-  [Whisper.cpp](https://github.com/ggerganov/whisper.cpp)
-  Deepseek V3
-  [Kokoro v0.19](https://huggingface.co/hexgrad/Kokoro-82M)

## Requirements

-  [Rust](https://www.rust-lang.org/tools/install)
-  [Bun](https://bun.sh)
-  [Python](https://www.python.org/downloads)

## Run

1. Run `python setup.py` in the project root. This will download the required model weights and compile the binaries needed for Sage.
2. Place a .env file containing `TOGETHER_API_KEY` in the project root.
3. Start the project with `bun dev`.

## Future work

1. Optimize the pipeline.
2. Make it easier to run. (Dockerize?)
3. Release as a library?
