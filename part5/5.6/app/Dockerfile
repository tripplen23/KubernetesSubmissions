FROM rust:1.85-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/hello /usr/local/bin/hello
# Knative injects PORT (8080 by default) into the container; the app reads it.
EXPOSE 8080
CMD ["/usr/local/bin/hello"]
