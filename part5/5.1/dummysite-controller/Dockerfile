FROM golang:1.22-bookworm AS builder
ENV GOTOOLCHAIN=local
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY main.go ./
RUN CGO_ENABLED=0 go build -o dummysite-controller .

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/dummysite-controller /usr/local/bin/dummysite-controller
CMD ["/usr/local/bin/dummysite-controller"]
