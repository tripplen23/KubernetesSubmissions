# Exercise 1.2 — Project v0.1 (todo-app web server)

## Assignment

> Create a project that contains a web-server. It doesn't have to do much — just
> respond with something sensible when it receives a request. Deploy it.
> Refactor your application in such a way that the application is started with
> a configurable port — the port can be specified using the `PORT` environment
> variable, which should default to `3000`.
> The log output **must** contain a line that says
> `Server started in port <port_number>` so that the grader can verify the port.

## Solution

- **Source**: see [`src/main.rs`](./src/main.rs) — Rust 1.85 + axum 0.8 + tokio + serde
- **Image**: [`tripplen23/todo-app:1.2`](https://hub.docker.com/r/tripplen23/todo-app/tags)
- **Manifests**:
  - [`manifests/deployment.yaml`](./manifests/deployment.yaml) — Deployment, `PORT=3000`
  - [`manifests/service.yaml`](./manifests/service.yaml) — ClusterIP service

### Endpoints

| Path | Method | Response |
|---|---|---|
| `/` | GET | `pong` |
| `/api/health` | GET | `{"status":"ok"}` |
| `/api/todos` | GET | `[]` (placeholder) |

### Build & run locally

```bash
cd part1/1.2
docker build -t tripplen23/todo-app:1.2 .
docker push tripplen23/todo-app:1.2
```

### Deploy

```bash
kubectl apply -f manifests/deployment.yaml
kubectl apply -f manifests/service.yaml
```

### Verify

```bash
# Startup log includes the required line:
kubectl logs -l app=todo-app | grep "Server started"
# → Server started in port 3000

# Port-forward and probe:
kubectl port-forward deployment/todo-app 3000:3000
curl http://localhost:3000/
# → pong
```

### Replace the pod after a rebuild

```bash
docker build -t tripplen23/todo-app:1.2 . && docker push tripplen23/todo-app:1.2
kubectl delete pod -l app=todo-app   # Deployment re-creates the pod with the new image
```
