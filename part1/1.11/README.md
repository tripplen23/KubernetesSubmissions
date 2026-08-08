# Exercise 1.11

## Goal

> Let's share data between "Ping-pong" and "Log output" applications
> using persistent volumes. Create both a PersistentVolume and
> PersistentVolumeClaim and alter the Deployment to utilize it. As
> PersistentVolumes are often maintained by cluster administrators
> rather than developers and those are not application specific you
> should keep the definition for those separated, perhaps in own
> folder.
>
> Save the number of requests to the "Ping-pong" application into a
> file in the volume and output it with the timestamp and the random
> string when sending a request to our "Log output" application. In the
> end, the two pods should share a persistent volume between the two
> applications. So the browser should display the following when
> accessing the "Log output" application:
>
> ```
> 2020-03-30T12:15:17.705Z: 8523ecb1-c716-4cb6-a044-b9e83bb98e43.
> Ping / Pongs: 3
> ```

In short: **two separate pods** (ping-pong + log-output) share ONE
PersistentVolume via a PVC — the ping-pong counter lives in a file on
the volume, and log-output reads both files to render
`<timestamp>: <random string>` + `Ping / Pongs: N`.

## What you should have before starting

- A running k3d cluster named `mycluster` with port `8081:80@loadbalancer` mapped
- Traefik (k3d's default Ingress controller) running in `kube-system`
- Docker Hub login: `docker login -u tripplen63`
- Working directory: `~/binh/KubernetesSubmissions/part1/1.11`

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

1. **Two apps, two pods, ONE volume.** ping-pong (1.9) and log-output
   (1.10) become separate Deployments again, but both mount the SAME
   PersistentVolumeClaim.
2. **ping-pong changes**: the in-memory `AtomicU64` counter (1.9) is
   replaced with a file on the shared volume — `pings.txt`. Every
   request reads the number, writes `N+1`, replies `pong N`. The count
   now survives pod restarts (that's the point of "persisting data").
3. **log-output changes**: the reader now also reads `pings.txt` and
   appends `Ping / Pongs: N` under the log lines.
4. **PV is admin-owned** → its definition lives in a separate folder
   (`manifests/pv/`), NOT next to the app manifests, as the exercise
   instructs.
5. Volume data survives pod deletion — delete both pods, they come
   back, and the ping-pong counter continues where it left off.

## Source code

Two separate crates under `part1/1.11/`:

```
part1/1.11/
├── ping-pong/          # from 1.9 — counter now persisted to a file
│   ├── src/main.rs
│   ├── Cargo.toml
│   └── Cargo.lock
├── log-output/         # from 1.10 — reader adds "Ping / Pongs: N"
│   ├── src/main.rs
│   ├── Cargo.toml
│   └── Cargo.lock
└── README.md
```

### `ping-pong/src/main.rs`

- `GET /pingpong` → reads `pings.txt` (default
  `/usr/src/app/files/pings.txt`, overridable via `PINGS_FILE`),
  writes `N+1` back, replies `pong N`.
- No `AtomicU64` anymore — the file IS the counter.

### `log-output/src/main.rs`

- `ROLE=writer` — same as 1.10, but the line format is now
  `<timestamp>: <random string>` (matching the expected output).
- `ROLE=reader` — `GET /` reads `timestamp.txt` AND `pings.txt`
  (env `PINGS_PATH`, default `/usr/src/app/files/pings.txt`), and
  returns:
  ```
  <timestamp>: <random string>
  ...
  Ping / Pongs: N
  ```

To verify both crates compile and work together locally before writing
Dockerfile/manifests:

```bash
cargo build --manifest-path ping-pong/Cargo.toml
cargo build --manifest-path log-output/Cargo.toml
mkdir -p /tmp/1.11-share

# terminal 1 — log-output writer:
ROLE=writer FILE_PATH=/tmp/1.11-share/timestamp.txt ./log-output/target/debug/log-output

# terminal 2 — log-output reader:
ROLE=reader PORT=3001 FILE_PATH=/tmp/1.11-share/timestamp.txt \
  PINGS_PATH=/tmp/1.11-share/pings.txt ./log-output/target/debug/log-output

# terminal 3 — ping-pong:
PORT=3002 PINGS_FILE=/tmp/1.11-share/pings.txt ./ping-pong/target/debug/ping-pong

# terminal 4 — hit ping-pong 3x, then the reader:
curl -s http://localhost:3002/pingpong   # → pong 0
curl -s http://localhost:3002/pingpong   # → pong 1
curl -s http://localhost:3002/pingpong   # → pong 2
curl -s http://localhost:3001/           # → <timestamp>: <uuid> ... Ping / Pongs: 3
```

Kill ping-pong and restart it — the counter continues (`pong 3`),
because the count lives in `/tmp/1.11-share/pings.txt`, not in memory.

**To stop the servers:**

- **Foreground** (no `&`): `Ctrl+C` in each terminal.
- **Background** (`&` or via a tool): `pkill -f log-output` /
  `pkill -f ping-pong` or `fuser -k 3001/tcp 3002/tcp`.

## Step 1 — Prepare the node directory for the local PV

The PV uses a **local** path — storage lives on a cluster node, not in
the pod. Create the directory on the agent node first:

```bash
docker exec k3d-mycluster-agent-0 mkdir -p /tmp/kube
docker exec k3d-mycluster-agent-0 ls -ld /tmp/kube
# → drwxr-xr-x 1 root root 4096 ... /tmp/kube
```

## Step 2 — Apply manifest the PersistentVolume (admin-owned, separate folder)

Apply it:

```bash
kubectl apply -f manifests/pv/persistentvolume.yaml
kubectl get pv
# → NAME        CAPACITY  ACCESS MODES  RECLAIM POLICY  STATUS  ...
# → example-pv  1Gi       RWO           Retain          Available
```

> **Concept check**: why does a local PV need `nodeAffinity`? (Hint:
> the data physically lives on ONE node's disk — pods on other nodes
> can't reach it.)

## Step 3 — Apply manifest the PersistentVolumeClaim

```bash
kubectl apply -f manifests/persistentvolumeclaim.yaml
kubectl get pvc
# → NAME         STATUS  VOLUME      CAPACITY  ACCESS MODES  STORAGECLASS
# → image-claim  Bound   example-pv  1Gi       RWO           my-example-pv
kubectl get pv
# → example-pv is now Bound (not Available) — the PVC claimed it
```

> **Concept check**: PVC is `Bound` — the claim matched the PV by
> `storageClassName` + capacity + access mode. If no PV matched, the
> PVC would stay `Pending`.

## Step 4 — Build and push both images

```bash
cd ~/binh/KubernetesSubmissions/part1/1.11/ping-pong
docker build -t tripplen63/ping-pong:1.11 .
docker push tripplen63/ping-pong:1.11

cd ~/binh/KubernetesSubmissions/part1/1.11/log-output
docker build -t tripplen63/log-output:1.11 .
docker push tripplen63/log-output:1.11
```

## Step 5 — Apply the app manifests

```bash
kubectl apply -f manifests/
kubectl get pods
# → NAME                         READY   STATUS    RESTARTS   AGE
# → log-output-xxxxxxxxxx-xxx    2/2     Running   0          10s
# → ping-pong-xxxxxxxxxx-xxx     1/1     Running   0          10s
```

## Step 6 — Verify the shared volume

Check the volume is mounted in BOTH pods:

```bash
kubectl exec deployment/ping-pong -- ls -la /usr/src/app/files/
# → total 0
# → (empty at first — nothing has written yet)
```

Send 3 requests to ping-pong:

```bash
curl -s http://localhost:8081/pingpong
# → pong 0
curl -s http://localhost:8081/pingpong
# → pong 1
curl -s http://localhost:8081/pingpong
# → pong 2
```

Now check the file inside the ping-pong pod:

```bash
kubectl exec deployment/ping-pong -- cat /usr/src/app/files/pings.txt
# → 3
```

And check log-output sees the same file (the PVC is shared!):

```bash
kubectl exec deployment/log-output -c reader -- cat /usr/src/app/files/pings.txt
# → 3
```

## Step 7 — Access the Log output application

```bash
curl -s http://localhost:8081/log
# → 2026-08-08T20:32:43.883Z: 916c22be-54a4-4e3a-8eac-39e5f174e1c2
# → 2026-08-08T20:32:48.883Z: 916c22be-54a4-4e3a-8eac-39e5f174e1c2
# → ...
# → Ping / Pongs: 3
```

Open your browser: `http://localhost:8081/log` → the expected output:

```
2026-08-08T20:32:43.883Z: 916c22be-... .
Ping / Pongs: 3
```

Hit `/pingpong` a few more times, refresh `/log` — `Ping / Pongs`
grows.

## Step 8 — Prove the data persists

Delete BOTH deployments' pods and let them recreate:

```bash
kubectl delete pod -l app=ping-pong
kubectl delete pod -l app=log-output
kubectl get pods
# → both back to Running within seconds
```

The counter survives because it lives on the PV, not in memory:

```bash
curl -s http://localhost:8081/pingpong
# → pong 3   (NOT pong 0 — the count persisted!)

curl -s http://localhost:8081/log
# → ... Ping / Pongs: 4
```

## Step 9 — Clean up

```bash
kubectl delete -f manifests/
kubectl delete -f manifests/pv/persistentvolume.yaml
kubectl get pv,pvc
# → No resources found
```

## P/S:

1. **PV** = cluster-wide storage resource, provisioned by an admin,
   independent of any pod's lifecycle. For local k3s you create a
   `local` PV pointing at a node path + `nodeAffinity`.
2. **PVC** = developer's claim; Kubernetes binds it to a matching PV
   by `storageClassName` / capacity / access mode. No match → `Pending`.
3. **PVs are admin-owned** → keep their definitions in a separate
   folder from app manifests (as the exercise demands).
4. Two DIFFERENT pods can share one PV by mounting the same PVC —
   that's how ping-pong's counter file becomes visible to log-output.
5. Persistent data survives pod deletion — unlike emptyDir (1.10) and
   unlike in-memory state (1.9).
