FROM rust:1.91-bookworm AS builder

RUN apt-get update && \
    apt-get install -y --no-install-recommends pkg-config libssl-dev llvm clang cmake && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY src ./src
COPY public ./public
RUN cargo build --release --locked --workspace && \
    mkdir /dist && \
    cp target/release/sage target/release/parakeet target/release/kokoro /dist/ && \
    cp -R target/release/build/espeak-rs-sys-*/out/share/espeak-ng-data /dist/

FROM debian:bookworm-slim
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 libstdc++6 libgomp1 && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /dist/ /usr/local/bin/
WORKDIR /app
ENV SAGE_HOST=0.0.0.0
EXPOSE 3000
CMD ["sage"]
