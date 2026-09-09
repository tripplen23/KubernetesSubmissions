# Exercise 3.6 — The project, step 15: Deployment Pipeline (GitHub Actions)

## Goal

> **"Setup automatic deployment for the project as well."** — Exercise 3.6,
> *Deployment Pipeline* (Chapter 4)
---

## Knowledge

### 1. What the pipeline does (the whole picture)

```
1. git push                   → GitHub Actions wakes up (workflow file)
2. auth: GitHub ⇄ Google      → OIDC chain of trust (NO stored secrets!)
3. get GKE credentials        → kubectl can talk to dwk-cluster
4. configure docker           → push auth for Artifact Registry
5. build images               → todo-app, todo-backend, todo-cron
6. publish images             → europe-north1-docker.pkg.dev/.../todo-app:main-<sha>
7. kustomize edit set image   → inject fresh image tags into the manifests
8. kubectl apply -k           → deploy
9. kubectl rollout status     → wait until everything is running
```

### 2. GitHub Actions basics

- A **workflow** is a YAML file in **`.github/workflows/`** **at the ROOT
  of the repository** (the repo already has part1–part3 — the workflow
  lives *beside* them, not inside `3.6`). GitHub runs it automatically.
- `on: push` → the workflow runs on any push, on any branch (that's what
  3.7 exploits).
- `env:` — variables shared by all steps.
- Each `step` runs on the same ephemeral Ubuntu runner.
- **`${{ secrets.X }}`** reads a *secret* you store on GitHub
  (Settings → Secrets and variables → Actions). Secrets are never visible
  in logs, never in git.
