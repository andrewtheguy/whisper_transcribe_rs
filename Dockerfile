FROM rust:1.81-bookworm AS rust-stage


FROM ubuntu:22.04 AS builder-stage

# Update default packages
RUN apt-get update

# Get Ubuntu packages
RUN apt-get install -y \
    build-essential \
    libssl-dev \
    pkg-config \
    clang-13 lld-13 libsdl2-dev\
    curl cmake && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /usr/local/cargo/bin

COPY --from=rust-stage /usr/local/cargo/bin /usr/local/cargo/bin
COPY --from=rust-stage /usr/local/rustup /usr/local/rustup
    

ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo
ENV PATH="/usr/local/cargo/bin:${PATH}"


COPY . /app

WORKDIR /app

RUN cargo build --release

FROM scratch
COPY --from=builder-stage /app/target/release/whisper_transcribe_rs /
