# sage

Local, lightweight voice chat with LLMs.

## Components

-  Whisper.cpp for ASR
-  Deepseek V3 for text gen
-  Kokoro v0.19 for speech gen
-  Bun & Elysia to tie all the pieces together

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
