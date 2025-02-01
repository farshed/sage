# sage

Local, lightweight voice chat with LLMs.

## Requirements

-  [Rust](https://www.rust-lang.org/tools/install)
-  [Bun](https://bun.sh)
-  [Python](https://www.python.org/downloads)

## Run

1. Run `python setup.py` in the project root. This will download the required model weights and compile the binaries needed for Sage.
2. Place a .env file containing TOGETHER_API_KEY in the project root directory.
3. Start the project with `bun dev`.
