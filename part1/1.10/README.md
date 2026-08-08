# Exercise 1.10

## Goal

> Split the "Log output" application into two different containers
> within a single pod:
>
> - One generates a random string on startup and writes a line with the
>   random string and timestamp every 5 seconds into a file.
> - The other reads that file and provides the content in the HTTP GET
>   endpoint for the user to see
>
> You may find
> [this](https://kubernetes.io/docs/reference/kubectl/generated/kubectl_logs/)
> helpful now since there are more than one container running inside a pod.

In short: the log-output app (1.1/1.3/1.7) becomes **two containers in
one pod** that share a file through an **emptyDir volume** — exactly
like the image-finder + image-response example in the course material.

## What you should have before starting

- A running k3d cluster named `mycluster` with port `8081:80@loadbalancer` mapped
- Traefik (k3d's default Ingress controller) running in `kube-system`
- Docker Hub login: `docker login -u tripplen63`
- Working directory: `~/binh/KubernetesSubmissions/part1/1.10`

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

## Goal summary

1. ONE binary, TWO roles — selected by the `ROLE` env var:
   - `ROLE=writer` → generate a random string once, then every 5s
     append `<timestamp> <random string>` to a file
   - `ROLE=reader` → HTTP server, `GET /` returns the file contents
2. ONE Deployment with **two containers** (`writer` + `reader`),
   both mounting the same **emptyDir volume** at
   `/usr/src/app/files`.
3. The reader listens on port 3000; a Service + Ingress expose it at
   `http://localhost:8081/`.
4. Because emptyDir is tied to the pod: delete the pod → data gone →
   writer starts over with a NEW random string.

## Source code (`src/main.rs`)

- `run_writer()`: `Uuid::new_v4()` once at startup, then
  `tokio::time::interval(5s)` appends
  `<rfc3339 timestamp> <uuid>` to `FILE_PATH` (default
  `/usr/src/app/files/timestamp.txt`).
- `run_reader()`: axum `GET /` reads the same `FILE_PATH` and returns
  its contents as plain text (or a placeholder if the file doesn't
  exist yet).
- The file path is configurable via `FILE_PATH` so the same binary
  works locally (`/tmp/...`) and in the pod (`/usr/src/app/files/...`).

To verify the source compiles and runs locally before writing
Dockerfile/manifests:

```bash
cargo build
# terminal 1 — writer:
rm -f /tmp/timestamp.txt
ROLE=writer FILE_PATH=/tmp/timestamp.txt ./target/debug/log-output
# → Writer started, random string: <uuid>
# → 2026-08-08T18:19:47.275Z <uuid>      (every 5s, same uuid)

# terminal 2 — reader:
ROLE=reader PORT=3001 FILE_PATH=/tmp/timestamp.txt ./target/debug/log-output
# → Server started in port 3001

# terminal 3:
curl -s http://localhost:3001/
# → 2026-08-08T18:19:47.275Z <uuid>
# → 2026-08-08T18:19:52.275Z <uuid>
# → ... (grows as the writer appends)
```

**To stop the servers:**

- **Foreground** (no `&`): `Ctrl+C` in each terminal.
- **Background** (`&` or via a tool): `pkill -f log-output`
  (careful while `cargo build` is also running) or
  `fuser -k 3001/tcp`.

## Step 1 — Build the Docker image

```bash
docker build -t tripplen63/log-output:1.10 .
docker images tripplen63/log-output:1.10
# → REPOSITORY             TAG   IMAGE ID       SIZE
# → tripplen63/log-output  1.10  <id>           ~130MB
```

## Step 2 — Test the container locally (no cluster)

The same image runs both roles — only the env vars differ:

```bash
docker run --rm -d --name writer -e ROLE=writer -e FILE_PATH=/usr/src/app/files/timestamp.txt \
  -v logdata:/usr/src/app/files tripplen63/log-output:1.10
```

Wait 6s, then read the file with a second container sharing the
volume:

```bash
docker run --rm -v logdata:/usr/src/app/files \
  tripplen63/log-output:1.10 cat /usr/src/app/files/timestamp.txt
# → 2026-08-08T18:19:47.275Z <uuid>
# → 2026-08-08T18:19:52.275Z <uuid>
```

Then test the reader role (port 3001 on the host):

```bash
docker run --rm -p 3001:3000 -e ROLE=reader -e FILE_PATH=/usr/src/app/files/timestamp.txt \
  -v logdata:/usr/src/app/files tripplen63/log-output:1.10
```

In another terminal:

```bash
curl -s http://localhost:3001/
# → 2026-08-08T18:19:47.275Z <uuid>
# → 2026-08-08T18:19:52.275Z <uuid>
```

Clean up — **stop BOTH containers** (the reader too — a running
container keeps the volume "in use" and `docker volume rm` refuses to
delete it):

```bash
docker stop writer
docker stop <reader-container-name-or-id>   # e.g. from `docker ps`
docker volume rm logdata
```

> **Note**: `-v logdata:/usr/src/app/files` is a named volume — it
> lets two separate containers share the file, mimicking what the
> emptyDir does inside a pod. On the cluster you won't need this:
> both containers live in the SAME pod and share the emptyDir directly.

## Step 3 — Push the image

The `log-output` repo already exists on Docker Hub from 1.1/1.3/1.7. Just push:

```bash
docker push tripplen63/log-output:1.10
```

**Expected last line:**

```
1.10: digest: sha256:... size: ...
```

## Step 4 — Apply manifests

Then:

```bash
kubectl apply -f manifests/
kubectl get pods
# → NAME                         READY   STATUS    RESTARTS   AGE
# → log-output-xxxxxxxxxx-xxx    2/2     Running   0          8s
#   (2/2 = two containers both running!)

kubectl get svc
# → NAME         TYPE        CLUSTER-IP     PORT(S)    AGE
# → log-output   ClusterIP   10.43.x.x      3000/TCP   5s
```

## Step 5 — Verify both containers

Watch the writer's logs (container flag!):

```bash
kubectl logs -f deployment/log-output -c writer
# → Writer started, random string: <uuid>
# → 2026-08-08T18:19:47.275Z <uuid>     (every 5s, same uuid)
```

Check the reader's logs:

```bash
kubectl logs deployment/log-output -c reader
# → Server started in port 3000
```

## Step 6 — Access the app through the Ingress

```bash
curl -s http://localhost:8081/
# → 2026-08-08T18:19:47.275Z <uuid>
# → 2026-08-08T18:19:52.275Z <uuid>
# → ... (grows every 5s — refresh the browser to see new lines)
```

Open your browser: `http://localhost:8081/` → the growing list of
timestamp + random string lines.

## Step 7 — Prove emptyDir is ephemeral

The file lives on the pod's emptyDir — delete the pod and the data is
gone (new pod, new random string):

```bash
kubectl delete pod -l app=log-output
# wait for the new pod (2/2 Running)
kubectl logs deployment/log-output -c writer | head -1
# → Writer started, random string: <DIFFERENT uuid>
```

The old lines are gone — the new writer started from an empty file
with a fresh random string. This is exactly the emptyDir lifecycle the
course material describes.

## Step 8 — Clean up

```bash
kubectl delete -f manifests/
kubectl get all -l app=log-output
# → No resources found in default namespace.
```

## Last note

1. **emptyDir volume** = shared filesystem INSIDE a pod: both
   containers mount it at the same `mountPath` and see the same files.
2. Volume lifecycle is tied to the **pod**: container restarts keep
   the data, pod restart/deletion wipes it.
3. One pod can run **multiple containers** — they share the pod's
   network namespace and volumes, but have isolated filesystems
   unless you explicitly mount a volume.
4. `kubectl logs` needs `-c <container>` when a pod has more than one
   container.
5. This is why the course says: emptyDir for caches and
   inter-container sharing, NOT for databases — next up: Persistent
   Volumes (1.11) which survive pod death.
