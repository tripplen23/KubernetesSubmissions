# Exercise 1.6

## What you should have before starting

- A running k3d cluster named `mycluster` with these port mappings:
  - `8081:80@loadbalancer`
  - `8082:30080@agent:0`
- Verify:
  ```bash
  k3d cluster list
  # → NAME  SERVERS  AGENTS  LOADBALANCER
  # → mycluster  1/1  1/1  true
  ss -tlnp 2>/dev/null | grep -E ':(8081|8082)'
  # → LISTEN 0  4096  *:8081  *:*
  # → LISTEN 0  4096  *:8082  *:*
  ```
- Docker Hub login: `docker login -u tripplen63`.
- Working directory: `~/binh/KubernetesSubmissions/part1/1.6`.

## Step 1 — Build the Docker image

```bash
cd part1/1.6
docker build -t tripplen63/todo-app:1.6 .
docker images tripplen63/todo-app:1.6
```

## Step 2 — Test the container locally (no cluster)

```bash
docker run --rm -p 3001:3000 tripplen63/todo-app:1.6
```

In another terminal:
```bash
curl -s http://localhost:3001/ | head -3
# → <!doctype html>
# → <html lang="en">
curl -s http://localhost:3001/api/health
# → {"status":"ok"}
```

Press **Ctrl+C** in the first terminal to stop the container. The
`--rm` flag tells Docker to delete the container as soon as it stops.

> **If you ran this in the background** (e.g. via `&` or a tool that
> doesn't share your terminal), `Ctrl+C` won't reach the container —
> it'll keep running on port 3001. To kill it from any other terminal:
> ```bash
> docker ps --format '{{.Names}}' | xargs -I{} docker stop {}
> # or, if you know the name (Docker picks a random one like
> # "mystifying_hellman"):
> docker stop mystifying_hellman
> ```
> Check the row "If you see `address already in use`" in the Common
> errors table below if a previous container is still bound to 3001.

## Step 3 — Push the image

The `todo-app` repo already exists on Docker Hub from 1.5. Just push:
```bash
docker push tripplen63/todo-app:1.6
```

**Expected last line:**
```
1.6: digest: sha256:... size: ...
```

## Step 4 — Apply manifests

```bash
kubectl apply -f manifests/deployment.yaml
kubectl apply -f manifests/service.yaml
kubectl get pods -l app=todo-app
```

**Expected (last command):**
```
NAME                      READY   STATUS    RESTARTS   AGE
todo-app-xxxxxxxxxx-xxx   1/1     Running   0          8s
```

Verify the Service got a NodePort:
```bash
kubectl get svc todo-app
```

**Expected:**
```
NAME       TYPE        CLUSTER-IP    EXTERNAL-IP   PORT(S)         AGE
todo-app   NodePort    10.43.x.x     <none>        3000:30080/TCP  5s
```

> **K8s concept** — the format `3000:30080/TCP` means:
> - `3000` = the Service's cluster-internal port (other pods call this)
> - `30080` = the NodePort (opened on every node)
> - `/TCP` = protocol
>
> The Service has no `EXTERNAL-IP` because it's a NodePort, not a
> LoadBalancer. NodePort doesn't get a public IP — the nodes themselves
> are the entrypoint.

## Step 5 — Access the app through the NodePort

```bash
curl -s http://localhost:8082/ | head -3
# → <!doctype html>
# → <html lang="en">

curl -s http://localhost:8082/api/health
# → {"status":"ok"}
```

Open your browser at **<http://localhost:8082/>**.

**How the request gets there:**
```
curl → host:8082 → k3d proxy (port mapping 8082:30080@agent:0)
     → agent node port 30080 → kube-proxy
     → Service todo-app:3000 → pod IP:3000
```

> **K8s concept** — a NodePort is opened on **every node** in the
> cluster, not just the agent. The k3d port mapping makes **one**
> specific agent's NodePort reachable from the host (localhost:8082).
> Try this to see the others:
> ```bash
> kubectl get nodes -o wide
> # each node has InternalIP + ExternalIP; the NodePort is on all of them
> ```

## Step 6 — Compare with `kubectl port-forward` (1.5)

You can still port-forward to the same pod:
```bash
kubectl port-forward deployment/todo-app 8088:3000
```
(In another terminal, hit `http://localhost:8088/`.)

This **also works** at the same time as the NodePort. They're independent.

> **K8s concept** — port-forward and NodePort both reach the pod, but:
> - `port-forward` = tunnel through kube-apiserver (1 user, 1 pod, debug)
> - NodePort = a port opened on every node by kubelet, forwarded by
>   kube-proxy to the matching Service endpoints (any user, any pod,
>   "production-like" but limited)
>
> Why do we have two ways? Because they have different **scopes** —
> port-forward is for developers, NodePort is for routing traffic into
> the cluster from a controlled environment (like your laptop's k3d
> proxy or a bare-metal cluster). The course uses NodePort here to
> show the real cluster routing — but in production you'd swap it for
> LoadBalancer or Ingress.

## Step 7 — Scale the Deployment and watch the routing

```bash
kubectl scale deployment/todo-app --replicas=3
kubectl get pods -l app=todo-app
```

Hit `http://localhost:8082/api/health` 5 times in a row. All return
`{"status":"ok"}` — the Service load-balances between the 3 replicas.

To see which pod handled each request:
```bash
kubectl logs -l app=todo-app --prefix -f
```

In another terminal, hit the URL again — you'll see lines from
different pods, one per request.

## Step 8 — Clean up

```bash
kubectl delete -f manifests/service.yaml
kubectl delete -f manifests/deployment.yaml
kubectl get all -l app=todo-app
# → No resources found in default namespace. But it might take some times
```

## What you should have learned

1. A **Service** is a stable name → pod-group mapping. The selector
   (`app: todo-app`) tells the Service which pods to route to. Without
   it, the Service is just a YAML file.
2. **NodePort** opens a port on every node (30000-32767). Great for
   dev/test, not for production (port conflicts, no TLS, no hostname
   routing). Production uses LoadBalancer (cloud) or Ingress.
3. `kubectl port-forward` and NodePort both work at the same time
   because they take different code paths through the cluster.
