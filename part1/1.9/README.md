# Exercise 1.9

## Goal

> Develop a second application that simply responds with "pong 0" to a
> GET request and increases a counter (the 0) so that you can see how
> many requests have been sent. The counter should be in memory so it
> may reset at some point. Create a new deployment for it and have it
> share ingress with "Log output" application. Route requests directed
> '/pingpong' to it.
>
> In future exercises, this second application will be referred to as
> "ping-pong application". It will be used with "Log output" application.

In short: a new **ping-pong** app (`GET /pingpong` → `pong 0`, `pong 1`,
...) shares one Ingress with the **log-output** app from 1.7.

## What you should have before starting

- A running k3d cluster named `mycluster` with port `8081:80@loadbalancer` mapped
- Traefik (k3d's default Ingress controller) running in `kube-system`
- Docker Hub login: `docker login -u tripplen63`
- Working directory: `~/binh/KubernetesSubmissions/part1/1.9`

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

**Check 2 — Traefik Ingress controller:**

```bash
kubectl get pods -n kube-system -l app.kubernetes.io/name=traefik
# Expected:
# NAME                       READY   STATUS    RESTARTS   AGE
# traefik-xxxxxxxxxx-xxxxx   1/1     Running   0          ...
```

## Source code (`src/main.rs`)

- `static COUNTER: AtomicU64` — in-memory counter, `fetch_add(1)` per
  request.
- Route: `GET /pingpong` → `format!("pong {}", n)`.
- axum 0.8, listens on `0.0.0.0:$PORT` (default 3000).

To verify the source compiles and runs locally before writing Dockerfile/manifests:

```bash
cargo build
PORT=3001 ./target/debug/ping-pong       # in terminal 1
# → Server started in port 3001

curl -s http://localhost:3001/pingpong   # in terminal 2, run 3x
# → pong 0
# → pong 1
# → pong 2
```

**To stop the server:**

- **Foreground** (no `&`): `Ctrl+C` in terminal 1.
- **Background** (`&` or via a tool): `pkill -f ping-pong`
  (careful while `cargo build` is also running) or `fuser -k 3001/tcp`.

## Step 1 — Build the Docker image

```bash
docker build -t tripplen63/ping-pong:1.9 .
docker images tripplen63/ping-pong:1.9
# → REPOSITORY             TAG  IMAGE ID       SIZE
# → tripplen63/ping-pong   1.9  <id>           ~130MB
```

## Step 2 — Test the container locally (no cluster)

```bash
docker run --rm -p 3001:3000 tripplen63/ping-pong:1.9
```

In another terminal:

```bash
curl -s http://localhost:3001/pingpong
# → pong 0
curl -s http://localhost:3001/pingpong
# → pong 1
```

Stop the container with `Ctrl+C` (foreground) or
`docker stop <name>` (background).

## Step 3 — Push the image

Create the Docker Hub repo first if this is the first push of the
`ping-pong` repository: <https://hub.docker.com/repository-create>
(name `ping-pong`, Public). Then:

```bash
docker push tripplen63/ping-pong:1.9
```

**Expected last line:**

```
1.9: digest: sha256:... size: ...
```

## Step 4 — Apply manifests

```bash
kubectl apply -f manifests/
kubectl get pods
# → NAME                         READY   STATUS    RESTARTS   AGE
# → log-output-xxxxxxxxxx-xxx    1/1     Running   0          8s
# → ping-pong-xxxxxxxxxx-xxx     1/1     Running   0          8s

kubectl get ingress
# → NAME   CLASS     HOSTS       ADDRESS                 PORTS   AGE
# → apps   traefik   localhost   172.18.0.3,172.18.0.4   80      ...
```

## Step 5 — Remove the todo-app Ingress (from 1.8)

Exercise 1.8 left a `todo-app` Ingress on host `localhost`. Delete it
so it doesn't claim `/` and shadow the new paths:

```bash
kubectl delete ingress todo-app     # if present
```

Verify only the `apps` Ingress remains:

```bash
kubectl get ingress
# → NAME   CLASS     HOSTS       ADDRESS                 PORTS   AGE
# → apps   traefik   localhost   172.18.0.3,172.18.0.4   80      ...
```

## Step 6 — Access the apps through the Ingress

```bash
curl -s http://localhost:8081/pingpong
# → pong 0
curl -s http://localhost:8081/pingpong
# → pong 1
curl -s http://localhost:8081/pingpong
# → pong 2

curl -s http://localhost:8081/log/status
# → {"timestamp":"...","random_string":"..."}
```

Open your browser:

- `http://localhost:8081/pingpong` → `pong 0`, refresh → `pong 1`, ...
- `http://localhost:8081/log/status` → log-output JSON

> Hit `/pingpong` several times — the counter only
> goes UP. Now `kubectl delete pod -l app=ping-pong` and hit it again.
> Why does it restart at `pong 0`?

## Step 7 — Clean up

```bash
kubectl delete -f manifests/
kubectl get all -l app=ping-pong
# → No resources found in default namespace.
```
