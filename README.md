# sage

Lightweight, local, voice chat with LLMs.

## Dependencies

-  [Rust](https://www.rust-lang.org/tools/install)
-  [Bun](https://bun.sh)
-  [Python](https://www.python.org/downloads) (optional)

## Run

Run `python setup.py`. This will download the required model weights and compile the binaries needed for Sage.

Alternatively, you can manually download the [model weights]() and place them in the `models` directory in the project root. Then compile `kokoro` and `whisper` with `cargo build --release` in their respective directories.

Start the project with `bun dev`. The UI is served at http://localhost:3000.
