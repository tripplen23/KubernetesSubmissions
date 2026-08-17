# Exercise 2.5 — Documentation and ConfigMaps

## Goal

> Use the official Kubernetes documentation for this exercise. The
> following
> [ConfigMap concept](https://kubernetes.io/docs/concepts/configuration/configmap/)
> and
> [Configure a Pod to Use a ConfigMap](https://kubernetes.io/docs/tasks/configure-pod-container/configure-pod-configmap/)
> should contain everything you need.
>
> Create a ConfigMap for the **"Log output"** application. The ConfigMap
> should define one file `information.txt` and one env variable `MESSAGE`.
>
> The app should map the file as a volume, set the environment variable
> and print the content of those in addition to the usual output:
>
> ```
> file content: this text is from file
> env variable: MESSAGE=hello world
> 2026-05-18T12:15:17.705Z: 8523ecb1-c716-4cb6-a044-b9e83bb98e43.
> Ping / Pongs: 3
> ```

This exercise introduces **ConfigMaps** — one of Kubernetes' two
configuration resources (the other is Secrets). Unlike 2.3/2.4, the
**code changes**: the log-output reader must now also print the contents
of a file (`information.txt`) mounted from a ConfigMap volume, and an
env variable (`MESSAGE`) injected from the same ConfigMap.

## Concepts covered

Read these two pages — they answer every "how" below:

- <https://kubernetes.io/docs/concepts/configuration/configmap/>
- <https://kubernetes.io/docs/tasks/configure-pod-container/configure-pod-configmap/>

| Question                                       | Answer (find it in the docs)                                                               |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------ |
| What's a ConfigMap vs a Secret?                | ConfigMap = plain config (any key/values, files); Secret = sensitive data (base64-encoded) |
| How do you inject a single key as an env var?  | `valueFrom.configMapKeyRef` (name + key)                                                   |
| How do you inject ALL keys as env vars?        | `envFrom.configMapRef`                                                                     |
| How do you mount ConfigMap keys as files?      | `volumes[].configMap` + `volumeMounts`                                                     |
| How do you mount only ONE key as one file?     | `volumes[].configMap.items[]` (key → path)                                                 |
| Does editing a ConfigMap update env vars?      | **No** — env vars need a pod restart                                                       |
| Does editing a ConfigMap update mounted files? | **Yes** — the volume is updated (eventually consistent)                                    |

## Step 1: build + push log-output

**Only `log-output` changes** (the reader prints the two new lines).
**`ping-pong` is unchanged** — reuse image `tripplen63/ping-pong:2.1`
which already pushed in exercise 2.1.

```bash
cd log-output
docker build -t tripplen63/log-output:2.5 .
docker push tripplen63/log-output:2.5
```

## Step 2 — Apply and verify

```bash
kubectl apply -f manifests/

kubectl get configmap -n exercises
# NAME                DATA   AGE
# log-output-config   2      5s

kubectl describe configmap log-output-config -n exercises
# Data
# ====
# MESSAGE:
# ----
# hello world
# information.txt:
# ----
# this text is from file

# bump the pong counter a few times, then read the log
curl -s http://localhost:8081/pingpong   # pong 0
curl -s http://localhost:8081/pingpong   # pong 1
curl -s http://localhost:8081/pingpong   # pong 2

sleep 6
curl -s http://localhost:8081/log
# file content: this text is from file
# env variable: MESSAGE=hello world
# 2026-05-18T12:15:17.705Z: 8523ecb1-c716-4cb6-a044-b9e83bb98e43.
# Ping / Pongs: 3
```

## Step 3 — Prove ConfigMap behaviour (concept check)

Two experiments to internalise the docs' two key facts:

**A. Editing a ConfigMap does NOT update env vars** (needs restart):

```bash
kubectl edit configmap log-output-config -n exercises   # change MESSAGE to "changed"
kubectl get pods -n exercises
# ... the reader pod is NOT restarted automatically

curl -s http://localhost:8081/log | grep "env variable"
# env variable: MESSAGE=hello world   ← STILL the old value

# Force a rollout to pick it up:
kubectl rollout restart deployment/log-output -n exercises
sleep 20
curl -s http://localhost:8081/log | grep "env variable"
# env variable: MESSAGE=changed       ← now updated
```

> Revert MESSAGE back to `hello world` afterwards with
> `kubectl edit configmap ...` + another `rollout restart` (or re-apply
> `configmap.yaml`).

**B. Editing a ConfigMap DOES update mounted files** (eventually):

```bash
kubectl edit configmap log-output-config -n exercises
# change information.txt content to "this text is updated"

# wait ~30-60s (the kubelet syncs configmap volumes on a timer)
curl -s http://localhost:8081/log | grep "file content"
# file content: this text is updated   ← file volume picked it up (no restart)
```

> Env vars are read once at container start; files are watched and
> synced into the volume. That asymmetry is the core ConfigMap lesson.

## Step 4 — Clean up

```bash
kubectl delete -f manifests/
kubectl get all,configmap -n exercises
# No resources found in exercises namespace
```

## P/S:

1. **ConfigMap = plain configuration** (files + env vars) scoped to a
   namespace; Secrets are for sensitive data (base64).
2. **Two injection paths**: env vars (`configMapKeyRef` / `envFrom`) and
   files (a `configMap` volume mounted into the pod).
3. **The env-vs-file asymmetry**: editing a ConfigMap updates mounted
   files automatically, but env vars only change on pod restart.
4. **ConfigMap + namespace**: the ConfigMap must live in the same
   namespace as the pod that consumes it.
