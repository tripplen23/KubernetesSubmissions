# Exercise 1.8

## Goal

> Switch to using Ingress instead of NodePort to access the project. You
> can delete the Ingress of the "Log output" application so they don't
> interfere with this exercise. We'll look more into paths and routing in
> the next exercise, and at that point, you can configure the project to
> run with the "Log output" application side by side.

In short: the **todo-app** (the project, from 1.2/1.5/1.6) is now
reached through an **Ingress** instead of a NodePort Service.

## What you should have before starting

- A running k3d cluster named `mycluster` with port `8081:80@loadbalancer` mapped
- Traefik (k3d's default Ingress controller) running in `kube-system`
- Docker Hub login: `docker login -u tripplen63`
- Working directory: `~/binh/KubernetesSubmissions/part1/1.8`

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

- Same as 1.6: axum 0.8, `GET /` → HTML landing page,
  `GET /api/health` → `{"status":"ok"}`, `GET /api/todos` → `[]`.
- Listens on `0.0.0.0:$PORT` (default 3000), prints
  `Server started in port NNNN`.
- Only the HTML text changed (says "reached via an Ingress" now).

To verify the source compiles and runs locally before writing
Dockerfile/manifests:

```bash
cargo build
PORT=3001 ./target/debug/todo-app       # in terminal 1
# → Server started in port 3001

curl -s http://localhost:3001/api/health   # in terminal 2
# → {"status":"ok"}
```

**To stop the server:**

- **Foreground** (no `&`): `Ctrl+C` in terminal 1.
- **Background** (`&` or via a tool): `pkill -f todo-app`
  (matches anything with "todo-app" in the command line — careful
  while `cargo build` is also running) or `fuser -k 3001/tcp`.

## Step 1 — Build the Docker image

```bash
docker build -t tripplen63/todo-app:1.8 .
docker images tripplen63/todo-app:1.8
# → REPOSITORY            TAG  IMAGE ID       SIZE
# → tripplen63/todo-app   1.8  <id>           ~130MB
```

## Step 2 — Test the container locally (no cluster)

```bash
docker run --rm -p 3001:3000 tripplen63/todo-app:1.8
```

In another terminal:

```bash
curl -s http://localhost:3001/
# → <!doctype html>... (Todo App landing page)
curl -s http://localhost:3001/api/health
# → {"status":"ok"}
```

Stop the container with `Ctrl+C` (foreground) or
`docker stop <name>` (background).

## Step 3 — Push the image

The `todo-app` repo already exists on Docker Hub from 1.2/1.4/1.5/1.6. Just push:

```bash
docker push tripplen63/todo-app:1.8
```

**Expected last line:**

```
1.8: digest: sha256:... size: ...
```

## Step 4 — Apply manifests

Then:

```bash
kubectl apply -f manifests/
kubectl get pods -l app=todo-app
# → NAME                       READY   STATUS    RESTARTS   AGE
# → todo-app-xxxxxxxxxx-xxx    1/1     Running   0          8s

kubectl get svc todo-app
# → NAME       TYPE        CLUSTER-IP     PORT(S)    AGE
# → todo-app   ClusterIP   10.43.x.x      3000/TCP   5s

kubectl get ingress
# → NAME       CLASS     HOSTS       ADDRESS                 PORTS   AGE
# → todo-app   traefik   localhost   172.18.0.3,172.18.0.4   80      ...
```

## Step 5 — Remove the log-output Ingress

Exercise 1.7 created an Ingress (or two) for the log-output app. Its
rules on host `localhost` would interfere with this exercise. Delete
only the Ingress resources (and any 1.7 middlewares):

```bash
kubectl delete ingress apps-log apps-todo      # names from 1.7, if present
kubectl delete middleware stripprefix-log stripprefix-todo   # if present
```

Verify only the todo-app Ingress remains:

```bash
kubectl get ingress
# → NAME       CLASS     HOSTS       ADDRESS                 PORTS   AGE
# → todo-app   traefik   localhost   172.18.0.3,172.18.0.4   80      ...
```

> **Why remove it?** Two Ingresses with rules on the same
> host (`localhost`) race for the same traffic. The log-output rules
> from 1.7 match `PathPrefix(/log)` — harmless here — but keeping the
> cluster tidy matches the assignment: log-output and the project run
> side by side only in the NEXT exercise (paths & routing).

## Step 6 — Access the app through the Ingress

```bash
curl -s http://localhost:8081/
# → <!doctype html>... (Todo App landing page)

curl -s http://localhost:8081/api/health
# → {"status":"ok"}

curl -s http://localhost:8081/api/todos
# → []
```

Open your browser:

- `http://localhost:8081/` → todo-app HTML
- `http://localhost:8081/api/health` → `{"status":"ok"}`

## Step 7 — Clean up

```bash
kubectl delete -f manifests/
kubectl get all -l app=todo-app
# → No resources found in default namespace.
```
