# Exercise 3.4 — Rewritten routing (GKE Gateway API)

## Goal

> Your ping-pong app now most likely needs to respond to the URL
> **`/pingpong`** to work in the cluster setup. It would be nice if we were
> **not forced to reflect the cluster-level URL structures in the
> applications**, and instead the app itself could provide the behavior in
> the **root path `/`**. Thanks to the flexibility of the Gateway API, this
> can be easily done by **route rewriting**.
>
> "Make this change to your ping-pong app and to the HTTP route!"
>
> — see [HTTP redirect and rewrite](https://gateway-api.sigs.k8s.io/guides/http-redirect-rewrite/).

---

## Knowledge: route rewriting in the Gateway API

In 3.3 the ping-pong **app** had to expose `/pingpong` because the cluster
routed `/pingpong` to it. That leaks the cluster's URL layout into the app
code — annoying if the layout changes.

The Gateway API lets the **route** do the rewriting instead:

- The **app** keeps its natural endpoints (`/` for pongs, `/pongs` for the
  count).
- The **HTTPRoute** matches the public path `/pingpong` and applies a
  **URLRewrite filter** that rewrites the path to `/` **before** forwarding
  to ping-pong-svc.

### The URLRewrite filter

```yaml
filters:
  - type: URLRewrite
    urlRewrite:
      path:
        type: ReplacePrefix
        replacePrefixMatch: /
```

- `ReplacePrefixMatch: /` means: replace the matched prefix (`/pingpong`)
  with `/` → `/pingpong` becomes `/`, `/pingpong/anything` → `/anything`.
- The client still sees `/pingpong`; the app never knows.

### Resulting flow

```
Internet ── GKE Gateway (L7 LB) ── HTTPRoute
              ├─ /pingpong  ──URLRewrite──▶ / ──▶ ping-pong-svc ──▶ ping-pong app at /
              └─ /          ────────────────▶ log-output-svc ──▶ log-output app at /
```

Everything else (GatewayClass, Gateway, ClusterIP services, health checks)
is the same as 3.3.

## Step 1 — create the GKE cluster + enable the Gateway API

### 1a. Create the cluster (private nodes — org-policy safe)

> Org policy `compute.vmExternalIpAccess` denies external IPs on VMs →
> nodes must be private (no public IP).

```bash
gcloud container clusters create dwk-cluster \
  --zone=europe-north1-b --cluster-version=1.36 \
  --disk-size=32 --num-nodes=4 --machine-type=e2-small \
  --enable-ip-alias --enable-private-nodes --master-ipv4-cidr=172.16.0.0/28 \
  --project=dwk-gke-506208
```

Wait for `STATUS: RUNNING` (~4-5 min). Fixes (persist at project level, but
run them if re-creating the cluster):

```bash
gcloud container clusters update dwk-cluster --zone=europe-north1-b \
  --project=dwk-gke-506208 --no-enable-master-authorized-networks

gcloud projects add-iam-policy-binding dwk-gke-506208 \
  --member="serviceAccount:323959491379-compute@developer.gserviceaccount.com" \
  --role="roles/artifactregistry.reader"
```

### 1b. Enable the Gateway API (once per cluster, ~5 min)

```bash
gcloud container clusters update dwk-cluster \
  --location=europe-north1-b --project=dwk-gke-506208 --gateway-api=standard
```

After it finishes:

```bash
kubectl get gatewayclass     # gke-l7-global-external-managed etc.
kubectl get nodes            # 4 nodes Ready
```

> ⚠️ If `update ... --gateway-api=standard` says `NOT_FOUND: no cluster
> named 'dwk-cluster'`, you skipped 1a — create the cluster first 😄

## Step 2 — build & push Dockerfiles

```bash
gcloud auth configure-docker   # once
cd ping-pong && docker build -t gcr.io/dwk-gke-506208/ping-pong:3.4 . && docker push gcr.io/dwk-gke-506208/ping-pong:3.4
cd ../log-output && docker build -t gcr.io/dwk-gke-506208/log-output:3.4 . && docker push gcr.io/dwk-gke-506208/log-output:3.4
```

## Step 3 — deploy + verify manifests

```bash
kubectl apply -f manifests/deployment-ping-pong.yaml
kubectl apply -f manifests/deployment-log-output.yaml
kubectl apply -f manifests/service.yaml
kubectl apply -f manifests/gateway.yaml
kubectl apply -f manifests/route.yaml

kubectl rollout status deploy/ping-pong
kubectl rollout status deploy/log-output
kubectl get pods        # both Running
kubectl get svc         # both ClusterIP
```

Watch the Gateway get its external IP (~5 min):

```bash
kubectl get gateway my-gateway
# NAME         CLASS                      ADDRESS        PROGRAMMED   AGE
# my-gateway   gke-l7-global-external-... 34.128.x.x     True         5m
kubectl get httproute my-route        # Accepted, no RefNotPermitted
kubectl describe httproute my-route   # filters: URLRewrite applied
```

### Verify the rewrite

| Path you open | What happens | Expected response |
|---|---|---|
| `http://<GATEWAY-IP>/pingpong` | gateway **rewrites** → ping-pong app at `/` | `pong 0`, `pong 1`, … |
| `http://<GATEWAY-IP>/` | direct → log-output | log lines + `Ping / Pongs: N` |

```bash
curl http://<GATEWAY-IP>/pingpong   # pong 0  ← came from app's "/" route (rewritten)
curl http://<GATEWAY-IP>/pingpong   # pong 1
curl http://<GATEWAY-IP>/           # log output + Ping / Pongs: 2
```

---

## Step 4 — clean up (IMPORTANT — saves credits!)

```bash
gcloud container clusters delete dwk-cluster --zone=europe-north1-b \
  --project=dwk-gke-506208

gcloud container images delete gcr.io/dwk-gke-506208/ping-pong:3.4 --quiet --force-delete-tags
gcloud container images delete gcr.io/dwk-gke-506208/log-output:3.4 --quiet --force-delete-tags
```

Verify:

```bash
gcloud container clusters list
gcloud compute instances list
gcloud compute forwarding-rules list
gcloud compute addresses list
```

## P/S:

1. **Route rewriting decouples URL structure from app code** — the app
   serves natural paths; the gateway maps public paths onto them.
2. `URLRewrite` + `ReplacePrefixMatch: /` turns `/pingpong` → `/`; GKE
   implements this filter on the L7 LB (client URL stays unchanged).
3. The app change: ping-pong's pong logic **moved to `/`** — no more
   `/pingpong` in code (see `src/main.rs`).
4. Rule order: specific (`/pingpong`) before catch-all (`/`).
5. Health checks still probe `/` — they'll pop the pong counter; harmless.
6. **Delete the cluster when idle** — GKE bills per node/hour + the LB.