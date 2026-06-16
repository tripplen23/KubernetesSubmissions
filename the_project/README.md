## Todo App

A simple web server — the main project for the DevOps with Kubernetes course.

### Endpoints

| Path | Method | Response |
|---|---|---|
| `/` | GET | `pong` |
| `/api/health` | GET | `{"status":"ok"}` |
| `/api/todos` | GET | `[]` (placeholder) |

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `PORT` | `3000` | TCP port the server listens on |

### Build and push

```bash
docker build -t tripplen23/todo-app:latest .
docker push tripplen23/todo-app:latest
```

### Deploy

```bash
kubectl apply -f manifests/deployment.yaml
kubectl apply -f manifests/service.yaml
```

### Verify

```bash
kubectl get pods
kubectl port-forward deployment/todo-app 3000:3000
curl http://localhost:3000/
# → pong
```
