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
