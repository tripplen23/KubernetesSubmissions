# Exercise 2.8 — The project, step 11 (todos in Postgres)

## Goal

> Create a database and save the todos there. Again, the database should
> be defined as a **stateful set** with **one replica**. Use **Secrets
> and/or ConfigMaps** to have the backend access the database.

The `todo-backend` moves from in-memory storage (`Vec<Todo>` + `next_id`)
to a **Postgres** database running as a **StatefulSet**. The database
connection details are injected via a **Secret** (the password) and a
**ConfigMap** (the non-secret host/port/db/user) — building on 2.5/2.6.

`todo-app` (the frontend) is **unchanged** — it still calls the backend
over HTTP and is reused at `:2.6`.

## Concepts covered (read the course page first)

- <https://courses.mooc.fi/org/uh-cs/courses/devops-with-kubernetes-2026/chapter-3/statefulsets-and-jobs>

| Question                       | Answer                                                                                      |
| ------------------------------ | ------------------------------------------------------------------------------------------- |
| Secret vs ConfigMap?           | Secret = sensitive (password), base64-encoded, not encrypted; ConfigMap = non-secret config |
| Which part goes in the Secret? | Only the **password** — everything else is a ConfigMap                                      |
| How does the backend use them? | Reads env vars; the password comes from `secretKeyRef`, the rest from `configMapKeyRef`     |
| StatefulSet storage?           | `volumeClaimTemplates` + `storageClassName: local-path` (dynamic, no manual PV)             |

## Source code changes (todo-backend only)

The in-memory store is replaced by Postgres:

1. Reads `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_HOST`,
   `POSTGRES_PORT`, `POSTGRES_DB` (each via `env_or`, panic if missing)
2. Assembles the URL in code — the password never sits in plain text in a
   manifest's `value:` field
3. `CREATE TABLE IF NOT EXISTS todos (id BIGSERIAL PRIMARY KEY, title TEXT NOT NULL, done BOOLEAN NOT NULL DEFAULT false)` at startup, retrying while the DB boots
4. `GET /todos` → `SELECT id, title, done FROM todos ORDER BY id`
5. `POST /todos` → `INSERT INTO todos (title) VALUES ($1) RETURNING id, title, done` → 201
6. **Opens a fresh connection per request** (auto-recovery on DB restart, same as 2.7)

Cargo.toml adds the async Postgres client:

```toml
[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
tokio-postgres = "0.7"
```

## Step 1 — build + push Dockerfile (todo-backend only)

```bash
cd todo-backend
docker build -t tripplen63/todo-backend:2.8 .
docker push tripplen63/todo-backend:2.8
```

> `todo-app` is unchanged — keep using `tripplen63/todo-app:2.6`.

## Step 2 — Apply and verify manifests

```bash
docker exec k3d-mycluster-agent-0 mkdir -p /tmp/kube

kubectl apply -f manifests/pv/persistentvolume.yaml
kubectl apply -f manifests/

kubectl get statefulset,pvc,pods -n project
# statefulset.apps/postgres-ss   1/1
# persistentvolumeclaim/data-postgres-ss-0   Bound   ...   local-path
# pod/postgres-ss-0              1/1  Running
# pod/todo-backend-xxx           1/1  Running
# pod/todo-app-xxx               1/1  Running

# add todos through the UI (backend persists them in Postgres)
curl -s -X POST http://localhost:8081/todos -d 'content=Learn Kubernetes' -w "%{http_code}\n"
# 303

curl -s http://localhost:8081/ | grep '<span>'
# <span>Learn Kubernetes</span>

# confirm the todos are in the database
kubectl exec postgres-ss-0 -n project -- psql -U postgres -c "SELECT * FROM todos;"
#  id |      title       | done
# ----+------------------+------
#   1 | Learn Kubernetes | f
```

## Step 3 — Prove the data is in the DB (stateful)

**A. Todos survive a todo-backend restart:**

```bash
kubectl delete pod -n project -l app=todo-backend
sleep 15
curl -s http://localhost:8081/ | grep '<span>'
# <span>Learn Kubernetes</span>   ← still there (from Postgres, not memory)
```

**B. Todos survive a Postgres restart:**

```bash
kubectl delete pod postgres-ss-0 -n project
kubectl wait --for=condition=ready pod/postgres-ss-0 -n project --timeout=90s
sleep 5
curl -s http://localhost:8081/ | grep '<span>'
# <span>Learn Kubernetes</span>   ← still there
```

> The backend opens a fresh connection per request, so it recovers
> automatically once Postgres is ready again.

## Step 4 — Clean up

```bash
kubectl delete -f manifests/
kubectl delete -f manifests/pv/persistentvolume.yaml
kubectl delete pvc -n project -l app=postgres 2>/dev/null
kubectl get statefulset,pods,pvc -n project
# No resources found
```

## P/S:

1. **Postgres as a StatefulSet** (1 replica) — the correct resource for a
   stateful workload, with dynamic `local-path` storage.
2. **Secret vs ConfigMap**: only the password is a Secret; host/port/db/
   user are a ConfigMap.
3. **`secretKeyRef` vs `configMapKeyRef`**: both inject env vars, but
   from different sources.
4. **No plain-text secret in manifests**: the backend assembles the URL
   from env vars in code, so the password never appears in a `value:`.
5. **Persistence**: todos survive app _and_ DB restarts.
