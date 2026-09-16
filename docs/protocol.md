# Speech streaming protocol

Every frame has a one-byte type, a four-byte unsigned little-endian payload length, and the payload. Payloads are limited to 32 MiB. The shared Rust implementation is in `crates/speech-protocol`; the browser reader is in `public/protocol.js`.

| Type | Value | Worker stdout | HTTP response |
| --- | --- | --- | --- |
| READY | 0 | Model loaded | Unused |
| DATA | 1 | Transcript UTF-8 or PCM audio | PCM audio |
| DONE | 2 | Request complete | JSON conversation history and timings |
| ERROR | 3 | UTF-8 error, request complete | JSON error, turn failed |
| METADATA | 4 | Unused | JSON sample rate, channel count, encoding |

Worker stdin accepts DATA frames. Parakeet receives WAV bytes. Kokoro receives UTF-8 text and returns little-endian float32 PCM, mono at 24 kHz. Workers process one request at a time and flush every frame. Diagnostics go to stderr so they cannot corrupt stdout.

The server maintains a bounded queue per worker. It waits for READY before accepting requests. Cancelled output is drained through DONE or ERROR before another request starts. A worker that exits, sends invalid framing, or exceeds its deadline is stopped and restarted for the next request.

`POST /chat` takes multipart fields `file` and `messages`. The file is a WAV recording up to 16 MiB. Messages are a JSON array of `{ "role": "user" | "assistant", "content": "..." }` objects. The response uses `application/x-sage-audio`, starting with METADATA, followed by DATA frames and a terminal DONE or ERROR frame.

Metadata is `{ "sampleRate": 24000, "channels": 1, "encoding": "f32le" }`. DONE contains `messages` and `timings` with `transcriptionMs`, `firstAudioMs`, and `totalMs`. ERROR contains an `error` string. The browser applies conversation history only after DONE and resamples PCM to the audio device's rate.

Each worker has a 120-second startup/request deadline. A full HTTP turn has a five-minute deadline. Dropping the HTTP response cancels generation and pending speech work. LLM deltas, worker audio, and HTTP frames use bounded channels, and the browser limits its queued playback to about eight seconds.
