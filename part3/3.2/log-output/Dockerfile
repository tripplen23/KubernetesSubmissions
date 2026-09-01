FROM rust:1.85-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/log-output /usr/local/bin/log-output
EXPOSE 3000
CMD ["/usr/local/bin/log-output"]