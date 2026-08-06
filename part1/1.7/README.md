# Exercise 1.7

## Prerequisite

- A running k3d cluster named `mycluster` with port `8081:80@loadbalancer` mapped
- Traefik (k3d's default Ingress controller) running in `kube-system`
- Docker Hub login: `docker login -u tripplen63`
- Working directory: `~/binh/KubernetesSubmissions/part1/1.7`

The **todo-app** Ingress is still deployed from exercise 1.6. Re-apply it if needed, or run both side by side.

### How to check the two prerequisites

**Check 1 — k3d cluster with the right port mapping:**

```bash
k3d cluster list
# Expected output:
# NAME        SERVERS  AGENTS  LOADBALANCER
# mycluster   1/1      1/1     true

ss -tlnp 2>/dev/null | grep -E ':(8081|8082)'
# Expected:
# LISTEN 0  4096  *:8081  *:*
# LISTEN 0  4096  *:8082  *:*
```

- If you see the `mycluster` row with `LOADBALANCER: true` and both
  `8081` + `8082` listening, you're set.
- If `k3d cluster list` is empty or the cluster name differs, recreate
  it (see `k3d cluster create mycluster --agents 1 --port 8081:80@loadbalancer --port 8082:30080@loadbalancer`).
- If the port is not listening, the cluster exists but the port
  mapping wasn't set up. Recreate the cluster (k3d can't add port
  mappings to a running cluster — they must be set at `create` time).

**Check 2 — Traefik Ingress controller:**

```bash
kubectl get pods -n kube-system -l app.kubernetes.io/name=traefik
# Expected:
# NAME                       READY   STATUS    RESTARTS   AGE
# traefik-xxxxxxxxxx-xxxxx   1/1     Running   0          ...
```

- `1/1 Running` = Traefik is healthy. You're set.
- `0/1` for a few minutes right after `k3d cluster create` is normal
  — wait 30s and re-check.
- `ImagePullBackOff` or `ErrImagePull` = Docker image not available.
  Run `kubectl describe pod -n kube-system -l app.kubernetes.io/name=traefik`
  for the cause.

## Goal summary

1. Build a new `log-output` binary that:
   - keeps the existing "print timestamp + random string every 5s" behaviour
   - also serves `GET /status` over HTTP, returning
     `{"timestamp": "...", "random_string": "..."}` as JSON
2. Containerise it (`tripplen63/log-output:1.7`)
3. Deploy it on the cluster with an **Ingress** so the browser can hit it
4. Use path-based routing on the same host (`localhost`) — different
   paths go to different apps:
   - `http://localhost:8081/todo/...` → todo-app
   - `http://localhost:8081/log/...` → log-output

## Source code (`src/main.rs`)

- `RANDOM_STRING` is a `std::sync::OnceLock<String>` set once at startup.
  Both the background log task and the `/status` HTTP handler read it.
- Background task uses `tokio::time::interval(5s)` and prints
  `<rfc3339_timestamp> <uuid>` to stdout.
- HTTP server uses axum 0.8, listens on `0.0.0.0:$PORT` (default 3000).
- Route: `GET /status` → JSON.

To verify the source compiles and runs locally before writing
Dockerfile/manifests:

```bash
cargo build
PORT=3001 ./target/debug/log_output       # in terminal 1
# → Server started in port 3001
# → 2026-07-30T18:12:13.856Z <uuid>     (every 5s)

curl -s http://localhost:3001/status        # in terminal 2
# → {"timestamp":"...","random_string":"..."}
```

**To stop the server:**

- **If you ran it foreground** (no `&`): `Ctrl+C` in terminal 1.
- **If you ran it background** (`&` or via a tool): the process is
  detached from your shell. Pick one:
  ```bash
  # by name (most reliable)
  pkill -f log_output
  # or by port
  fuser -k 3001/tcp
  # or via Docker if it's a container
  docker ps | grep log-output
  docker stop <container-name>
  ```
  `pkill -f log_output` will match anything with "log_output" in the
  command line, including the cargo build process, so use it
  carefully when also building.

## Step 1 — Build the Docker image

```bash
docker build -t tripplen63/log-output:1.7 .
docker images tripplen63/log-output:1.7
# → REPOSITORY              TAG  IMAGE ID       SIZE
# → tripplen63/log-output   1.7  <id>           ~200MB
```

## Step 2 — Test the container locally (no cluster)

```bash
docker run --rm -p 3001:3000 tripplen63/log-output:1.7
```

In another terminal:

```bash
curl -s http://localhost:3001/status
# → {"timestamp":"2026-07-30T...","random_string":"<uuid>"}
```

## Step 3 — Push the image

The `log-output` repo already exists on Docker Hub from 1.1/1.3. Just push:

```bash
docker push tripplen63/log-output:1.7
```

**Expected last line:**

```
1.7: digest: sha256:... size: ...
```

## Step 4 — Apply manifests

```bash
kubectl apply -f manifests/
kubectl get pods -l app=log-output
# → NAME                         READY   STATUS    RESTARTS   AGE
# → log-output-xxxxxxxxxx-xxx    1/1     Running   0          8s

kubectl get svc -l app=log-output
# → NAME            TYPE        CLUSTER-IP   PORT(S)    AGE
# → log-output      ClusterIP   10.43.x.x    3000/TCP   5s

kubectl get ingress
# → NAME        CLASS     HOSTS       ADDRESS                 PORTS   AGE
# → apps-log    traefik   localhost   172.18.0.3,172.18.0.4   80      ...
# → apps-todo   traefik   localhost   172.18.0.3,172.18.0.4   80      ...
```

## Step 5 — Access the app through the Ingress

```bash
curl -s -H "Host: localhost" http://localhost:8081/log/status
# → {"timestamp":"...","random_string":"..."}
```

Open your browser:

- `http://localhost:8081/log/status` → log-output JSON
- `http://localhost:8081/todo/` → todo-app HTML
- `http://localhost:8081/todo/api/health` → todo-app health

> **Path routing**: `pathType: Prefix` means "this rule applies to
> `/log` and any path that starts with `/log`" (so `/log/status` and
> `/log/whatever` both hit log-output). If todo-app is on
> `path: /todo` (Prefix), the Ingress routes `/todo` and `/todo/...`
> to todo-app.

## Step 6 — Verify the background log task

```bash
kubectl logs -f -l app=log-output
```

Wait 5–10s. You should see lines like:

```
2026-07-30T18:12:13.856Z a881d441-08f2-4826-aed3-807a939d428d
2026-07-30T18:12:18.856Z a881d441-08f2-4826-aed3-807a939d428d
```

The UUID stays the same across all log lines (because `OnceLock`).
The timestamp changes every 5s.

## Step 7 — Clean up

```bash
kubectl delete -f manifests/
kubectl get all -l app=log-output
# → No resources found in default namespace.
```

(If you also edited todo-app's ingress in `part1/1.6/manifests/`, leave
it alone or clean up separately — depends on which option you chose
in Step 4.)
