# https://hf-mirror.com/hexgrad/Kokoro-82M/resolve/main/kokoro-v0_19.onnx
# https://github.com/farshed/sage/releases/download/voices.json_v0.1/voices.json
# https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin

FROM rust:latest

RUN apt-get update && \
    apt-get install -y curl unzip pkg-config libssl-dev llvm clang cmake && \
    rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://bun.sh/install | bash && \
    mv /root/.bun/bin/bun /usr/local/bin/

WORKDIR /app

RUN mkdir -p /app/models /app/tmp/input /app/tmp/output /app/tmp/transcripts

COPY . .

RUN cd whisper && cargo build --release && \
    cd ../kokoro && cargo build --release && \
    cd .. && \
    bun install

EXPOSE 3000

CMD ["bun", "start"]
