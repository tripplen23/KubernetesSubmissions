# KubernetesSubmissions

Submissions for the University of Helsinki's **DevOps with Kubernetes** MOOC
(2026 edition, chapter 2).

Each exercise lives in its own folder and is self-contained: source code,
multi-stage `Dockerfile`, Kubernetes manifests, and a per-exercise `README.md`
that mirrors the original assignment text plus the link to the published
Docker Hub image. The structure follows the convention used by
[VikSil/DevOps_with_Kubernetes](https://github.com/VikSil/DevOps_with_Kubernetes).

## Exercises

| Folder | Exercise | App | Image | Tag |
|---|---|---|---|---|
| [`1.1/`](./1.1) | 1.1 — Getting started | `log_output` (Rust + tokio) | `tripplen23/log-output:1.1` | [`1.1`](https://github.com/tripplen23/KubernetesSubmissions/releases/tag/1.1) |
| [`1.2/`](./1.2) | 1.2 — Project v0.1 | `todo-app` (Rust + axum) | `tripplen23/todo-app:1.2` | [`1.2`](https://github.com/tripplen23/KubernetesSubmissions/releases/tag/1.2) |
| [`1.3/`](./1.3) | 1.3 — Declarative approach | `log_output` (manifests) | `tripplen23/log-output:1.3` | [`1.3`](https://github.com/tripplen23/KubernetesSubmissions/releases/tag/1.3) |
| [`1.4/`](./1.4) | 1.4 — Project v0.2 | `todo-app` (manifests) | `tripplen23/todo-app:1.4` | [`1.4`](https://github.com/tripplen23/KubernetesSubmissions/releases/tag/1.4) |

## Prerequisites

A running Kubernetes cluster with `kubectl` connected to it. Quickstart with k3d:

```bash
curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash
k3d cluster create
kubectl cluster-info
```

## Layout

```
KubernetesSubmissions/
├── 1.1/                Exercise 1.1 — log_output (Getting started)
│   ├── src/main.rs
│   ├── Dockerfile
│   ├── manifests/deployment.yaml
│   └── README.md
├── 1.2/                Exercise 1.2 — todo-app v0.1
│   ├── src/main.rs
│   ├── Dockerfile
│   ├── manifests/{deployment,service}.yaml
│   └── README.md
├── 1.3/                Exercise 1.3 — log_output (Declarative approach)
├── 1.4/                Exercise 1.4 — todo-app v0.2
└── README.md           (this file)
```

## Submission

Each exercise is published as a GitHub release with a tag matching the
exercise number (`1.1`, `1.2`, `1.3`, `1.4`). The link in the submission form
is the GitHub release URL for that tag.
