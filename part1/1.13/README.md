# Exercise 1.13

## Goal

> It is time to start adding some real functionality to our project!
> As promised, the project shall have a todo app functionality. So in
> this exercise
>
> - add an input field. The input should not take todos that are over
>   140 characters long.
> - add a send button. It does not have to send the todo yet.
> - add a list of the existing todos with some hardcoded todos.

### Prerequisites

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

1. `GET /` renders the todo-app HTML page with:
   - the hourly image from 1.12,
   - an **input field** (`maxlength="140"` — the browser refuses
     typing more than 140 characters),
   - a **Send button** (client-side JS shows an alert, no request is
     sent yet — that's fine per the exercise),
   - a **list of hardcoded todos** (3 items, one done, two open).
2. `GET /api/todos` returns the same hardcoded list as JSON — the
   placeholder the project will replace with real persistence in a
   later exercise.
3. `/image`, `/api/health` and `/shutdown` are unchanged from 1.12.

## Source code (`src/main.rs`)

```bash
cargo build
mkdir -p /tmp/1.13-share
PORT=3001 IMAGE_PATH=/tmp/1.13-share/image.jpg ./target/debug/todo-app

# another terminal:
curl -s http://localhost:3001/ | grep 'maxlength="140"'   # → present
curl -s http://localhost:3001/ | grep 'send-btn'           # → present
curl -s http://localhost:3001/api/todos                    # → JSON list of 3
curl -s http://localhost:3001/image -o /tmp/img.jpg        # → still works
```

**To stop the server:** foreground → `Ctrl+C`; background →
`pkill -f todo-app` or `fuser -k 3001/tcp`.

## Step 1 — Prepare the node directory for the local PV

```bash
docker exec k3d-mycluster-agent-0 mkdir -p /tmp/kube
docker exec k3d-mycluster-agent-0 ls -ld /tmp/kube
# → drwxr-xr-x 1 root root 4096 ... /tmp/kube
```

> If you did 1.12 the folder already exists — the command is safe to
> re-run. The old `image.jpg` (and 1.11's `pings.txt` /
> `timestamp.txt`) are still there; the PV is reused as-is.

## Step 2 — PersistentVolume + PersistentVolumeClaim

Apply and check they bind:

```bash
kubectl apply -f manifests/pv/persistentvolume.yaml
kubectl apply -f manifests/persistentvolumeclaim.yaml
kubectl get pv,pvc
# → example-pv   Bound   image-claim
```

## Step 3 — Build and push the image

```bash
docker build -t tripplen63/todo-app:1.13 .
docker push tripplen63/todo-app:1.13
```

## Step 4 — Apply the app manifests

```bash
kubectl apply -f manifests/
kubectl get pods
# → todo-app-xxxxxxxxxx-xxx   1/1  Running
```

## Step 5 — Verify the todo UI

Open `http://localhost:8081/` in the browser. You should see:

1. the **hourly picture** (from 1.12),
2. an **input field** — try typing more than 140 characters: the
   browser stops you at 140,
3. a **Send button** — click it: an alert appears
   ("Sending not implemented yet..."), no request is sent,
4. a **list of 3 todos** (2 checked/done, 1 open).

Verify the endpoints:

```bash
curl -s http://localhost:8081/api/todos | python3 -m json.tool
# → 3 todos, first two done:true, last one done:false

curl -s http://localhost:8081/ | grep 'maxlength="140"'
# → <input type="text" id="todo-input" maxlength="140" ...>
```

## Step 6 — Sanity check the 1.12 features still work

The project is cumulative — make sure nothing regressed:

```bash
curl -s -o /tmp/img.jpg -w "%{http_code}\n" http://localhost:8081/image
# → 200 (image served, cached on the PV from 1.12)

curl -s http://localhost:8081/api/health
# → {"status":"ok"}

curl -s http://localhost:8081/shutdown   # optional
# pod restarts, image + todos are still served (todos are hardcoded in
# the binary, the image is on the PV)
```

## Step 7 — Clean up

```bash
kubectl delete -f manifests/
kubectl delete -f manifests/pv/persistentvolume.yaml
kubectl get pv,pvc
# → No resources found
```

## P/S:

1. **Progressive enhancement**: 1.13 only touches the frontend of the
   project — the image, health and shutdown endpoints from earlier
   exercises keep working untouched.
2. **Client-side constraints**: `maxlength` is the right tool for "the
   input should not take todos that are over 140 characters long" —
   enforced by the browser before any JS runs.
3. **Placeholder APIs**: `/api/todos` returning hardcoded JSON is a
   contract for the future — the next exercise will wire the form to
   it (and a database).
4. **Hardcoded data is fine when the exercise says so**: "It does not
   have to send the todo yet" — don't over-engineer; the course will
   ask for persistence when it wants it.