- **`$GITHUB_ENV` trick**: `echo "VAR=value" >> $GITHUB_ENV` exports a
  variable to the *next* steps (that's how the image tags travel).
- **`$GITHUB_SHA`** and **`github.ref_name`** — values GitHub injects: the
  commit hash and the branch name.
- **`permissions: id-token: write`** on the job — REQUIRED for OIDC auth.
  Without it, auth fails with *"Actions did not inject
  $ACTIONS_ID_TOKEN_REQUEST_TOKEN"* (we hit this exactly).

### 3. The OIDC chain of trust — the "what did we actually do" part

The classic way: create a service account **JSON key**, store it as a
GitHub secret. Works, but a stolen key = full access, and it never expires.

The modern way: **Workload Identity Federation**. GitHub proves
who it is with a **short-lived signed token** (no stored secrets), and
Google exchanges it for a service-account token. Five pieces:

| # | Resource | What it is | Why needed |
|---|---|---|---|
| 1 | **Service Account** `github-actions-sa` | a GCP identity | GCP can't grant permissions to "GitHub" directly — the pipeline acts *as* this SA |
| 2 | **Roles on the SA** | `artifactregistry.writer` + `container.admin` | the SA's powers: push images + deploy to GKE |
| 3 | **Workload Identity Pool** `github-pool` | container for trusted external identities | scopes WHERE external tokens can come from |
| 4 | **OIDC Provider** `github-provider` | registers GitHub as trusted issuer + attribute mapping/condition (which repo) | "tokens from GitHub are trusted — and here's how to read them" |
| 5 | **Impersonation binding** | allows `principalSet` (our repo) to impersonate the SA | locks it down: ONLY this repo can use the SA |

When the pipeline runs: GitHub mints a token automatically, GCP verifies
and exchanges it for a SA token. **Nowhere is a secret stored.**

### 4. The image tag recipe — why it's so long

```
$REGISTRY/$PROJECT_ID/$REPOSITORY/$IMAGE:$BRANCH-$GITHUB_SHA
europe-north1-docker.pkg.dev/dwk-gke-506208/my-repository/todo-app:main-c00adaefda1f7169
```

- `europe-north1-docker.pkg.dev` — the Artifact Registry endpoint (region).
- `dwk-gke-506208` — project id.
- `my-repository` — the Docker repo we create in Artifact Registry.
- `todo-app` — the image name.
- `main-c00adaefda1f7169` — branch + commit → **unique per push** →
  Kubernetes always sees a *new* image → `imagePullPolicy: Always`
  redeploys.

### 5. Deployment strategy vs. ReadWriteOnce PVC (the material's warning)

The material warns: *"If your pod uses a Persistent Volume Claim access
mode ReadWriteOnce, you may need to consider the deployment strategy, since
the default (RollingUpdate) may cause problems."*

Why: a **ReadWriteOnce** volume can be mounted by **one pod at a time**.
RollingUpdate (default) starts the NEW pod *before* terminating the OLD one
(`maxSurge`) — the new pod cannot attach the RWO PVC while the old pod
still holds it → the new pod sits **Pending** forever → rollout hangs.

Our project:
- **todo-app** has the PVC (`image-claim`, RWO) → switch strategy to
  **`Recreate`** (terminate old pod first, then start new — one holder at a
  time, RWO satisfied).
- **todo-backend**: no volume → default RollingUpdate is fine.
- **postgres-ss** is a **StatefulSet**: pods update in place, one at a
  time, each with its own PVC — no conflict by design.

### 6. Namespace

The project lives in namespace `project`. Kubernetes does not auto-create
it, and the pipeline doesn't either — **you create it once** (Step 1). The
manifests pin `namespace: project`.

---

## Step 1 — GCP: cluster + namespace + Artifact Registry repo

### 1a. Cluster (private nodes — org policy blocks external IPs) + namespace

```bash
gcloud container clusters create dwk-cluster \
  --zone=europe-north1-c --cluster-version=1.36 \
  --disk-size=32 --num-nodes=4 --machine-type=e2-small \
  --enable-ip-alias --enable-private-nodes --master-ipv4-cidr=172.16.10.0/28 \
  --project=dwk-gke-506208
```

Wait for `RUNNING`, then:

```bash
gcloud container clusters update dwk-cluster --zone=europe-north1-c \
  --project=dwk-gke-506208 --no-enable-master-authorized-networks
gcloud container clusters get-credentials dwk-cluster --zone=europe-north1-c --project=dwk-gke-506208
kubectl create namespace project
```

### 1b. Artifact Registry — a Docker repository (the pipeline pushes here)

> Artifact Registry (`europe-north1-docker.pkg.dev`) is the *current*
> Google registry; `gcr.io` is legacy. The pipeline pushes to the repo
> `my-repository`.

```bash
gcloud artifacts repositories create my-repository \
  --repository-format=docker --location=europe-north1 --project=dwk-gke-506208
```

### 1c. Base image for postgres

The pipeline rebuilds the three apps, but **postgres is a public image**
(not rebuilt). GKE private nodes can't be sure they can pull Docker Hub,
so push postgres into your registry once (referenced by `postgres.yaml`):

```bash
docker pull postgres:16
docker tag postgres:16 gcr.io/dwk-gke-506208/postgres:16
docker push gcr.io/dwk-gke-506208/postgres:16
```

---

## Step 2 — GCP IAM: the OIDC chain of trust (5 commands)

Replace placeholders, for example: `PROJECT_ID=dwk-gke-506208`,
`PROJECT_NUMBER=323959491379` (run `gcloud projects list` to confirm),
`YOUR_ORG/YOUR_REPO=tripplen23/KubernetesSubmissions` (my GitHub repo).

**1. Service account + permissions** (the identity the pipeline acts as):

```bash
gcloud iam service-accounts create github-actions-sa --display-name="GitHub Actions SA" --project=PROJECT_ID

gcloud projects add-iam-policy-binding PROJECT_ID \
  --role="roles/artifactregistry.writer" \
  --member="serviceAccount:github-actions-sa@PROJECT_ID.iam.gserviceaccount.com"

gcloud projects add-iam-policy-binding PROJECT_ID \
  --role="roles/container.admin" \
  --member="serviceAccount:github-actions-sa@PROJECT_ID.iam.gserviceaccount.com"
```

> `artifactregistry.writer` = push images; `container.admin` = deploy to
> the GKE cluster.

**2. Workload identity pool** (container for the trusted identities):

```bash
gcloud iam workload-identity-pools create github-pool --location=global --display-name="GitHub Actions Pool" --project=PROJECT_ID
```

**3. OIDC provider** (register GitHub as a trusted source, restrict to YOUR
repo via the attribute-condition):

```bash
gcloud iam workload-identity-pools providers create-oidc github-provider \
  --location=global --workload-identity-pool=github-pool \
  --display-name="GitHub provider" \
  --attribute-mapping="google.subject=assertion.sub,attribute.repository=assertion.repository" \
  --attribute-condition="assertion.repository=='YOUR_ORG/YOUR_REPO'" \
  --issuer-uri="https://token.actions.githubusercontent.com" \
  --project=PROJECT_ID
```

**4. Allow YOUR repo to impersonate the service account** (the lock):

```bash
gcloud iam service-accounts add-iam-policy-binding github-actions-sa@PROJECT_ID.iam.gserviceaccount.com \
  --role="roles/iam.workloadIdentityUser" \
  --member="principalSet://iam.googleapis.com/projects/PROJECT_NUMBER/locations/global/workloadIdentityPools/github-pool/attribute.repository/YOUR_ORG/YOUR_REPO" \
  --project=PROJECT_ID
```

> This last step needs permission to **set policy on a service account**
> (`roles/iam.serviceAccountAdmin`). If it fails with
> `IAM_PERMISSION_DENIED`, grant yourself that role first, then retry:
>
> ```bash
> gcloud projects add-iam-policy-binding PROJECT_ID \
>   --role=roles/iam.serviceAccountAdmin --member=user:YOU@EMAIL
> ```

---

## Step 3 — GitHub: the three secrets

In the repo: **Settings → Secrets and variables → Actions → New repository
secret** (or the `gh` CLI):

| Secret | Value |
|---|---|
| `GKE_PROJECT` | `dwk-gke-506208` |
| `SERVICE_ACCOUNT` | `github-actions-sa@dwk-gke-506208.iam.gserviceaccount.com` |
| `WORKLOAD_IDENTITY_PROVIDER` | `projects/323959491379/locations/global/workloadIdentityPools/github-pool/providers/github-provider` |

```bash
gh secret set GKE_PROJECT --repo tripplen23/KubernetesSubmissions
# type the value: dwk-gke-506208
gh secret set SERVICE_ACCOUNT --repo tripplen23/KubernetesSubmissions
# type the value: github-actions-sa@dwk-gke-506208.iam.gserviceaccount.com
gh secret set WORKLOAD_IDENTITY_PROVIDER --repo tripplen23/KubernetesSubmissions
# type the value: projects/323959491379/locations/global/workloadIdentityPools/github-pool/providers/github-provider
```

> ⚠️ `gh secret set` reads the VALUE from **stdin** (or `--body`) — it does
> NOT take the name; if you `echo "GKE_PROJECT" | gh secret set
> GKE_PROJECT`, you store the literal placeholder (we hit this during
> testing — auth failed with "Invalid value for audience").

---

## Step 4 — hand-type the Dockerfiles (3 files)

Same as 3.5 — unchanged. Create them in the provided source folders.

### `todo-app/Dockerfile`

```dockerfile
FROM rust:1.85-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/todo-app /usr/local/bin/todo-app
EXPOSE 3000
CMD ["/usr/local/bin/todo-app"]
```

### `todo-backend/Dockerfile`

```dockerfile
FROM rust:1.85-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/todo-backend /usr/local/bin/todo-backend
EXPOSE 3000
CMD ["/usr/local/bin/todo-backend"]
```

### `todo-cron/Dockerfile`

```dockerfile
FROM alpine:3.19
RUN apk add --no-cache curl
WORKDIR /app
COPY generate-todo.sh /generate-todo.sh
RUN chmod +x /generate-todo.sh
CMD ["/generate-todo.sh"]
```

---

## Step 5 — hand-type the manifests (9 files + kustomization)

Create the folder `manifests/` inside `part3/3.6/`. 3 changes vs 3.5:
1. **images become placeholders** (`TODO_APP`, `TODO_BACKEND`,
   `TODO_CRON`) — the pipeline injects the real image via `kustomize edit
   set image`,
2. **todo-app Deployment → `Recreate` strategy** (RWO PVC — see §5 above),
3. **`kustomization.yaml` gets an `images:` section**.

### `manifests/secret.yaml`

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: postgres-secret
  namespace: project
type: Opaque
data:
  POSTGRES_PASSWORD: ZXhhbXBsZQ==
```

### `manifests/configmap.yaml`

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: postgres-config
  namespace: project
data:
  POSTGRES_USER: postgres
  POSTGRES_HOST: postgres-svc
  POSTGRES_PORT: "5432"
  POSTGRES_DB: postgres
```

### `manifests/configmap-todo.yaml`

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: todo-config
  namespace: project
data:
  TODO_BACKEND_URL: http://todo-backend-svc:2345
  IMAGE_URL: https://picsum.photos/1200
  IMAGE_PATH: /usr/src/app/files/image.jpg
  MAX_AGE_SECS: "600"
```

### `manifests/persistentvolumeclaim.yaml`

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: image-claim
  namespace: project
spec:
  storageClassName: standard
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 1Gi
```

### `manifests/postgres.yaml` (Service + StatefulSet in one file)

```yaml
apiVersion: v1
kind: Service
metadata:
  name: postgres-svc
  namespace: project
  labels:
    app: postgres
spec:
  ports:
    - port: 5432
      name: web
  clusterIP: None
  selector:
    app: postgres
---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: postgres-ss
  namespace: project
spec:
  serviceName: postgres-svc
  replicas: 1
  selector:
    matchLabels:
      app: postgres
  template:
    metadata:
      labels:
        app: postgres
    spec:
      containers:
        - name: postgres
          image: gcr.io/dwk-gke-506208/postgres:16
          ports:
            - name: web
              containerPort: 5432
          env:
            - name: POSTGRES_PASSWORD
              valueFrom:
                secretKeyRef:
                  name: postgres-secret
                  key: POSTGRES_PASSWORD
            # mount at the PARENT dir; data lives in a subdir so initdb
            # never sees the filesystem's lost+found
            - name: PGDATA
              value: /var/lib/postgresql/data
          volumeMounts:
            - name: data
              mountPath: /var/lib/postgresql
  volumeClaimTemplates:
    - metadata:
        name: data
      spec:
        accessModes: ["ReadWriteOnce"]
        storageClassName: standard
        resources:
          requests:
            storage: 100Mi
```

### `manifests/deployment-todo-backend.yaml`

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: todo-backend
  namespace: project
  labels:
    app: todo-backend
spec:
  replicas: 1
  selector:
    matchLabels:
      app: todo-backend
  template:
    metadata:
      labels:
        app: todo-backend
    spec:
      containers:
        - name: todo-backend
          image: TODO_BACKEND
          imagePullPolicy: Always
          ports:
            - containerPort: 3000
          env:
            - name: PORT
              value: "3000"
            - name: POSTGRES_USER
              valueFrom:
                configMapKeyRef:
                  name: postgres-config
                  key: POSTGRES_USER
            - name: POSTGRES_HOST
              valueFrom:
                configMapKeyRef:
                  name: postgres-config
                  key: POSTGRES_HOST
            - name: POSTGRES_PORT
              valueFrom:
                configMapKeyRef:
                  name: postgres-config
                  key: POSTGRES_PORT
            - name: POSTGRES_DB
              valueFrom:
                configMapKeyRef:
                  name: postgres-config
                  key: POSTGRES_DB
            - name: POSTGRES_PASSWORD
              valueFrom:
                secretKeyRef:
                  name: postgres-secret
                  key: POSTGRES_PASSWORD
```

### `manifests/deployment-todo-app.yaml` (note the `Recreate` strategy!)

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: todo-app
  namespace: project
  labels:
    app: todo-app
spec:
  strategy:
    type: Recreate
  replicas: 1
  selector:
    matchLabels:
      app: todo-app
  template:
    metadata:
      labels:
        app: todo-app
    spec:
      volumes:
        - name: image-storage
          persistentVolumeClaim:
            claimName: image-claim
      containers:
        - name: todo-app
          image: TODO_APP
          imagePullPolicy: Always
          ports:
            - containerPort: 3000
          envFrom:
            - configMapRef:
                name: todo-config
          env:
            - name: PORT
              value: "3000"
          volumeMounts:
            - name: image-storage
              mountPath: /usr/src/app/files
```

### `manifests/service.yaml` (both Services in one file)

```yaml
apiVersion: v1
kind: Service
metadata:
  name: todo-app-svc
  namespace: project
  labels:
    app: todo-app
spec:
  type: ClusterIP
  selector:
    app: todo-app
  ports:
    - name: http
      port: 3000
      targetPort: 3000
      protocol: TCP
---
apiVersion: v1
kind: Service
metadata:
  name: todo-backend-svc
  namespace: project
  labels:
    app: todo-backend
spec:
  type: ClusterIP
  selector:
    app: todo-backend
  ports:
    - name: http
      port: 2345
      targetPort: 3000
      protocol: TCP
```

### `manifests/cronjob.yaml`

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: todo-cron
  namespace: project
spec:
  schedule: "0 * * * *"
  jobTemplate:
    spec:
      template:
        spec:
          restartPolicy: OnFailure
          containers:
            - name: todo-cron
              image: TODO_CRON
              imagePullPolicy: Always
              env:
                - name: TODO_BACKEND_URL
                  value: "http://todo-backend-svc:2345"
```

### `manifests/kustomization.yaml` — resource order MATTERS (deps first)

```yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - secret.yaml
  - configmap.yaml
  - configmap-todo.yaml
  - persistentvolumeclaim.yaml
  - postgres.yaml
  - deployment-todo-backend.yaml
  - deployment-todo-app.yaml
  - service.yaml
  - cronjob.yaml
images:
  - name: TODO_APP
    newName: gcr.io/dwk-gke-506208/todo-app
    newTag: "3.6"
  - name: TODO_BACKEND
    newName: gcr.io/dwk-gke-506208/todo-backend
    newTag: "3.6"
  - name: TODO_CRON
    newName: gcr.io/dwk-gke-506208/todo-cron
    newTag: "3.6"
```

> ⚠️ **Quote the tag**: `newTag: "3.6"` (not `3.6`). YAML reads an
> unquoted `3.6` as a *number*, and kustomize rejects it:
> `cannot unmarshal number into Image.images.newTag of type string`
> (we hit this during testing!). `newName`/`newTag` are the *defaults* for
> manual deploys; the pipeline overrides them with `kustomize edit set
> image`.

---

## Step 6 — the GitHub Actions workflow

Create **`.github/workflows/main.yaml`** at the repository **root** (next
to `part1/`, `part2/`, ... — NOT inside `part3/3.6/`):

```yaml
name: Release application

on:
  push:

env:
  PROJECT_ID: ${{ secrets.GKE_PROJECT }}
  GKE_CLUSTER: dwk-cluster
  GKE_ZONE: europe-north1-c
  REGISTRY: europe-north1-docker.pkg.dev
  REPOSITORY: my-repository
  BRANCH: ${{ github.ref_name }}

jobs:
  build-publish-deploy:
    name: Build, Publish and Deploy
    runs-on: ubuntu-latest
    permissions:
      id-token: write
      contents: read

    steps:
      - name: Checkout
        uses: actions/checkout@v6

      - uses: google-github-actions/auth@v3
        with:
          workload_identity_provider: '${{ secrets.WORKLOAD_IDENTITY_PROVIDER }}'
          service_account: '${{ secrets.SERVICE_ACCOUNT }}'

      - name: 'Set up Cloud SDK'
        uses: google-github-actions/setup-gcloud@v3

      - name: 'Use gcloud CLI'
        run: gcloud info

      - name: 'Get GKE credentials'
        uses: 'google-github-actions/get-gke-credentials@v3'
        with:
          cluster_name: '${{ env.GKE_CLUSTER }}'
          project_id: '${{ env.PROJECT_ID }}'
          location: '${{ env.GKE_ZONE }}'

      - name: 'Configure Docker to push to Artifact Registry'
        run: gcloud --quiet auth configure-docker $REGISTRY

      - name: 'Form image names'
        run: |
          echo "IMAGE_TAG_APP=$REGISTRY/$PROJECT_ID/$REPOSITORY/todo-app:$BRANCH-$GITHUB_SHA" >> $GITHUB_ENV
          echo "IMAGE_TAG_BACKEND=$REGISTRY/$PROJECT_ID/$REPOSITORY/todo-backend:$BRANCH-$GITHUB_SHA" >> $GITHUB_ENV
          echo "IMAGE_TAG_CRON=$REGISTRY/$PROJECT_ID/$REPOSITORY/todo-cron:$BRANCH-$GITHUB_SHA" >> $GITHUB_ENV

      - name: 'Build todo-app'
        run: docker build --tag $IMAGE_TAG_APP part3/3.6/todo-app

      - name: 'Build todo-backend'
        run: docker build --tag $IMAGE_TAG_BACKEND part3/3.6/todo-backend

      - name: 'Build todo-cron'
        run: docker build --tag $IMAGE_TAG_CRON part3/3.6/todo-cron

      - name: 'Publish images'
        run: |
          docker push $IMAGE_TAG_APP
          docker push $IMAGE_TAG_BACKEND
          docker push $IMAGE_TAG_CRON

      - name: 'Set up Kustomize'
        uses: imranismail/setup-kustomize@v3

      - name: 'Deploy with Kustomize'
        run: |
          cd part3/3.6/manifests
          kustomize edit set image TODO_APP=$IMAGE_TAG_APP
          kustomize edit set image TODO_BACKEND=$IMAGE_TAG_BACKEND
          kustomize edit set image TODO_CRON=$IMAGE_TAG_CRON
          kustomize build . | kubectl apply -f -
          kubectl rollout status deployment todo-app -n project
          kubectl rollout status deployment todo-backend -n project
          kubectl rollout status statefulset postgres-ss -n project
          kubectl get pods -n project -o wide
```

Read it in 4 chunks:
1. **trigger + env** — `on: push`, the constants, `BRANCH=github.ref_name`.
2. **auth block** — checkout, `auth@v3` (OIDC via the two secrets),
   `setup-gcloud`, `get-gke-credentials` (kubectl context for the runner),
   `auth configure-docker $REGISTRY` (docker push auth).
3. **build + publish** — "Form image names" writes the three tags into
   **$GITHUB_ENV** (they persist across steps), then `docker build --tag`
   each app + `docker push`.
4. **deploy** — the kustomize action installs `kustomize`; `edit set
   image` rewrites the `images:` entries in `kustomization.yaml` to the
   fresh `main-<sha>` tags; `kustomize build . | kubectl apply -f -`
   deploys; `rollout status` waits until each workload is up.

---

## Step 7 — kick it: commit + push → the pipeline deploys

```bash
git add -A
git commit -m "ci: add deployment pipeline + 3.6 project on GKE"
git push
```

GitHub now runs `Release application` (watch the **Actions** tab):

```bash
gh run list --limit 3
gh run watch            # live
gh run view <ID> --log-failed   # when it fails (see errors table)
```

A green `✓` means the whole chain worked: auth → build → push → deploy →
rollout.

---

## Step 8 — verify the deployment (proves the pipeline deployed it)

```bash
kubectl rollout status deploy/todo-app -n project          # "successfully rolled out"
kubectl rollout status deploy/todo-backend -n project
kubectl get pods -n project -o wide        # 3× Running — postgres-ss-0, todo-app, todo-backend

# the image really comes from Artifact Registry, tagged main-<sha>:
kubectl get deploy todo-app -n project \
  -o jsonpath='{.spec.template.spec.containers[0].image}'
# → europe-north1-docker.pkg.dev/dwk-gke-506208/my-repository/todo-app:main-<sha>
```

And the app still works (deployed by the pipeline — no human touch):

```bash
kubectl port-forward -n project svc/todo-app-svc 8081:3000 &
LONG=$(python3 -c "print('x'*141)")
curl -s -o /dev/null -w "GET / -> %{http_code}\n" http://localhost:8081/                 # 200
curl -s -o /dev/null -w "POST 141 -> %{http_code}\n" -X POST http://localhost:8081/todos --data "content=$LONG"          # 400
curl -s -o /dev/null -w "POST ok -> %{http_code}\n" -X POST http://localhost:8081/todos --data "content=deployed-by-pipeline"  # 303
```

---

## Step 9 — clean up (credits!)

```bash
gcloud container clusters delete dwk-cluster --zone=europe-north1-c --project=dwk-gke-506208

# images live in Artifact Registry now (not gcr) — delete the whole repo:
gcloud artifacts repositories delete my-repository \
  --location=europe-north1 --project=dwk-gke-506208 --async

# the postgres base image pushed to legacy gcr:
gcloud container images delete gcr.io/dwk-gke-506208/postgres:16 --quiet --force-delete-tags

# leftover PVC disks (they SURVIVE cluster deletion):
gcloud compute disks list --project=dwk-gke-506208     # any pvc-* → delete:
gcloud compute disks delete pvc-... --project=dwk-gke-506208 --zone=europe-north1-c

# the OIDC chain (no longer needed after the lab):
gcloud iam workload-identity-pools delete github-pool --location=global --project=dwk-gke-506208
gcloud iam service-accounts delete github-actions-sa@dwk-gke-506208.iam.gserviceaccount.com --quiet --project=dwk-gke-506208
gh secret delete GKE_PROJECT SERVICE_ACCOUNT WORKLOAD_IDENTITY_PROVIDER --repo tripplen23/KubernetesSubmissions
```

Final check — clusters / instances / forwarding-rules / addresses / disks
all = 0:

```bash
gcloud container clusters list --project=dwk-gke-506208
gcloud compute disks list --project=dwk-gke-506208
```

---

## Common errors / gotchas (all hit during this lab's testing!)

| Symptom | Cause | Fix |
|---|---|---|
| auth@v3: "did not inject $ACTIONS_ID_TOKEN_REQUEST_TOKEN" | missing OIDC permission on the job | add `permissions: id-token: write, contents: read` on the job |
| auth@v3: "Invalid value for audience" | wrong `WORKLOAD_IDENTITY_PROVIDER` secret (e.g. literal placeholder) | set it to `projects/<N>/locations/global/workloadIdentityPools/github-pool/providers/github-provider` |
| `gcloud iam service-accounts add-iam-policy-binding`: IAM_PERMISSION_DENIED | you lack service-account policy permission | grant yourself `roles/iam.serviceAccountAdmin` then retry |
| rollout hangs / new pod Pending after deploy | RWO PVC + RollingUpdate (todo-app) | `strategy: type: Recreate` on the deployment |
| kustomize: `cannot unmarshal number into Image.images.newTag` | unquoted `newTag: 3.6` parsed as float | `newTag: "3.6"` |
| run failed in ~11s | early step (checkout/auth/gcloud) error | `gh run view --log-failed` |
| `gh secret set` stored the literal word | piped the NAME, not the VALUE | give the value via stdin/`--body` |
| IAM_PERMISSION_DENIED deleting AR repo | repo has images | add `--async` (or delete the images first) |
| `gcloud compute disks delete`: disk in use | PVC disk still attached to node | wait for cluster deletion first |
| `gcloud container clusters create`: operation RUNNING forever, 0 instances created, zone has no capacity | **GCE_STOCKOUT** — zone ran out of e2-small capacity | zone has too little capacity (regional stockout). **Probe another zone** first (`gcloud compute instances create probe-x --zone=europe-north1-c --machine-type=e2-small --no-address --project=dwk-gke-506208`), then create there. Any new cluster needs its own master CIDR (`172.16.10.0/28` this one) — the old cluster's `172.16.0.0/28` is still registered in the region until the stuck operation finishes. |
| `gcloud container clusters delete`: "Cluster is running incompatible operation" | GKE CREATE operation stuck retrying stockout — **cannot be cancelled** (`Operation type CREATE_CLUSTER cannot be cancelled`) | wait it out (GKE gives up after a while, op becomes DONE+cluster ERROR), then `gcloud container clusters delete`. Never stack a second CREATE on the same name in another zone — it'll error `Already exists` |
| `workload-identity-pools create`: ALREADY_EXISTS / `providers create-oidc`: NOT_FOUND, while nothing is listed | the pool was **soft-deleted** (shows `DELETED` + expireTime, name reserved ~30 days); provider create can't find a deleted pool | resurrect it: `gcloud iam workload-identity-pools undelete github-pool --location=global --project=dwk-gke-506208` (restores the pool AND its provider with config intact) — verified 2026-09-08 |

## P/S:

1. **CI/CD = build → publish → deploy, triggered by a push.** The moment
   you `git push`, production updates.
2. **OIDC chain of trust (5 pieces)**: SA → roles → identity pool → OIDC
   provider with repo condition → impersonation binding. **No stored
   secrets.**
3. `permissions: id-token: write` is the unlock for OIDC from Actions.
4. Image tag `main-<sha>` = unique per push → deployments always run a
   fresh image.
5. **RollingUpdate vs RWO PVC** → `Recreate` on the volume-carrying
   deployment; StatefulSets are safe by design.
6. The whole project (app + backend + db + cron) now ships to production
   with one workflow.
7. Clean up cluster, AR repo, images, leftover disks — every byte of
   borrowed cloud costs.