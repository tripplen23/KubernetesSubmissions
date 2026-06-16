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

## Step by step

### 1. Build and push

```bash
docker build -t tripplen23/todo-app:latest .
docker push tripplen23/todo-app:latest
```

> Rebuild every time you change the code — the cluster runs the old image until you push a new one.

### 2. Deploy

```bash
kubectl apply -f manifests/deployment.yaml
kubectl apply -f manifests/service.yaml
```

### 3. Replace the running pod (after code changes)

```bash
kubectl delete pod -l app=todo-app
```

The Deployment recreates the pod with the new image automatically.

### 4. Verify

```bash
# Check startup message
kubectl logs -l app=todo-app

# Port-forward and test
kubectl port-forward deployment/todo-app 3000
curl http://localhost:3000/
# → pong
```

Startup log should show: `Server started in port 3000`
