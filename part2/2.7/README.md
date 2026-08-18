# Exercise 2.7 — Stateful applications (Postgres)

## Goal

> Run a **Postgres** database as a **stateful set** (with one replica)
> and save the **Ping-pong** application counter into the database.

The Ping-pong counter moves from in-memory (`AtomicU64`) to a Postgres
database. The database itself runs as a **StatefulSet** — the correct
resource for stateful workloads — and uses a **headless Service** for
network identity plus **dynamic provisioning** for its storage (no manual
PV this time; K3s' `local-path` provisioner creates it on demand).

## Concepts covered (read the course page first)

- <https://courses.mooc.fi/org/uh-cs/courses/devops-with-kubernetes-2026/chapter-3/statefulsets-and-jobs>

| Question                                         | Answer                                                                                                      |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| Deployment vs StatefulSet?                       | Deployment = stateless replicas sharing storage; StatefulSet = stable identity + **own volume per replica** |
| Why "headless" service?                          | `clusterIP: None` → no load balancing; DNS resolves to each pod's IP (`redis-stset-0.redis-svc`)            |
| `volumeClaimTemplates`?                          | Creates a **separate PVC per replica** (unlike a shared volume in a Deployment)                             |
| Manual PV needed?                                | **No** — `storageClassName: local-path` uses dynamic provisioning; K3s creates the PV for you               |
| Does deleting the StatefulSet delete the volume? | **No** — the PVC/PV survive and re-bind when you re-apply                                                   |

## Source code changes (ping-pong)

The counter was `Arc<AtomicU64>`. It is now a row in Postgres. The app:

1. Reads `DATABASE_URL` (a `postgres://…` URL) — **no default, panics if missing** (2.6 style)
2. Connects **with retry** — the DB may still be starting, so it retries up to 30× with a 2s backoff before panicking
3. `CREATE TABLE IF NOT EXISTS pongs (id SERIAL PRIMARY KEY, count BIGINT NOT NULL DEFAULT 0)` + seeds row `(1, 0)`
4. `GET /pingpong` → `UPDATE pongs SET count = count + 1 WHERE id = 1 RETURNING count`, replies `pong {new-1}` (same behaviour as before)
5. `GET /pongs` → `SELECT count FROM pongs WHERE id = 1`

Cargo.toml adds the async Postgres client:

```toml
[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
tokio-postgres = "0.7"
```

## Step 1 — build + push Dockerfile

```bash
cd ping-pong/
docker build -t tripplen63/ping-pong:2.7 .
docker push tripplen63/ping-pong:2.7
```

## Step 2 — Apply and verify manifests

```bash
kubectl apply -f manifests/

kubectl get statefulset,pvc,pods -n exercises
# statefulset.apps/postgres-ss   1/1
# persistentvolumeclaim/data-postgres-ss-0   Bound   ...   100Mi   RWO   local-path
# pod/postgres-ss-0              1/1  Running
# pod/ping-pong-xxx              1/1  Running

# bump the counter — it's now persisted in Postgres
curl -s http://localhost:8081/pingpong   # pong 0
curl -s http://localhost:8081/pingpong   # pong 1
curl -s http://localhost:8081/pingpong   # pong 2
```

## Step 3 — Prove it's really stateful (the whole point)

**A. The counter survives a Ping-pong pod restart** (it's in the DB now):

```bash
kubectl delete pod -n exercises -l app=ping-pong
sleep 15
kubectl get pods -n exercises | grep ping-pong
# ping-pong-xxx   1/1  Running   (a brand-new pod)

curl -s http://localhost:8081/pingpong   # pong 3  ← NOT pong 0!
```

> Before 2.7 the counter reset to 0 on restart. Now the replacement pod
> reconnects to Postgres and continues from 3.

**B. The DB volume survives a Postgres pod restart** (StatefulSet
identity + storage):

```bash
kubectl delete pod postgres-ss-0 -n exercises
kubectl wait --for=condition=ready pod/postgres-ss-0 -n exercises --timeout=90s
sleep 5
kubectl get pods -n exercises | grep postgres
# postgres-ss-0   1/1  Running   (same name — stable identity)

curl -s http://localhost:8081/pingpong   # pong 4  ← counter kept!
```

> The app opens a fresh connection per request, so once Postgres is
> ready again it recovers automatically — no manual restart needed.

**C. Debugging the DB directly** (course hint):

```bash
kubectl run psql-debug -it --rm --restart=Never --image postgres:16 -n exercises -- sh
# inside the pod:
$ psql postgres://postgres:example@postgres-svc:5432/postgres
postgres=# SELECT * FROM pongs;
#  id | count
# ----+-------
#   1 |     4
```

## Step 4 — Clean up

```bash
kubectl delete -f manifests/
# Note: deleting the StatefulSet does NOT delete its PVC/PV (data safety).
kubectl delete pvc -n exercises -l app=postgres 2>/dev/null
kubectl delete pv $(kubectl get pv -o name | grep postgres) 2>/dev/null
kubectl get statefulset,pods,pvc -n exercises
# No resources found
```

> **StatefulSet data-safety feature**: `kubectl delete -f manifests/`
> removes the StatefulSet but leaves the PVC/PV behind (by design), so
> your data survives and re-binds if you re-apply. To clean fully you
> must delete the PVC (and the dynamically-provisioned PV) explicitly.

## P/S

1. **StatefulSets** for stateful apps — stable pod identity (`postgres-ss-0`)
   and a **dedicated volume per replica** via `volumeClaimTemplates`.
2. **Headless service** (`clusterIP: None`) gives each pod a stable DNS
   name without load balancing.
3. **Dynamic provisioning** (`local-path`): no manual PV — K3s creates
   the volume on demand from the PVC template.
4. **Persistence in practice**: the counter survives app restarts _and_
   DB restarts; deleting the StatefulSet keeps the data.
