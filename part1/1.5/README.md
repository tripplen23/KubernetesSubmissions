# Exercise 1.5

## Step 1 — Build the Docker image

```bash
cd ~/binh/KubernetesSubmissions/part1/1.5
docker build -t tripplen63/todo-app:1.5 .
```

**Check the image:**
```bash
docker images tripplen63/todo-app
```

## Step 2 — Test the container locally (no cluster)

```bash
docker run --rm -p 3001:3000 tripplen63/todo-app:1.5
```

**Expected (foreground, blocks):**
```
Server started in port 3000
```

In another terminal, check the 3 endpoints:
```bash
curl -s http://localhost:3001/ | head -3
# → <!doctype html>
# → <html lang="en">
# →   <head>

curl -s http://localhost:3001/api/health
# → {"status":"ok"}

curl -s http://localhost:3001/api/todos
# → []
```

Press **Ctrl+C** in the first terminal to stop the container.

## Step 3 — Create the Docker Hub repository (If don't have one)

Open <https://hub.docker.com/repository-create> in your browser:

1. **Name**: `todo-app` (must match the image name; lowercase, hyphens only)
2. **Visibility**: `Public` (so K8s can pull without `imagePullSecrets`)
3. **Description**: optional, e.g. "DevOps with Kubernetes — todo-app"
4. Click **Create**

**Expected**: a page like
`https://hub.docker.com/repository/docker/tripplen63/todo-app` showing
"This repository is empty. Use the Docker CLI to push your images."

> If you see "Repository not found" when you push, the repo name in your
> manifest (`spec.containers[0].image`) must match exactly. The current
> manifest uses `tripplen63/todo-app:1.5`.

## Step 4 — Push the image to Docker Hub

```bash
docker push tripplen63/todo-app:1.5
```

**Expected (last line):**
```
1.5: digest: sha256:abc123... size: 1234
```

**If you see `insufficient_scope: authorization failed`:**
You are not logged in. Run `docker login -u tripplen63` again (see
"What you should have before starting").

## Step 5 — Apply manifests to the cluster

```bash
kubectl apply -f manifests/deployment.yaml
kubectl apply -f manifests/service.yaml
```

**Expected:**
```
deployment.apps/todo-app created
service/todo-app created
```

(The word `configured` instead of `created` means the Deployment already
exists; K8s reconciles to the new state.)

Watch the pod come up:
```bash
kubectl get pods -l app=todo-app
```

**Expected:**
```
NAME                       READY   STATUS    RESTARTS   AGE
todo-app-xxxxxxxxxx-xxxxx  1/1     Running   0          8s
```

Wait for `1/1 Running`. If it isn't, run
`kubectl describe pod -l app=todo-app` and read the **Events** section
at the bottom: it tells what went wrong.

## Step 6 — Verify with `kubectl port-forward`

```bash
kubectl port-forward deployment/todo-app 8088:3000
```

**Expected:**
```
Forwarding from 127.0.0.1:8088 -> 3000
Forwarding from [::1]:8088 -> 3000
```

Open **<http://localhost:8088/>** in your browser: the "Todo App" HTML
landing page with three endpoint bullets.

## Step 7 — Clean up

```bash
# Stop the port-forward: Ctrl+C in its terminal, or
# find and kill the background process

kubectl delete -f manifests/deployment.yaml
kubectl delete -f manifests/service.yaml
# confirm everything is gone:
kubectl get all -l app=todo-app
# → No resources found in default namespace.
```

> **Why clean up?** K8s keeps pods running when your terminal closes,
> so forgotten resources pile up. After every exercise: `kubectl delete -f`
> or `kubectl delete deploy,svc,pod -l app=<name>`.
