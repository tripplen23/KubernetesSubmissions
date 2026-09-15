FROM rust:1.85-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN useradd -m app
COPY --from=builder /app/target/release/ping-pong /usr/local/bin/ping-pong
USER app
EXPOSE 3541
CMD ["ping-pong"]