# Exercise 3.8 — The project, step 17: deleting a branch deletes the environment

> Follow-up to 3.7. The pipeline now deploys each branch to a namespace
> named after it — but when a branch is deleted, its namespace (with its
> todo-app, backend, postgres, cron) stays behind. This lab adds a
> **second workflow** that listens for branch deletions and deletes the
> matching namespace. No app code changes — a new workflow file only.

## Goal

```text
feature deleted                     main (untouched)
      │                                 │
      ▼                                 ▼
 namespace X  --deleted-->       namespace project stays
   (workflow "Delete environment")
```

## Knowledge — the `delete` event

- A workflow with `on: delete` fires whenever **any ref (branch or tag)
  is deleted** on GitHub — from the UI, API, or `git push origin
  --delete <branch>`.
- **Delete workflows run from the default branch** (`main`): the
  workflow file must be **on `main`**, not on the branch being deleted.
- In the run, `github.ref_name` / `GITHUB_REF_NAME` is the **short name**
  of the deleted ref (e.g. `feat37`), and `GITHUB_REF_TYPE` tells you
  whether it was a `branch` or a `tag` — tag deletions must be ignored.
- **Do NOT use `actions/checkout` in this workflow.** On a `delete`
  event the deleted ref no longer exists, so checkout of that ref fails.
  Cleanup needs no source code — auth + credentials + `kubectl` are
  enough.
- `kubectl delete namespace <ns> --ignore-not-found` — removes the whole
  environment (all workloads it contains) and does not fail when the
  namespace is already gone.
- Reuses the same WIF chain (SA `github-actions-sa`, pool
  `github-pool`, the 3 secrets) — nothing new to create.

---

## Step 1 — Create the new workflow (full file)

New file **`.github/workflows/delete-environment.yaml`** (next to
`main.yaml`, at the repo root):

```yaml
name: Delete environment

on:
  delete

env:
  PROJECT_ID: ${{ secrets.GKE_PROJECT }}
  GKE_CLUSTER: dwk-cluster
  GKE_ZONE: europe-north1-c

jobs:
  delete-environment:
    name: Delete namespace for deleted branch
    runs-on: ubuntu-latest
    permissions:
      id-token: write
      contents: read

    steps:
      # NOTE: no checkout — on `delete` the ref no longer exists.

      - uses: google-github-actions/auth@v3
        with:
          workload_identity_provider: '${{ secrets.WORKLOAD_IDENTITY_PROVIDER }}'
          service_account: '${{ secrets.SERVICE_ACCOUNT }}'

      - name: 'Set up Cloud SDK'
        uses: google-github-actions/setup-gcloud@v3

      - name: 'Get GKE credentials'
        uses: 'google-github-actions/get-gke-credentials@v3'
        with:
          cluster_name: '${{ env.GKE_CLUSTER }}'
          project_id: '${{ env.PROJECT_ID }}'
          location: '${{ env.GKE_ZONE }}'

      - name: 'Delete branch environment'
        run: |
          # on: delete fires for tags too — only react to branch deletions
          if [ "$GITHUB_REF_TYPE" != "branch" ]; then
            echo "deleted ref is a $GITHUB_REF_TYPE — nothing to do"
            exit 0
          fi

          NAMESPACE="$GITHUB_REF_NAME"
          echo "deleted branch → deleting namespace $NAMESPACE"

          # never touch the project environment by accident
          if [ "$NAMESPACE" = "main" ]; then
            echo "main deleted? refusing to delete project"
            exit 0
          fi

          kubectl delete namespace "$NAMESPACE" --ignore-not-found
```

### What it does line by line

| Line | Why |
|---|---|
| `on: delete` | any ref (branch/tag) deleted anywhere |
| `$GITHUB_REF_TYPE != "branch"` | skip tag deletions |
| `NAMESPACE="$GITHUB_REF_NAME"` | deleted branch name == its namespace (3.7 naming) |
| `"main"` guard | hard safety rail protecting namespace `project` |
| `--ignore-not-found` | branch deleted twice / namespace already gone → still green |

---

## Step 2 — End-to-end test (uses 3.7, still on the local lab repo)

Requires the 3.7-patched pipeline and the cluster from 3.7 running.

```bash
# 1) create + push a test branch — 3.7 pipeline builds its namespace
git checkout -b feat38test
echo "" >> part3/3.6/todo-app/src/main.rs
git add -A && git commit -m "test: branch env for 3.8" && git push -u origin feat38test

# 2) wait for that run to deploy, then spot-check
gh run list --branch feat38test --limit 1
kubectl get pods -n feat38test -o wide    # Running: todo-app, todo-backend, postgres-ss-0

# 3) delete the branch — both the remote branch AND its environment should die
#    (delete fires the new workflow)
git push origin --delete feat38test

# 4) find the cleanup run (separate workflow, separate name) and verify
gh run list --workflow 'Delete environment' --limit 1
gh run watch

# 5) namespace must be GONE (workloads + its PVCs are deleted with it)
kubectl get ns feat38test          # Error from server (NotFound)
```

After cleanup, `git branch -d feat38test` locally too.

---

## Step 3 — Verify the protection rules

```bash
# a TAG deletion must NOT delete anything:
git tag test-tag && git push origin test-tag && git push origin --delete test-tag
gh run list --workflow 'Delete environment' --limit 1   # run shows "deleted ref is a tag — nothing to do"
kubectl get ns project   # still here ✔
```

---

## Step 4 — Clean up (only after the whole chapter is done)

The whole chain is still needed until you exit the course — really:

```bash
# cluster + images + repo (same as 3.6):
gcloud container clusters delete dwk-cluster --zone=europe-north1-c --project=dwk-gke-506208
gcloud artifacts repositories delete my-repository --location=europe-north1 --project=dwk-gke-506208 --async
gcloud container images delete gcr.io/dwk-gke-506208/postgres:16 --quiet --force-delete-tags
gcloud compute disks list --project=dwk-gke-506208     # any pvc-* → delete them

# ONLY after you're done with the course — the OIDC chain:
gcloud iam workload-identity-pools delete github-pool --location=global --project=dwk-gke-506208
gcloud iam service-accounts delete github-actions-sa@dwk-gke-506208.iam.gserviceaccount.com --quiet --project=dwk-gke-506208
gh secret delete GKE_PROJECT SERVICE_ACCOUNT WORKLOAD_IDENTITY_PROVIDER --repo tripplen23/KubernetesSubmissions
```