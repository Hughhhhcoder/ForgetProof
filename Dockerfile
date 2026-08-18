FROM rust:1.91-bookworm AS builder

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY examples ./examples
RUN cargo build --release

FROM python:3.11-slim

WORKDIR /opt/forgetproof
COPY --from=builder /src/target/release/forgetproof /usr/local/bin/forgetproof
COPY python ./python
COPY schemas ./schemas
COPY examples ./examples
ENV PYTHONPATH=/opt/forgetproof/python
ENV PYTHONDONTWRITEBYTECODE=1
ENTRYPOINT ["forgetproof"]
