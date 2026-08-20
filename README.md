# KubernetesSubmissions

Submissions for the University of Helsinki's **DevOps with Kubernetes** MOOC
(2026 edition).

## Exercises

### Part 1

| Folder | Exercise | App | Image | Submit link |
|---|---|---|---|---|
| [`part1/1.1/`](./part1/1.1) | 1.1 — Getting started | `log_output` (Rust + tokio) | `tripplen63/log-output:1.1` | [tag `1.1`](https://github.com/tripplen23/KubernetesSubmissions/tree/1.1/part1/1.1) |
| [`part1/1.2/`](./part1/1.2) | 1.2 — Project v0.1 | `todo-app` (Rust + axum) | `tripplen63/todo-app:1.2` | [tag `1.2`](https://github.com/tripplen23/KubernetesSubmissions/tree/1.2/part1/1.2) |
| [`part1/1.3/`](./part1/1.3) | 1.3 — Declarative approach | `log_output` (manifests) | `tripplen63/log-output:1.3` | [tag `1.3`](https://github.com/tripplen23/KubernetesSubmissions/tree/1.3/part1/1.3) |
| [`part1/1.4/`](./part1/1.4) | 1.4 — Project v0.2 | `todo-app` (manifests) | `tripplen63/todo-app:1.4` | [tag `1.4`](https://github.com/tripplen23/KubernetesSubmissions/tree/1.4/part1/1.4) |
| [`part1/1.5/`](./part1/1.5) | 1.5 — Project, step 3 (HTML) | `todo-app` (HTML landing page) | `tripplen63/todo-app:1.5` | [tag `1.5`](https://github.com/tripplen23/KubernetesSubmissions/tree/1.5/part1/1.5) |
| [`part1/1.6/`](./part1/1.6) | 1.6 — Project, step 4 (NodePort) | `todo-app` (NodePort Service) | `tripplen63/todo-app:1.6` | [tag `1.6`](https://github.com/tripplen23/KubernetesSubmissions/tree/1.6/part1/1.6) |
| [`part1/1.7/`](./part1/1.7) | 1.7 — Project, step 5 (Ingress) | `log-output` (HTTP `/status`) | `tripplen63/log-output:1.7` | [tag `1.7`](https://github.com/tripplen23/KubernetesSubmissions/tree/1.7/part1/1.7) |
| [`part1/1.8/`](./part1/1.8) | 1.8 — Ingress instead of NodePort | `todo-app` (Ingress, ClusterIP) | `tripplen63/todo-app:1.8` | [tag `1.8`](https://github.com/tripplen23/KubernetesSubmissions/tree/1.8/part1/1.8) |
| [`part1/1.9/`](./part1/1.9) | 1.9 — More services | `ping-pong` (counter) + log-output | `tripplen63/ping-pong:1.9` | [tag `1.9`](https://github.com/tripplen23/KubernetesSubmissions/tree/1.9/part1/1.9) |
| [`part1/1.10/`](./part1/1.10) | 1.10 — Even more services (emptyDir) | `log-output` (writer + reader) | `tripplen63/log-output:1.10` | [tag `1.10`](https://github.com/tripplen23/KubernetesSubmissions/tree/1.10/part1/1.10) |
| [`part1/1.11/`](./part1/1.11) | 1.11 — Persisting data (PV/PVC) | `ping-pong` + `log-output` (shared PV) | `tripplen63/ping-pong:1.11`, `tripplen63/log-output:1.11` | [tag `1.11`](https://github.com/tripplen23/KubernetesSubmissions/tree/1.11/part1/1.11) |
| [`part1/1.12/`](./part1/1.12) | 1.12 — The project, step 6 (hourly image) | `todo-app` (hourly picture from Lorem Picsum, cached on PV) | `tripplen63/todo-app:1.12` | [tag `1.12`](https://github.com/tripplen23/KubernetesSubmissions/tree/1.12/part1/1.12) |
| [`part1/1.13/`](./part1/1.13) | 1.13 — The project, step 7 (todo UI) | `todo-app` (input ≤140 chars + Send button + hardcoded todos) | `tripplen63/todo-app:1.13` | [tag `1.13`](https://github.com/tripplen23/KubernetesSubmissions/tree/1.13/part1/1.13) |

### Part 2

| Folder | Exercise | App | Image | Submit link |
|---|---|---|---|---|
| [`part2/2.1/`](./part2/2.1) | 2.1 — Connecting pods | `ping-pong` + `log-output` (HTTP between pods, no shared volume) | `tripplen63/ping-pong:2.1`, `tripplen63/log-output:2.1` | [tag `2.1`](https://github.com/tripplen23/KubernetesSubmissions/tree/2.1/part2/2.1) |
| [`part2/2.2/`](./part2/2.2) | 2.2 — The project, step 8 (todo-backend) | `todo-app` (UI) + `todo-backend` (GET/POST /todos, in-memory) | `tripplen63/todo-app:2.2`, `tripplen63/todo-backend:2.2` | [tag `2.2`](https://github.com/tripplen23/KubernetesSubmissions/tree/2.2/part2/2.2) |
| [`part2/2.3/`](./part2/2.3) | 2.3 — Keep them separated (namespaces) | `ping-pong` + `log-output` moved to namespace `exercises` | `tripplen63/ping-pong:2.1`, `tripplen63/log-output:2.1` | [tag `2.3`](https://github.com/tripplen23/KubernetesSubmissions/tree/2.3/part2/2.3) |
| [`part2/2.4/`](./part2/2.4) | 2.4 — The project, step 9 (namespace) | `todo-app` + `todo-backend` moved to namespace `project` | `tripplen63/todo-app:2.2`, `tripplen63/todo-backend:2.2` | [tag `2.4`](https://github.com/tripplen23/KubernetesSubmissions/tree/2.4/part2/2.4) |
| [`part2/2.5/`](./part2/2.5) | 2.5 — Documentation and ConfigMaps | `log-output` reads ConfigMap file + env (namespace `exercises`) | `tripplen63/log-output:2.5`, `tripplen63/ping-pong:2.1` | [tag `2.5`](https://github.com/tripplen23/KubernetesSubmissions/tree/2.5/part2/2.5) |
| [`part2/2.6/`](./part2/2.6) | 2.6 — The project, step 10 (no hardcoded config) | `todo-app` + `todo-backend` config via ConfigMap/env (namespace `project`) | `tripplen63/todo-app:2.6`, `tripplen63/todo-backend:2.6` | [tag `2.6`](https://github.com/tripplen23/KubernetesSubmissions/tree/2.6/part2/2.6) |
| [`part2/2.7/`](./part2/2.7) | 2.7 — Stateful applications | `ping-pong` counter in Postgres StatefulSet (namespace `exercises`) | `tripplen63/ping-pong:2.7` | [tag `2.7`](https://github.com/tripplen23/KubernetesSubmissions/tree/2.7/part2/2.7) |
| [`part2/2.8/`](./part2/2.8) | 2.8 — The project, step 11 (todos in Postgres) | `todo-backend` stores todos in Postgres StatefulSet via Secret/ConfigMap (namespace `project`) | `tripplen63/todo-backend:2.8`, `tripplen63/todo-app:2.6` | [tag `2.8`](https://github.com/tripplen23/KubernetesSubmissions/tree/2.8/part2/2.8) |

## Prerequisites

A running Kubernetes cluster with `kubectl` connected to it. Quickstart with k3d:

```bash
curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash
k3d cluster create
kubectl cluster-info
```
