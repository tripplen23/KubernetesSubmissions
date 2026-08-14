# Exercise 2.2 — The project, step 8 (todo-backend)

## Goal

> Let us get back to our Project. In the last exercise of the previous
> Chapter, we added a random pic and a form for creating todos to the
> app. The next step is to create a new service that takes care of
> saving the todo items.
>
> This new service, let us call it todo-backend, should have a
> **GET /todos** endpoint for fetching the list of todos and a
> **POST /todos** endpoint for creating a new todo. The todos can be
> saved **in memory**, we'll add a database later.

```
 Browser ──► todo-app (serves HTML + image, caches pics)
              │  GET /todos  +  POST /todos   (HTTP, like 2.1)
              ▼
          todo-backend (new service — in-memory todo store)
```

Two applications now:

- **todo-app** (from 1.13): serves the HTML page with the form and the
  hourly image; it **no longer hardcodes todos** — it fetches them from
  the new todo-backend service over HTTP and renders them.
- **todo-backend** (new): `GET /todos` returns the list, `POST /todos`
  adds a todo. Data lives in memory (a database comes later).

## prerequisites

**Check k3d cluster with the right port mapping:**

```bash
k3d cluster list
# NAME        SERVERS  AGENTS  LOADBALANCER
# mycluster   1/1      1/1     true

ss -tlnp 2>/dev/null | grep -E ':(8081|8082)'
# LISTEN 0  4096  *:8081  *:*
# LISTEN 0  4096  *:8082  *:*
```

**Check Traefik Ingress controller:**

```bash
kubectl get pods -n kube-system -l app.kubernetes.io/name=traefik
# traefik-xxxxxxxxxx-xxxxx   1/1  Running
```

## Source code

### `todo-backend/src/main.rs` (the new service)

- In-memory store: `TodoStore { todos: Vec<Todo>, next_id: u64 }`
  behind `Arc<Mutex<_>>` (shared state via axum `State`).
- `GET /todos` → `Json<Vec<Todo>>` (clones the list).
- `POST /todos` — body `{"title": "..."}`:
  - trims the title, rejects empty or > 140 chars with `400`;
  - otherwise assigns `id` from `next_id`, pushes, replies
    `201 Created` with the created todo.

### `todo-app/src/main.rs` (from 1.13, now talking to the backend)

- `GET /` — same page as 1.13 (image + form + list) but the list is
  **fetched over HTTP**: `reqwest::get("{TODO_BACKEND_URL}/todos")`,
  then rendered server-side.
- `POST /todos` — receives the HTML form (`content` field), validates
  (empty / >140 → 400), forwards a JSON `{"title": ...}` to the
  backend with a `reqwest` client, then `Redirect::to("/")` (303).
- `/image` + `/api/health` unchanged from 1.12/1.13.
- `TODO_BACKEND_URL` env var, default `http://todo-backend-svc:3000`
  (the Service DNS name — pod-to-pod HTTP as in 2.1).

verify locally:

```bash
cargo build --manifest-path todo-backend/Cargo.toml
cargo build --manifest-path todo-app/Cargo.toml

# terminal 1 — todo-backend on port 3003
PORT=3003 ./todo-backend/target/debug/todo-backend

# terminal 2 — todo-app on port 3001
mkdir -p /tmp/2.2-share
PORT=3001 TODO_BACKEND_URL=http://localhost:3003 \
  IMAGE_PATH=/tmp/2.2-share/image.jpg ./todo-app/target/debug/todo-app

# terminal 3 — test
curl -s localhost:3003/todos                                  # []
curl -s -X POST localhost:3003/todos -H "Content-Type: application/json" \
  -d '{"title":"Learn Kubernetes"}'                           # 201
curl -s localhost:3003/todos                                  # [ {id:0,...} ]
curl -s localhost:3001/ | grep '<span>'                       # rendered list
curl -s -X POST localhost:3001/todos -d 'content=Buy milk' -w "%{http_code}\n"
# 303 → refresh / → "Buy milk" is in the list
```

**To stop the servers:** `Ctrl+C` in each foreground terminal, or
`pkill -f 'debug/todo-backend'; pkill -f 'debug/todo-app'`.

## Step 1 — Build and push both images

```bash
cd todo-backend
docker build -t tripplen63/todo-backend:2.2 .
docker push tripplen63/todo-backend:2.2

cd ../todo-app
docker build -t tripplen63/todo-app:2.2 .
docker push tripplen63/todo-app:2.2
```

## Step 2 — PV/PVC (only if missing)

```bash
kubectl get pv,pvc
# example-pv     Bound    image-claim      ← exists, skip this step
# No resources found                       ← missing, continue below
```

Prepare the node directory and apply:

```bash
docker exec k3d-mycluster-agent-0 mkdir -p /tmp/kube

kubectl apply -f manifests/pv/persistentvolume.yaml
kubectl apply -f manifests/persistentvolumeclaim.yaml
kubectl get pv,pvc
# example-pv     Bound    image-claim     ← both bound, continue
```

## Step 3 — Apply the app manifests

```bash
kubectl apply -f manifests/          # (PV/PVC first if missing)
kubectl get pods
# todo-app-xxx       1/1  Running
# todo-backend-xxx   1/1  Running

kubectl get svc
# todo-app-svc       ClusterIP   10.43.x.x   3000/TCP
# todo-backend-svc   ClusterIP   10.43.x.x   3000/TCP
```

## Step 4 — Verify

```bash
# 1. The backend works (direct from your machine via port-forward first)
kubectl port-forward svc/todo-backend-svc 3004:3000 &
curl -s localhost:3004/todos                          # []
curl -s -X POST localhost:3004/todos -H "Content-Type: application/json" \
  -d '{"title":"Learn Kubernetes"}' -w " %{http_code}\n"
# {"id":0,"title":"Learn Kubernetes","done":false} 201
curl -s localhost:3004/todos                          # list with 1 item
kill %1

# 2. The frontend through the Ingress
curl -s http://localhost:8081/ | grep '<span>'        # rendered list
# <span>Learn Kubernetes</span>

# 3. Create a todo through the form → 303 → it appears
curl -s -X POST http://localhost:8081/todos -d 'content=Buy milk' -w "%{http_code}\n"
# 303
curl -s http://localhost:8081/ | grep '<span>'
# <span>Learn Kubernetes</span>
# <span>Buy milk</span>

# 4. The backend now holds both
kubectl exec deployment/todo-backend -- wget -qO - http://localhost:3000/todos 2>/dev/null || true
```

## Step 5 — Clean up

```bash
kubectl delete -f manifests/
kubectl delete -f manifests/pv/persistentvolume.yaml 2>/dev/null
kubectl get pods,svc
# only service/kubernetes remains
```

## P/S:

1. **Splitting the project**: UI (todo-app) and data (todo-backend)
   are now separate deployments; the UI calls the data service over
   HTTP via a Service DNS name (2.1 pattern).
2. **In-memory state**: a `Mutex<Vec<Todo>>` is enough until a
   database arrives — restarting the backend loses todos, and that's
   expected at this point in the course.
3. **Two different HTTP roles**: `GET` returns state, `POST` mutates
   it (201 + the created resource); validation (≤140 chars) lives in
   the backend as the single source of truth.
4. **Cluster-internal services stay internal**: no Ingress for
   todo-backend — only the frontend is reachable from the browser.
