# Exercise 1.3 — Declarative approach (log_output, v2)

## Assignment

> Re-do Exercise 1.1 in a more **declarative** way. Use a manifest that defines
> the desired state of the deployment (or several deployments), and apply it
> with `kubectl apply -f`. The application should still print a random string
> to stdout every 5 seconds.

## Solution

- **Source**: see [`src/main.rs`](./src/main.rs) — same Rust binary as 1.1
- **Image**: [`tripplen23/log-output:1.3`](https://hub.docker.com/r/tripplen23/log-output/tags)
- **Manifest**: [`manifests/deployment.yaml`](./manifests/deployment.yaml) — labelled, `imagePullPolicy: Always`

This exercise focuses on the *workflow*: commit manifest → `kubectl apply -f`
→ cluster converges to desired state. No `kubectl run` imperative commands.

### Deploy

```bash
cd part1/1.3
kubectl apply -f manifests/deployment.yaml
kubectl get deploy,pods -l app=log-output
kubectl logs -f -l app=log-output
```

Re-applying the manifest after edits is a no-op when the desired state is
unchanged; deleting the resource (`kubectl delete -f manifests/deployment.yaml`)
and re-applying brings it back.
