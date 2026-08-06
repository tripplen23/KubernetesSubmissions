FROM rust:1.85-slim AS builder
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/target/release/log_output /usr/local/bin/log_output
EXPOSE 3000
CMD ["/usr/local/bin/log_output"]