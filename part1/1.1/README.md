# Exercise 1.1 — Getting started (log_output)

## Assignment

> Create an application that generates a random string on startup, stores it in
> memory, and prints it to stdout every 5 seconds with an ISO-8601 timestamp, e.g.
>
> ```
> 2020-03-30T12:15:17.705Z: 8523ecb1-c716-4cb6-a044-b9e83bb98e43
> 2020-03-30T12:15:22.705Z: 8523ecb1-c716-4cb6-a044-b9e83bb98e43
> ```

## Solution

- **Source**: see [`src/main.rs`](./src/main.rs) — Rust 1.85 + tokio + uuid + chrono
- **Image**: [`tripplen23/log-output:1.1`](https://hub.docker.com/r/tripplen23/log-output/tags)
- **Manifest**: [`manifests/deployment.yaml`](./manifests/deployment.yaml)
- **Dockerfile**: multi-stage (`rust:1.85-slim` builder → `debian:bookworm-slim` runtime)

### Build & run locally

```bash
cd part1/1.1
docker build -t tripplen23/log-output:1.1 .
docker push tripplen23/log-output:1.1
```

### Deploy

```bash
kubectl apply -f manifests/deployment.yaml
kubectl get pods -l app=log-output
kubectl logs -f -l app=log-output
```

You should see a new log line every 5 seconds:

```
2026-07-29T14:30:00.123Z: <uuid>
2026-07-29T14:30:05.123Z: <uuid>
```
