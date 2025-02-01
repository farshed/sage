# sage

Lightweight, local voice chat with LLMs.

## Dependencies

-  [Rust](https://www.rust-lang.org/tools/install)
-  [Bun](https://bun.sh)
-  [Python](https://www.python.org/downloads) (optional)

## Run

Run `python setup.py` in the project root. This will download the required model weights and compile the binaries needed for Sage.

Alternatively, you can manually download [model weights]() and place them in `models` directory in the project root. Then compile `kokoro` and `whisper` with `cargo build --release` in their respective directories.

Place a .env file containing TOGETHER_API_KEY in the project root directory.

Start the project with `bun dev`.
