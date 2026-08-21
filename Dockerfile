FROM rust:1.91-bookworm AS builder

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY examples ./examples
RUN cargo build --release --bin memoryproof --bin forgetproof

FROM python:3.11-slim

LABEL org.opencontainers.image.title="MemoryProof"
LABEL org.opencontainers.image.description="Test what your AI remembers. Prove what it forgot."
LABEL org.opencontainers.image.source="https://github.com/Hughhhhcoder/MemoryProof"
LABEL org.opencontainers.image.licenses="Apache-2.0"

WORKDIR /opt/memoryproof
COPY --from=builder /src/target/release/memoryproof /usr/local/bin/memoryproof
COPY --from=builder /src/target/release/forgetproof /usr/local/bin/forgetproof
COPY python ./python
COPY schemas ./schemas
COPY examples ./examples
ENV PYTHONPATH=/opt/memoryproof/python
ENV PYTHONDONTWRITEBYTECODE=1
ENTRYPOINT ["memoryproof"]
