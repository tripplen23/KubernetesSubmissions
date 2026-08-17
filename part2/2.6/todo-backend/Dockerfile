FROM rust:1.85-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/todo-backend /usr/local/bin/todo-backend
EXPOSE 3000
CMD ["/usr/local/bin/todo-backend"]