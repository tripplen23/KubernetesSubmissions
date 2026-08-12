# Exercise 2.1 — Connecting pods

## Goal

> Connect the Log output application and the Ping pong application with
> HTTP. So, instead of sharing data via files, use an HTTP GET endpoint
> in the Ping pong app to respond with the number of pongs for the Log
> output app. Remove the volume between the two applications for the
> time being.

```
 Browser ──► Log output app (reader) ──HTTP GET──► Ping pong app
              │
              └─ shows: <timestamp>: <uuid>.   +  Ping / Pongs: N
```

The response of the HTTP GET to Log output stays the same:

```
2026-05-18T12:15:17.705Z: 8523ecb1-c716-4cb6-a044-b9e83bb98e43.
Ping / Pongs: 3
```

## Prerequisites

**Check k3d cluster with the right port mapping:**

```bash
k3d cluster list
# NAME        SERVERS  AGENTS  LOADBALANCER
# mycluster   1/1      1/1     true

ss -tlnp 2>/dev/null | grep -E ':(8081|8082)'
# LISTEN 0  4096  *:8081  *:*
# LISTEN 0  4096  *:8082  *:*
```

**Check 2 — Traefik Ingress controller:**

```bash
kubectl get pods -n kube-system -l app.kubernetes.io/name=traefik
# traefik-xxxxxxxxxx-xxxxx   1/1  Running
```

## Source code

### `ping-pong/src/main.rs`

The counter is an in-memory `AtomicU64` again.
Two routes:

- `GET /pingpong` → `pong 0`, `pong 1`, ... (increments, returns the
  previous value)
- `GET /pongs` → `3` (the current count, no increment — **this is the
  new HTTP endpoint the Log output app calls**)

### `log-output/src/main.rs`

Two-role structure (`ROLE=writer` / `ROLE=reader`):

- **writer**: every 5s appends `<rfc3339 timestamp>: <uuid>` to
  `timestamp.txt` (in the emptyDir).
- **reader**: serves `GET /` — reads `timestamp.txt` and does
  `reqwest::get(pings_url)` where `PINGS_URL` defaults to
  `http://ping-pong-svc:3000/pongs`. Response:

  ```
  <timestamp>: <uuid>.
  Ping / Pongs: <N>
  ```

> `reqwest` uses `rustls-tls` + `default-features = false` (no OpenSSL
> needed in the final image). **GOTCHA**: on rustc 1.85 you must pin
> `icu_*@2.1.0` + `idna_adapter@1.2.0` via `cargo update --precise`
> (see 1.12). If you copy the `Cargo.lock` from 1.12/1.13 the versions
> are already locked correctly.

To verify locally before writing Dockerfile/manifests:

```bash
cargo build --manifest-path ping-pong/Cargo.toml
cargo build --manifest-path log-output/Cargo.toml

# terminal 1 — ping-pong on port 3002
PORT=3002 ./ping-pong/target/debug/ping-pong

# terminal 2 — log-output writer + reader (ports 3001)
mkdir -p /tmp/2.1-share
PORT=3001 ROLE=writer FILE_PATH=/tmp/2.1-share/timestamp.txt ./log-output/target/debug/log-output
PORT=3001 ROLE=reader FILE_PATH=/tmp/2.1-share/timestamp.txt \
  PINGS_URL=http://localhost:3002/pongs ./log-output/target/debug/log-output

# terminal 3 — test
curl -s localhost:3002/pingpong   # pong 0
curl -s localhost:3002/pingpong   # pong 1
curl -s localhost:3002/pingpong   # pong 2
curl -s localhost:3002/pongs      # 3
sleep 6 && curl -s localhost:3001
# → timestamp lines + "Ping / Pongs: 3"
```

**To stop the servers:** `Ctrl+C` in each foreground terminal, or
`pkill -f 'debug/ping-pong'; pkill -f 'debug/log-output'`.

## Step 1 — Build and push both images

```bash
cd ping-pong
docker build -t tripplen63/ping-pong:2.1 .
docker push tripplen63/ping-pong:2.1

cd ../log-output
docker build -t tripplen63/log-output:2.1 .
docker push tripplen63/log-output:2.1
```

## Step 2 — Apply manifests

```bash
kubectl apply -f manifests/
kubectl get pods
# ping-pong-xxx   1/1  Running
# log-output-xxx  2/2  Running

kubectl get svc
# ping-pong-svc    ClusterIP   10.43.x.x   3000/TCP
# log-output-svc   ClusterIP   10.43.x.x   3000/TCP
```

## Step 3 — Verify over HTTP

```bash
# 1. Ping pong through the Ingress
curl -s http://localhost:8081/pingpong   # pong 0
curl -s http://localhost:8081/pingpong   # pong 1
curl -s http://localhost:8081/pingpong   # pong 2

# 2. Log output through the Ingress — note Ping / Pongs: 3
curl -s http://localhost:8081/log
# 2026-08-12T...: 8523ecb1-...
# 2026-08-12T...: 8523ecb1-...
# Ping / Pongs: 3
```

The number `3` came over HTTP from the ping-pong pod — **no shared
volume involved**.

## Step 4 — The debugging pod (busybox)

The course uses a stand-alone `busybox` Pod to debug pod-to-pod
networking.

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-busybox
  labels:
    app: my-busybox
spec:
  containers:
    - image: busybox
      command:
        - sleep
        - "3600"
      imagePullPolicy: IfNotPresent
      name: busybox
  restartPolicy: Always
```

Apply and test:

```bash
kubectl apply -f busybox.yaml
kubectl get pod my-busybox     # wait until Running

# wget the Log output app by Service name
kubectl exec -it my-busybox -- wget -qO - http://log-output-svc:3000

# or open a shell and run several commands
kubectl exec -it my-busybox -- sh
/ # wget -qO - http://log-output-svc:3000
/ # wget -qO - http://ping-pong-svc:3000/pongs
/ # exit
```

**Verified working** on this cluster (agent-tested): the busybox pod
runs, `wget` resolves the Service DNS names and returns the HTML/json.
Notes:

- busybox has **no `curl`** — use `wget` (as the course warns).
- If you get `Unable to use a TTY`, your terminal emulator doesn't
  support `-it` — drop the `-it` (`kubectl exec my-busybox -- ...`) or
  run from a real terminal.
- Also try hitting the pod IP directly:
  `kubectl get pod -o wide` then
  `kubectl exec -it my-busybox -- wget -qO - http://<POD-IP>:3000`.

Done testing → delete the stand-alone pod (it has no Deployment to
recreate it):

```bash
kubectl delete pod my-busybox
```

## Step 7 — Clean up

```bash
kubectl delete -f manifests/
kubectl delete pod my-busybox 2>/dev/null   # if still around
kubectl get pods,svc
# only service/kubernetes remains
```

## P/S

1. **Service = DNS name inside the cluster.** Pods talk to each other
   via `http://<service-name>:<port>` — that's the whole point of
   "connecting pods" in this chapter.
2. **HTTP instead of shared files.** Decoupled apps: log-output no
   longer needs ping-pong's filesystem, only its HTTP endpoint.
3. **emptyDir vs PV.** Within one pod, emptyDir is enough (writer ↔
   reader). Between pods, use HTTP (or a Service) — not a shared
   volume.
4. **Debugging pods.** A stand-alone busybox Pod is a great way to
   test cluster-internal networking; it has no Deployment, so delete
   it when done.
