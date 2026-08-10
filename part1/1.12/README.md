# Exercise 1.12

## Goal

> Since the project looks a bit boring right now, let's add a picture!
>
> The goal is to add an hourly image to the project.
>
> Get a random picture from Lorem Picsum like https://picsum.photos/1200
> and display it in the project. Find a way to store the image so it
> stays the same for 10 minutes.
>
> - After 10 minutes have passed, you might give the old pic still one
>   more time, and for the next request, there should be a new picture
>
> Make sure to cache the image into a persistent volume so that the API
> isn't needed for new images every time we access the application or
> the container crashes.
>
> The best way to test what happens when your container shuts down is
> likely by shutting down the container, so you can add logic for that
> as well, for testing purposes.

In short: the todo-app project (1.8) gains a **random picture that
changes every 10 minutes** — fetched once from Lorem Picsum, then
served from a cache file on a **PersistentVolume** so the API isn't
called on every request and the image survives container crashes. A
`/shutdown` endpoint lets you kill the container on purpose to test
that.

## What you should have before starting

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

1. `GET /` renders the todo-app HTML page with an
   `<img src="/image">` tag.
2. `GET /image`:
   - Cache hit (file exists, younger than 10 minutes) → serve the
     file, **no network call**.
   - No file / expired → download `https://picsum.photos/1200`,
     write it to the PersistentVolume (`/usr/src/app/files/image.jpg`),
     serve it. The next 10 minutes of requests are served from disk.
3. `GET /shutdown` → `std::process::exit(0)` — kills the container so
   you can verify the image is still there when the pod comes back.
4. Image file lives on a PV (mounted at `/usr/src/app/files`), so
   container/pod restarts don't lose it — and the Lorem Picsum API is
   only called once every 10 minutes.

## Source code (`src/main.rs`)

To verify locally before writing Dockerfile/manifests:

```bash
cargo build
mkdir -p /tmp/1.12-share
PORT=3001 IMAGE_PATH=/tmp/1.12-share/image.jpg ./target/debug/todo-app

# another terminal:
curl -s http://localhost:3001/image -o /tmp/img1.jpg   # fetches from picsum
file /tmp/img1.jpg                                     # → JPEG 1200x1200
curl -s http://localhost:3001/image -o /tmp/img2.jpg   # served from cache
cmp /tmp/img1.jpg /tmp/img2.jpg                        # → identical, no API call
curl -s http://localhost:3001/shutdown                 # process exits
ls -la /tmp/1.12-share/image.jpg                       # file survives
```

**To stop the server:** foreground → `Ctrl+C`; background →
`pkill -f todo-app` or `fuser -k 3001/tcp`. (Or just
`curl localhost:3001/shutdown` — that's the exercise's whole point!)

## Step 1 — Prepare the node directory for the local PV

```bash
docker exec k3d-mycluster-agent-0 mkdir -p /tmp/kube
docker exec k3d-mycluster-agent-0 ls -ld /tmp/kube
# → drwxr-xr-x 1 root root 4096 ... /tmp/kube
```

## Step 2 — Apply the PersistentVolume (admin-owned, separate folder)

```bash
kubectl apply -f manifests/pv/persistentvolume.yaml
kubectl get pv
# → example-pv  Available (or Bound if 1.11's PVC is still around — delete old PVC first)
```

## Step 3 — Apply the PersistentVolumeClaim

```bash
kubectl apply -f manifests/persistentvolumeclaim.yaml
kubectl get pvc
# → NAME         STATUS  VOLUME      CAPACITY  ACCESS MODES  STORAGECLASS
# → image-claim  Bound   example-pv  1Gi       RWO           my-example-pv
```

> if the PVC from 1.11 still exists with the same
> name, `kubectl apply` reports `unchanged` and it stays Bound to the
> same PV — that's fine, the volume is reusable.

## Step 4 — Build and push the image

```bash
docker build -t tripplen63/todo-app:1.12 .
docker push tripplen63/todo-app:1.12
```

## Step 5 — Apply the app manifests

Apply and wait:

```bash
kubectl apply -f manifests/
kubectl get pods
# → todo-app-xxxxxxxxxx-xxx   1/1  Running
```

## Step 6 — Verify the hourly image

First request → downloads from Lorem Picsum and caches it:

```bash
curl -s -o /tmp/k8s-img1.jpg -w "%{http_code} %{size_download}B\n" http://localhost:8081/image
# → 200 1xxxxxB

kubectl exec deployment/todo-app -- ls -la /usr/src/app/files/
# → -rw-r--r-- 1 root root 1xxxxx ... image.jpg

file /tmp/k8s-img1.jpg
# → JPEG image data ... 1200x1200
```

Open the browser: `http://localhost:8081/` → the picture on the page.
Refresh → same picture (cached).

Second request right after → served from cache, no API call:

```bash
curl -s -o /tmp/k8s-img2.jpg -w "%{http_code} %{size_download}B\n" http://localhost:8081/image
cmp /tmp/k8s-img1.jpg /tmp/k8s-img2.jpg && echo "SAME image (cache works)"
```

## Step 7 — Test container shutdown (the exercise's point)

Kill the container on purpose:

```bash
curl -s http://localhost:8081/shutdown
# → Shutting down on request (exercise 1.12 test)
```

The pod restarts (CrashLoop? no — the deployment recreates it):

```bash
kubectl get pods
# → todo-app-xxxxxxxxxx-xxx   1/1  Running   1 (1 restart)
```

Now the image must still be served from the PV — WITHOUT calling the
API (it's younger than 10 minutes):

```bash
curl -s -o /tmp/k8s-img3.jpg -w "%{http_code} %{size_download}B\n" http://localhost:8081/image
cmp /tmp/k8s-img1.jpg /tmp/k8s-img3.jpg && echo "SAME image after container restart (PV works)"
```

> **Compare with 1.10**: emptyDir would have lost the image on pod
> restart. The PV keeps it.

## Step 8 — (Optional) Watch the 10-minute rotation

The exercise allows the old picture one more time after 10 minutes,
then the next request fetches a new one. Our implementation fetches a
new picture as soon as the cached file is older than 10 minutes.

To see the rotation quickly without waiting, restart the pod so the
file's mtime is fresh, then either wait 10 minutes or temporarily
lower `MAX_AGE_SECS` in the source (not recommended for submission —
keep 600):

```bash
kubectl delete pod -l app=todo-app   # pod restarts, mtime resets
```

Then just leave the browser open — every 10 minutes the picture
changes on the next request.

## Step 9 — Clean up

```bash
kubectl delete -f manifests/
kubectl delete -f manifests/pv/persistentvolume.yaml
kubectl get pv,pvc
# → No resources found
```

## P/S:

1. **Caching on a PV**: fetch once, serve from disk — the API is
   called at most once per 10 minutes, and a container crash doesn't
   lose the picture.
2. **mtime as TTL**: `metadata().modified().elapsed()` is a dead-simple
   "is this file fresh?" check — no timers, no state, survives restarts.
3. **Local PVs** (1.11 recap): admin-owned, node-bound, survives pod
   deletion — exactly what the exercise means by "cache into a
   persistent volume".
4. **Deliberate shutdown for testing**: a `/shutdown` endpoint (or
   `kubectl delete pod`) is the right way to prove persistence —
   restart the container and verify the data is still served.
