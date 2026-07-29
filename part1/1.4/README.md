# Exercise 1.4 — Project v0.2 (todo-app, declarative)

## Assignment

> Continue from Exercise 1.2. Refactor the project structure to use
> **declarative manifests** (Deployment + Service) and re-deploy via
> `kubectl apply -f`. The application should still respect the `PORT` env var
> (default `3000`) and log `Server started in port <port_number>` on startup.

## Solution

- **Source**: see [`src/main.rs`](./src/main.rs) — same Rust binary as 1.2
- **Image**: [`tripplen63/todo-app:1.4`](https://hub.docker.com/r/tripplen63/todo-app/tags)
- **Manifests**:
  - [`manifests/deployment.yaml`](./manifests/deployment.yaml)
  - [`manifests/service.yaml`](./manifests/service.yaml)

Both manifests are versioned and live in the repo — no more imperative
`kubectl run`/`kubectl expose`. The repo is the single source of truth.

### Endpoints

| Path | Method | Response |
|---|---|---|
| `/` | GET | `pong` |
| `/api/health` | GET | `{"status":"ok"}` |
| `/api/todos` | GET | `[]` (placeholder) |

### Deploy

```bash
cd part1/1.4
kubectl apply -f manifests/deployment.yaml
kubectl apply -f manifests/service.yaml
```

### Verify

```bash
kubectl logs -l app=todo-app | grep "Server started"
# → Server started in port 3000

kubectl port-forward deployment/todo-app 3000:3000
curl http://localhost:3000/api/health
# → {"status":"ok"}
```

### Roll back / update

```bash
cd part1/1.4
# Code change → rebuild & push:
docker build -t tripplen63/todo-app:1.4 . && docker push tripplen63/todo-app:1.4

# Force the Deployment to roll out the new image:
kubectl delete pod -l app=todo-app
# or edit manifests/deployment.yaml and `kubectl apply -f manifests/deployment.yaml`
```
