# Exercise 2.6 — The project, step 10 (no hardcoded config)

## Goal

> Make sure that your project has **no hard coded ports, URLs, or other
> configurations in the source code**. Pass all the configurations to
> pods as env variables that are defined either in a **config map** or
> in **deployments**.

## What was hardcoded (2.4) → what it becomes (2.6)

| Value              | 2.4 (hardcoded in source)              | 2.6 (env var)          |
| ------------------ | -------------------------------------- | ---------------------- |
| `PORT`             | `"3000"` fallback in `main()`          | `PORT` env             |
| `IMAGE_PATH`       | `const "/usr/src/app/files/image.jpg"` | `IMAGE_PATH` env       |
| `IMAGE_URL`        | `const "https://picsum.photos/1200"`   | `IMAGE_URL` env        |
| `MAX_AGE_SECS`     | `const 600`                            | `MAX_AGE_SECS` env     |
| `TODO_BACKEND_URL` | `const "http://todo-backend-svc:2345"` | `TODO_BACKEND_URL` env |

## Source code changes (todo-app)

Replace the four `const` declarations with a single helper that reads a
required env var and **fails loudly** if it is missing (no fallback):

```rust
/// Read a required env var, failing loudly if it is missing. No
/// hardcoded defaults — every value is injected by the Deployment or a
/// ConfigMap (exercise 2.6).
fn env_or(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("environment variable {name} is not set"))
}
```

Then use it everywhere a config value is needed:

```rust
// in index() and create_todo()
let backend_url = env_or("TODO_BACKEND_URL");

// in image()
let path = env_or("IMAGE_PATH");
let image_url = env_or("IMAGE_URL");
let max_age: u64 = env_or("MAX_AGE_SECS").parse().expect("MAX_AGE_SECS must be a valid number");

// in main()
let port: u16 = env_or("PORT").parse().expect("PORT must be a valid number");
```

todo-backend only has one config value (PORT), so it just loses the
default:

```rust
let port: u16 = env::var("PORT")
    .expect("environment variable PORT is not set")
    .parse()
    .expect("PORT must be a valid number");
```

> **Key idea**: `panic!` on a missing env var is _good_ here — a pod
> started without its configuration should crash loudly and show
> `CrashLoopBackOff`, not silently run with a wrong value.

## Step 1 — Build + push Dockerfiles

```bash
cd todo-app
docker build -t tripplen63/todo-app:2.6 .
docker push tripplen63/todo-app:2.6

cd ../todo-backend
docker build -t tripplen63/todo-backend:2.6 .
docker push tripplen63/todo-backend:2.6
```

## Step 2 — Apply and verify

```bash
docker exec k3d-mycluster-agent-0 mkdir -p /tmp/kube

kubectl apply -f manifests/pv/persistentvolume.yaml
kubectl apply -f manifests/
kubectl get all,configmap -n project
# pod/todo-app-xxx       1/1  Running
# pod/todo-backend-xxx   1/1  Running
# configmap/todo-config   4       ...

# verify the config actually reached the pod as env vars
kubectl exec deployment/todo-app -n project -- env | grep -E 'TODO_BACKEND_URL|IMAGE_URL|MAX_AGE_SECS|PORT'
# TODO_BACKEND_URL=http://todo-backend-svc:2345
# IMAGE_URL=https://picsum.photos/1200
# MAX_AGE_SECS=600
# PORT=3000

# full flow still works
curl -s -X POST http://localhost:8081/todos -d 'content=Learn Kubernetes' -w "%{http_code}\n"
# 303
curl -s http://localhost:8081/ | grep '<span>'
# <span>Learn Kubernetes</span>
```

## Step 3 — Prove the value is config-driven (concept check)

Change `MAX_AGE_SECS` in the ConfigMap — the image cache TTL — and see
it take effect **without rebuilding the image**:

```bash
kubectl patch configmap todo-config -n project -p '{"data":{"MAX_AGE_SECS":"30"}}'
kubectl rollout restart deployment/todo-app -n project   # env vars need a restart
sleep 15
kubectl exec deployment/todo-app -n project -- env | grep MAX_AGE_SECS
# MAX_AGE_SECS=30
```

> The same image (`todo-app:2.6`) now behaves differently purely from a
> ConfigMap edit + restart — that's the whole point of 2.6: no config in
> code. Revert to `600` afterwards (`kubectl edit configmap ...` + restart).

## Step 4 — Clean up

```bash
kubectl delete -f manifests/
kubectl delete -f manifests/pv/persistentvolume.yaml
kubectl get all,configmap -n project
# No resources found in project namespace
```

## P/S:

1. **Configuration belongs outside code**: ports, URLs, paths and TTLs
   are injected as env vars, so the same image can run differently per
   environment (dev vs staging vs prod).
2. **Two injection places**: a ConfigMap (`envFrom.configMapRef`) for
   shared config, a Deployment `env` for pod-specific values.
3. **Fail fast**: reading a missing env var should panic, not silently
   default — a misconfigured pod should crash visibly.
4. **Config change without rebuild**: editing a ConfigMap + rolling
   restart reconfigures the app with zero code/image changes.
