# 4.7 — Baby steps to GitOps

The chapter reverses the deployment pipeline: instead of CI *pushing* into the cluster,
the cluster *pulls* the desired state from a Git repository. **ArgoCD** is the tool, and
this lab is the smallest honest version of the chapter's flow: an app whose image tag
lives in a repository, ArgoCD watching that repository, and a commit — nothing else —
that changes what runs.

One difference from the chapter is forced by this cluster: **it cannot reach
`github.com`** — nor `gitlab.com`, `codeberg.org` or `bitbucket.org` (measured from a
pod: `000 in 8s`). ArgoCD clones over the network, so the repository has to live inside
the cluster, and the lab runs a small **Gitea** beside it. Everything after that is the
chapter's flow unchanged: a repo, a Kustomization, an `Application`, automated sync,
selfHeal.

---

## Step 0 — what you need in front of you

- the GKE cluster and `kubectl` pointing at it;
- `docker` (images are mirrored into your registry, because pods cannot reach
  `quay.io`, `ghcr.io` or `public.ecr.aws`);
- `git`;
- **the `kustomize` CLI** — `kubectl kustomize` renders, but `kustomize edit` is what the
  release step needs:

```bash
curl -s "https://raw.githubusercontent.com/kubernetes-sigs/kustomize/master/hack/install_kustomize.sh" | bash
sudo mv kustomize /usr/local/bin/
kustomize version
```

- and room in the cluster: ArgoCD is seven pods and Gitea one. Check with the lab's own
  capacity script and free the previous labs' pods if the scheduler complains:

```bash
kubectl -n project get deploy broadcaster -o jsonpath='{.spec.replicas}{"\n"}'
kubectl -n project scale deploy broadcaster --replicas=1
```

---

## Step 1 — push and pull are two different problems

Today's pipeline (3.6) *pushes*: GitHub Actions builds an image, then calls
`kubectl apply` on the cluster. That works because the pipeline holds cluster
credentials — which is exactly the problem. Anyone who can run the pipeline can change
the cluster, and a cluster that cannot be reached from outside (a laptop, a private
network) cannot be deployed to at all.

GitOps reverses it: CI still builds and publishes the image, but it no longer touches
the cluster. It writes *what should run* into a repository, and a component inside the
cluster — ArgoCD — reads that repository and makes it true. The repository becomes the
only source of truth for the cluster's state, so:

- nobody needs cluster access except the cluster itself — the security argument;
- every change to the cluster is a commit: reviewable, revertible, attributable;
- the same repository can be pointed at another cluster, which is then simply *that*
  state.

The exercise's own words: *"when you commit to the repository, the application is
automatically updated"*. That is the whole lab — with a repository this cluster can
actually reach.

---

## Step 2 — the app, and the image that will move

