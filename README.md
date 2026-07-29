# KubernetesSubmissions

Submissions for the University of Helsinki's **DevOps with Kubernetes** MOOC

## Exercises

| Folder | Exercise | App | Image | Submit link |
|---|---|---|---|---|
| [`part1/1.1/`](./part1/1.1) | 1.1 — Getting started | `log_output` (Rust + tokio) | `tripplen63/log-output:1.1` | [tag `1.1`](https://github.com/tripplen63/KubernetesSubmissions/tree/1.1/part1/1.1) |
| [`part1/1.2/`](./part1/1.2) | 1.2 — Project v0.1 | `todo-app` (Rust + axum) | `tripplen63/todo-app:1.2` | [tag `1.2`](https://github.com/tripplen63/KubernetesSubmissions/tree/1.2/part1/1.2) |
| [`part1/1.3/`](./part1/1.3) | 1.3 — Declarative approach | `log_output` (manifests) | `tripplen63/log-output:1.3` | [tag `1.3`](https://github.com/tripplen63/KubernetesSubmissions/tree/1.3/part1/1.3) |
| [`part1/1.4/`](./part1/1.4) | 1.4 — Project v0.2 | `todo-app` (manifests) | `tripplen63/todo-app:1.4` | [tag `1.4`](https://github.com/tripplen63/KubernetesSubmissions/tree/1.4/part1/1.4) |

> Submission rule used in this course: the submission link is the
> **GitHub repository URL pinned to a tag** that matches the exercise number
> (e.g. `.../tree/1.1/part1/1.1`). Tag → commit mapping:
>
> | Tag | Commit | Points at |
> |---|---|---|
> | `1.1` | `114a6bd` | `part1/1.1/` — log_output (Getting started) |
> | `1.2` | `8b19e3d` | `part1/1.2/` — todo-app v0.1 |
> | `1.3` | `e9776ac` | `part1/1.3/` — log_output (declarative) |
> | `1.4` | `4833b17` | `part1/1.4/` — todo-app v0.2 (declarative) |

## Prerequisites

A running Kubernetes cluster with `kubectl` connected to it. Quickstart with k3d:

```bash
curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash
k3d cluster create
kubectl cluster-info
```