The app is this folder's `log-output` — a Rust/axum service that prints one line every
five seconds (the chapter's "log output" idea) and serves a page saying which version it
is running:

```text
[log] 1789656008 6cb77df4 version=unknown
```

`APP_VERSION` comes from the environment, so the Deployment — not the image — decides
what the page claims. That is deliberate: the GitOps demo changes the Deployment, and
the page must show it.

Type the Dockerfile beside the app:

`part4/4.7/log-output/Dockerfile`

```dockerfile
FROM rust:1.85-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/log-output /usr/local/bin/log-output
EXPOSE 3000
CMD ["/usr/local/bin/log-output"]
```

```bash
R=europe-north1-docker.pkg.dev/dwk-gke-506208/my-repository
docker build -t $R/log-output:4.7 part4/4.7/log-output
docker push $R/log-output:4.7
```

The app runs as a plain container too, which is the quickest way to see what it does. Publish
it on **3100** — 3000 is a port the rest of the lab needs (Step 3 port-forwards Gitea there):

```bash
docker run --rm -p 3100:3000 -e APP_VERSION=v1 $R/log-output:4.7
# Ctrl-C when you have seen the log lines
```

```bash
curl -s localhost:3100/ | grep -o "version: <b>[^<]*</b>"
curl -s localhost:3100/healthz
```

---

## Step 3 — a git server inside the cluster

The images first, because pods here cannot reach the registries Gitea and ArgoCD
publish to:

```bash
R=europe-north1-docker.pkg.dev/dwk-gke-506208/my-repository
docker pull gitea/gitea:1.24                       && docker push $R/gitea:1.24
docker pull quay.io/argoproj/argocd:v3.5.3         && docker push $R/argocd:v3.5.3
docker pull ghcr.io/dexidp/dex:v2.45.1             && docker push $R/dex:v2.45.1
docker pull public.ecr.aws/docker/library/redis:8.2.3-alpine && docker push $R/redis:8.2.3-alpine
```

Then the namespace and Gitea itself. Note the strategy: Gitea keeps its repositories on
a **ReadWriteOnce** volume, and a rolling update would leave the new pod waiting for a
volume the old pod still holds — the `Multi-Attach` deadlock from 4.5. A single replica
with a `Recreate` strategy is the correct shape here, not a workaround:

`part4/4.7/gitea/gitea.yaml`

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: gitea-data
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 1Gi
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: gitea
  labels:
    app: gitea
spec:
  replicas: 1
  strategy:
    type: Recreate
  selector:
    matchLabels:
      app: gitea
  template:
    metadata:
      labels:
        app: gitea
    spec:
      containers:
        - name: gitea
          image: europe-north1-docker.pkg.dev/dwk-gke-506208/my-repository/gitea:1.24
          ports:
            - containerPort: 3000
          env:
            - name: GITEA__database__DB_TYPE
              value: sqlite3
            - name: GITEA__server__ROOT_URL
              value: http://gitea.gitops.svc.cluster.local:3000/
            - name: GITEA__server__SSH_DISABLED
              value: "true"
            - name: GITEA__security__INSTALL_LOCK
              value: "true"
            - name: GITEA__service__DISABLE_REGISTRATION
              value: "true"
            - name: GITEA__repository__ENABLE_PUSH_CREATE_USER
              value: "true"
            - name: GITEA__repository__DEFAULT_PRIVATE
              value: public
          resources:
            requests:
              cpu: 50m
              memory: 192Mi
            limits:
              cpu: 500m
              memory: 512Mi
          volumeMounts:
            - name: data
              mountPath: /data
      volumes:
        - name: data
          persistentVolumeClaim:
            claimName: gitea-data
---
apiVersion: v1
kind: Service
metadata:
  name: gitea
spec:
  selector:
    app: gitea
  ports:
    - port: 3000
      targetPort: 3000
```

Three settings are doing the work that would otherwise be manual setup:
`ENABLE_PUSH_CREATE_USER` lets a `git push` create its own repository,
`DEFAULT_PRIVATE=public` makes ArgoCD able to clone it without credentials, and
`INSTALL_LOCK` skips the web installer.

```bash
kubectl create namespace gitops
kubectl apply -n gitops -f part4/4.7/gitea/gitea.yaml
kubectl rollout status deploy/gitea -n gitops
```

An admin account, so you can push with a password over HTTP — **one line, all of it**:

```bash
kubectl -n gitops exec deploy/gitea -- env GITEA_I_AM_BEING_UNSAFE_RUNNING_AS_ROOT=true gitea admin user create --username gitops --password gitops-lab47 --email gitops@example.com --admin --must-change-password=false
```

```bash
# it worked if the list now shows the account:
kubectl -n gitops exec deploy/gitea -- env GITEA_I_AM_BEING_UNSAFE_RUNNING_AS_ROOT=true gitea admin user list
```

From here on the repository server is at `gitea.gitops.svc.cluster.local:3000`, and the
work happens with ordinary `git`. Make the config directory that the rest of the lab
uses — it lives in this lab's folder, so the repository you push *is* part of your
submission:

```bash
kubectl -n gitops port-forward svc/gitea 3000:3000
# in a second terminal, from the repository root
mkdir -p part4/4.7/config && cd part4/4.7/config && git init -b main
git remote add gitea http://gitops:gitops-lab47@localhost:3000/gitops/config.git
```

![Gitea's front page on localhost:3000, version 1.24.7](./assets/image1.png)

The remote named `gitea` is what the pushes below use — an explicit remote avoids the
*"The current branch main has no upstream branch"* error, which is git saying it was given
no remote at all.

**Looking at what you pushed.** Open <http://localhost:3000> and sign in with the account
from the command above — **username `gitops`, password `gitops-lab47`**:

- the repository is at <http://localhost:3000/gitops/config> — the file tree (`base/`,
  `overlays/prod/`) and the commit under it;
- direct links: `/gitops/config/src/branch/main/overlays/prod/kustomization.yaml` for the
  file, `/gitops/config/commits/branch/main` for the log;
- the form also offers **Sign in with a security key**, which fails with *"Could not read
  your security key … relying party ID is not a registrable domain suffix"*: Gitea's
  configured URL is `gitea.gitops.svc.cluster.local` (what ArgoCD needs), so the browser
  refuses the passkey on `localhost`. Use the username and password fields.

---

## Step 4 — ArgoCD

The install manifest comes from GitHub, and **your laptop** fetches it — the cluster
never does:

```bash
curl -sL https://raw.githubusercontent.com/argoproj/argo-cd/stable/manifests/install.yaml \
  -o /tmp/argocd-install.yaml
grep -E "^\s+image: " /tmp/argocd-install.yaml | sort -u
```

```text
image: ghcr.io/dexidp/dex:v2.45.1
image: public.ecr.aws/docker/library/redis:8.2.3-alpine
image: quay.io/argoproj/argocd:v3.5.3
```

Three images, none of them reachable by a pod — so they are replaced by the mirrors
before the file is applied:

```bash
R=europe-north1-docker.pkg.dev/dwk-gke-506208/my-repository
sed -e "s|quay.io/argoproj/argocd:v3.5.3|$R/argocd:v3.5.3|g" \
    -e "s|ghcr.io/dexidp/dex:v2.45.1|$R/dex:v2.45.1|g" \
    -e "s|public.ecr.aws/docker/library/redis:8.2.3-alpine|$R/redis:8.2.3-alpine|g" \
    /tmp/argocd-install.yaml > /tmp/argocd-mirrored.yaml

kubectl create namespace argocd
kubectl apply --server-side -n argocd -f /tmp/argocd-mirrored.yaml
```

Check the file you are about to apply — **every** image must be yours, and this is the check
that catches a missed substitution (one is easy to miss):

```bash
grep -E "^\s+image: " /tmp/argocd-mirrored.yaml | sort -u
```

If a pod still sits in `ImagePullBackOff` afterwards it is one of two things: the `sed`
missed that occurrence, or a StatefulSet's pod survived the apply. Both are one command
away:

```bash
kubectl -n argocd set image deploy/argocd-applicationset-controller "*=$R/argocd:v3.5.3"
kubectl -n argocd delete pod argocd-application-controller-0
```

> **The namespace is not optional.** The install manifest's objects carry no
> `namespace:` field, so `kubectl apply -f /tmp/argocd-mirrored.yaml` without `-n argocd`
> puts the whole of ArgoCD into whatever namespace your context happens to be in — for
> this project that is `project`, next to your todos. If that happens, the objects are
> easy to find again, because the manifest labels everything
> `app.kubernetes.io/part-of: argocd`:
>
> ```bash
> kubectl -n project delete all,sa,cm,secret,role,rolebinding,networkpolicy \
>   -l app.kubernetes.io/part-of=argocd
> ```

Seven pods start. Two things on this cluster differ from the chapter's instructions, and
both have the same shape — *the pod cannot reach the internet*:

- **`ImagePullBackOff` on all seven pods**, and with it *"secrets
  'argocd-initial-admin-secret' not found"*: `argocd-server` creates that secret when it
  starts, so it cannot exist before the images pull. It means the `sed` above was skipped
  — apply again.
- **A `LoadBalancer` service never becomes reachable here.** The chapter patches
  `argocd-server` to `LoadBalancer`; these nodes have no external addresses, so
  port-forward instead and leave the service as it is.

The UI is served by `argocd-server` on 443, and a port-forward is how you reach it:

```bash
kubectl -n argocd get pods
kubectl -n argocd port-forward svc/argocd-server 8080:443

# the initial admin password, as the chapter says: base64 in a Secret
kubectl -n argocd get secret argocd-initial-admin-secret \
  -o jsonpath='{.data.password}' | base64 -d; echo
```

Open <https://localhost:8080>, accept the certificate, log in as `admin`. The first
screen is empty — there is nothing to sync yet.

![The ArgoCD login page at localhost:8080, the chapter's first screen](./assets/image.png)

---

## Step 5 — the state in a repository

This is the state ArgoCD will keep: a Kustomize base describing the app, and one
overlay per environment. Type these four files (they are what you push to Gitea):

`part4/4.7/config/base/kustomization.yaml`

```yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - deployment.yaml
  - service.yaml
```

`part4/4.7/config/base/deployment.yaml` — note `PROJECT/IMAGE`, a placeholder the overlay replaces:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: log-output-dep
spec:
  replicas: 1
  selector:
    matchLabels:
      app: log-output
  template:
    metadata:
      labels:
        app: log-output
    spec:
      containers:
        - name: log-output
          image: PROJECT/IMAGE
          ports:
            - containerPort: 3000
          env:
            - name: APP_VERSION
              value: v1
          resources:
            requests:
              cpu: 20m
              memory: 32Mi
            limits:
              cpu: 100m
              memory: 128Mi
```

`part4/4.7/config/base/service.yaml`

```yaml
apiVersion: v1
kind: Service
metadata:
  name: log-output-svc
spec:
  selector:
    app: log-output
  ports:
    - port: 3000
      targetPort: 3000
```

`part4/4.7/config/overlays/prod/kustomization.yaml` — it refers to the base, renames
everything with a prefix, chooses the namespace and fills in the real image:

```yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - ../../base
namePrefix: prod-
namespace: prod
images:
  - name: PROJECT/IMAGE
    newName: europe-north1-docker.pkg.dev/dwk-gke-506208/my-repository/log-output
    newTag: "4.7"
```

Render it before pushing — Kustomize is the thing that decides what ArgoCD will see:

```bash
cd part4/4.7/config
kustomize build overlays/prod
```

```bash
git add -A
git commit -m "base + prod overlay"
git push -u gitea main
```

![Gitea's activity feed: gitops created the repository and pushed to main](./assets/image2.png)
![The gitops/config repository with its one commit, base + prod overlay](./assets/image3.png)

That push also creates the repository: Gitea accepts the first push as the repository's
creation. Two things must be true before going on, because an `Application` pointing at a
repository that does not exist — or that ArgoCD is not allowed to read — shows `Unknown`
with *"failed to list refs: authentication required: Unauthorized"*:

```bash
# the repository exists AND is readable without credentials (ArgoCD reads it anonymously)
curl -s -o /dev/null -w "%{http_code}\n" http://localhost:3000/api/v1/repos/gitops/config
```

```text
200
```

`404` here means the repository is **private** — and a push-created repository can come out
private even with `DEFAULT_PRIVATE=public` set. Flip it over the API with the account from
Step 3, or with **Settings → Make public** in the UI:

```bash
curl -s -X PATCH -u gitops:gitops-lab47 -H "Content-Type: application/json" \
  -d '{"private": false}' http://localhost:3000/api/v1/repos/gitops/config \
  | python3 -m json.tool | grep '"private"'
```

```text
  "private": false,
```

Then run the check again: `200` is what ArgoCD needs, and a public repository is what keeps
this lab free of credentials.

---

## Step 6 — the Application: first in the UI, then in YAML

ArgoCD does nothing by itself. An `Application` says *which repository, which path, which
cluster, which namespace* — and you can meet that object in the UI before writing it.

**Creating it by hand.** With the UI open (Step 4), press **+ NEW APP** at the top and
fill the panel in:

- **General** — Application Name `log-output`, Project `default`, **Sync Policy**
  **Automatic** — tick **ENABLE AUTO-SYNC**, then **PRUNE RESOURCES** and **SELF HEAL**
  (these three are the `automated` options the YAML sets further down);
- **Source** — Repository URL
  `http://gitea.gitops.svc.cluster.local:3000/gitops/config.git`, Revision `HEAD`,
  Path `overlays/prod`;
- **Destination** — Cluster URL `https://kubernetes.default.svc` (in-cluster),
  Namespace `prod`. Under **SYNC OPTIONS** also tick **AUTO-CREATE NAMESPACE**: the `prod`
  namespace does not exist yet, and the YAML says the same thing as `CreateNamespace=true`.

![The NEW APP form for log-output, with Auto-Sync, Prune and Self Heal ticked](./assets/image4.png)

Press **CREATE**. The card appears as `OutOfSync` and turns `Synced`, `Progressing`
becomes `Healthy` — the app is running, and you never touched `kubectl`.

![The log-output card in ArgoCD: Synced and Healthy](./assets/image5.png)

![The resource tree: the Service, the Deployment and its pod, all green](./assets/image6.png)

**Reading it — the four places worth knowing.**

- **The two badges** at the top of the app card: *Sync Status* (`Synced` = the cluster
  matches the repository) and *Health* (`Healthy` = the workloads are actually up). From
  the terminal, the same information:

```bash
kubectl -n argocd get application log-output
```

  If both are **blank**, nothing is reconciling: the `argocd-application-controller` pod is
  what fills them in, so check `kubectl -n argocd get pods` before wondering about the
  application.

- **The resource tree** (the app's graph view): `Deployment → ReplicaSet → Pod`, each node
  with its own status. The Deployment node shows the replicas that are ready, e.g. `1/1`,
  with the pods underneath it. The two ways to ask "how many are running?":

```bash
kubectl -n prod get deploy,rs,pods
kubectl -n prod get deploy prod-log-output-dep \
  -o jsonpath='{.status.readyReplicas}{"/"}{.status.replicas}{"\n"}'
```

![The three verification commands: Synced/Healthy, the Deployment at 1/1, one running pod](./assets/image7.png)

- **SYNC and REFRESH** (top of the app): *Refresh* re-reads the repository now instead of
  waiting for the next poll, *Sync* reconciles immediately, *Hard Refresh* also drops
  ArgoCD's cached manifests. None of them changes the repository — they only make ArgoCD
  notice it sooner.

- **HISTORY AND ROLLBACK** (in the app's panel): one entry per revision ArgoCD has
  deployed. A rollback re-deploys an older revision, which with `selfHeal` on lasts
  exactly until the next sync restores the repository's version. The durable "rollback"
  is a revert commit — that is the point of all this.

**The same object, from a file.** The UI just wrote an object into the cluster; in a
repository you write it yourself, which is what the chapter's exercises expect. Delete the
UI-made app first so the two do not collide:

```bash
kubectl -n argocd delete application log-output
```

`part4/4.7/application.yaml`

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: log-output
  namespace: argocd
spec:
  project: default
  source:
    repoURL: http://gitea.gitops.svc.cluster.local:3000/gitops/config.git
    path: overlays/prod
    targetRevision: HEAD
  destination:
    server: https://kubernetes.default.svc
    namespace: prod
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
      - CreateNamespace=true
```

```bash
kubectl apply -n argocd -f part4/4.7/application.yaml

# watch it: OutOfSync → Synced, Progressing → Healthy
kubectl -n argocd get application log-output -w
```

```text
NAME         SYNC STATUS   HEALTH STATUS
log-output   Synced        Healthy
```

Two fields are the whole GitOps contract:

- `automated.prune` — an object deleted from the repository is deleted from the cluster;
- `automated.selfHeal` — a change made *by hand* in the cluster is reverted to what the
  repository says.

In the UI, the app's tree shows what the chapter explains: a **Deployment** that owns a
**ReplicaSet**, which owns the **Pods** — the ReplicaSet is the level that keeps the
promised number of pods alive, which is why deleting a pod changes nothing in Git.

```bash
kubectl -n prod get pods
kubectl -n prod port-forward svc/prod-log-output-svc 3001:3000
curl -s localhost:3001/ | grep -o "version: <b>[^<]*</b>"
```

```text
version: <b>v1</b>
```

---

## Step 7 — the two proofs

**A commit is the only thing that changes the cluster.** Edit the overlay to release
`v2` — one more patch file, which changes only what differs from the base:

`part4/4.7/config/overlays/prod/deployment.yaml`

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: log-output-dep
spec:
  template:
    spec:
      containers:
        - name: log-output
          env:
            - name: APP_VERSION
              value: v2
```

and reference it in the overlay — the whole file now:

`part4/4.7/config/overlays/prod/kustomization.yaml`

```yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - ../../base
namePrefix: prod-
namespace: prod
patches:
  - path: deployment.yaml
images:
  - name: PROJECT/IMAGE
    newName: europe-north1-docker.pkg.dev/dwk-gke-506208/my-repository/log-output
    newTag: "4.7"
```

```bash
cd part4/4.7/config
git add -A && git commit -m "release v2" && git push gitea main
```

ArgoCD polls the repository — its default interval is 180 seconds, so this takes a
couple of minutes unless you press **Refresh** in the UI. Watch it happen:

![The application after the v2 commit, the Deployment rolling out a new ReplicaSet](./assets/image8.png)
![Synced to HEAD 33bf607 — the release commit, deployed without anyone touching the cluster](./assets/image9.png)

```bash
kubectl -n prod get deploy prod-log-output-dep \
  -o jsonpath='{.spec.template.spec.containers[0].env[?(@.name=="APP_VERSION")].value}{"\n"}'
kubectl -n prod rollout status deploy/prod-log-output-dep
curl -s localhost:3001/ | grep -o "version: <b>[^<]*</b>"
```

```text
version: <b>v2</b>
```

In the UI, that release is a sequence worth watching: the app turns `OutOfSync`, the
Deployment node spins a new ReplicaSet and pod, and the card settles back to `Synced` /
`Healthy`. If nothing happens within a few minutes, press **REFRESH** — the poll interval
is what you are waiting for, not a failure.

**A hand-made change is undone.** This is `selfHeal`, and it is the difference between a
deployment tool and a state machine:

```bash
kubectl -n prod scale deploy prod-log-output-dep --replicas=5
kubectl -n prod get deploy prod-log-output-dep   # five for a moment…
# …and then ArgoCD notices the drift and puts it back:
kubectl -n prod get deploy prod-log-output-dep   # one again
```

![Scaled to five by hand — and back at 1/1 once ArgoCD reconciled](./assets/image10.png)

The UI shows it too: the app goes `OutOfSync`, the tree grows five pods, and the extra
pods terminate when the next poll arrives. `kubectl edit` on an image or an env var
behaves the same way — the next reconciliation restores the repository's version. The way
to change the cluster is to change the repository.

---

## Step 8 — the pipeline that commits for you

The chapter's workflow builds the image, bumps the tag in `kustomization.yaml` with
`kustomize edit set image`, and commits that change back to the repository — which is
what triggers ArgoCD. Since CI already knows how to publish to Artifact Registry (3.6),
the only new pieces are the last two steps:

`part4/4.7/.github/workflows/release.yaml`

```yaml
name: Build, publish and release

on:
  push:
    branches: [main]

permissions:
  contents: write          # the workflow commits kustomization.yaml back to the repo

jobs:
  build-publish-release:
    name: Build, Push, Release
    runs-on: ubuntu-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v6

      - name: Authenticate to Google Cloud
        uses: google-github-actions/auth@v3
        with:
          workload_identity_provider: ${{ secrets.WORKLOAD_IDENTITY_PROVIDER }}
          service_account: ${{ secrets.SERVICE_ACCOUNT }}

      - name: Build and publish log-output
        run: |
          gcloud auth configure-docker europe-north1-docker.pkg.dev -q
          R=europe-north1-docker.pkg.dev/${{ secrets.GKE_PROJECT }}/my-repository
          docker build -t "$R/log-output:$GITHUB_SHA" part4/4.7/log-output
          docker push "$R/log-output:$GITHUB_SHA"

      - name: Set up Kustomize
        uses: imranismail/setup-kustomize@v3

      - name: Point the overlay at the new image
        run: |
          cd part4/4.7/config/overlays/prod
          kustomize edit set image \
            PROJECT/IMAGE=europe-north1-docker.pkg.dev/${{ secrets.GKE_PROJECT }}/my-repository/log-output:$GITHUB_SHA

      - name: Commit the release
        uses: EndBug/add-and-commit@v10
        with:
          add: part4/4.7/config/overlays/prod/kustomization.yaml
          message: "Release ${{ github.sha }}"
```

> **Where the file has to live.** GitHub only runs workflows from `.github/workflows` at
> the **root** of the repository (the docs are explicit: *"You must store workflow files in
> the `.github/workflows` directory of your repository"*). A `.github/workflows/release.yaml`
> inside `part4/4.7/` is therefore inert — it is documentation. That is on purpose: this
> lab's loop is driven by hand, and the file shows what CI would do instead.
>
> This workflow commits into *this* GitHub repository, which ArgoCD cannot read — so in
> this lab you do its two interesting steps by hand: `kustomize edit set image …` and
> `git push`. On a cluster that can reach GitHub, the same workflow with `repoURL`
> pointing at the repository is all it takes, and that is precisely what the chapter
> builds.

The one thing to notice about the shape: **CI never talks to the cluster.** It publishes
an image and writes a line of YAML. Deployment belongs to the thing that owns the state.

---

## Step 9 — cleanup

```bash
kubectl delete namespace prod
kubectl delete -n argocd -f /tmp/argocd-mirrored.yaml
kubectl delete namespace argocd
kubectl delete namespace gitops
rm -f /tmp/argocd-install.yaml /tmp/argocd-mirrored.yaml
```

The applications' CRDs are removed by the manifest; the ones belonging to Argo
**Rollouts** are not touched.

---

## P.S. — what this exercise leaves you with

- **Push and pull solve different problems.** A pipeline that pushes needs credentials
  for your cluster and cannot reach one that is not exposed; a cluster that pulls needs
  only a repository it can read.
- **The repository is the state, so drift is a bug.** `selfHeal` and `prune` are what
  turn "we deploy from Git" into "the cluster *is* Git".
- **Kustomize keeps the environments honest.** A base with the differences as overlays —
  a prefix, a namespace, an image, a patched env var — is how prod and staging stay
  recognisably the same app.
- **Secrets stay outside.** The chapter's 4.9 assumes it, and it is the standard split:
  ArgoCD reconciles configuration, not credentials.
- **What you cannot reach shapes the design.** The chapter's repository is on GitHub;
  this cluster cannot reach GitHub, so the repository runs inside it. The mechanism is
  the same, and knowing *why* it changed is worth more than following the instructions.
