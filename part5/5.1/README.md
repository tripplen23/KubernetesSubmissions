# 5.1 — DIY CRD & Controller: DummySite

The exercise sits at the bottom of Chapter 6's *Custom Resource Definitions* page. It asks for a `DummySite` resource
with a string property `website_url`, plus a controller that receives created DummySite objects from the API and
creates the resources the functionality needs. A DummySite with `website_url: https://example.com/` has to produce a
copy of that website. The page fixes the workflow that must succeed (*apply role, account and binding, apply
deployment, apply DummySite*) and leaves the technology to you. It does not depend on the earlier exercises.

The material's example is a `Countdown` resource whose controller schedules Jobs, written in JavaScript
(`kubernetes-hy/material-example/app10`) and in Go (`app10-go`); the page says Go is the better option. This lab
takes that advice and splits the work by language: **the controller is Go** (`client-go`), because that is what the
CRD exercise is about, and **the server is Rust** (`axum` + `reqwest`), because the rest of the project is Rust.
Both are practiced, neither is fighting the material.

Three pieces make the exercise work:

- a **CRD** teaches the API server a new kind, so `DummySite` objects can be stored at all;
- a **server** that fetches one URL and serves the copy — the thing a site ends up needing;
- a **controller** that watches DummySites and creates a Deployment and Service for each;

The controller is where the exercise is decided, and the idea is small: watch a resource and, for every
object that appears, make the cluster match what it asks for. In Go that is an informer, a handler and two
API calls, with no HTTP plumbing and no generated code.

## Step 0 — the cluster

Everything below runs against the local **k3d** cluster `mycluster` (context `k3d-mycluster`). The commands are bare
`kubectl`, so they act on whichever context is current, and after Part 4 that is easily the GKE cluster, whose
kubeconfig namespace is `project`. Switch once:

```bash
kubectl config use-context k3d-mycluster
kubectl get nodes
```

Or prefix each command with `--context k3d-mycluster` instead of switching. Otherwise the namespaced objects (the
ServiceAccount in Step 3, the Deployment in Step 4) go to `project`, which does not exist on that cluster, and Step 3
stops with `namespaces "project" not found`. The ClusterRole and ClusterRoleBinding report `unchanged`, being
cluster-scoped and already there.

## Step 1 — the resource: a CustomResourceDefinition

A CRD describes a new kind of object. Until it exists the API server has nowhere to put a `DummySite`, and
`kubectl apply -f dummysite-example.yaml` fails with `no matches for kind "DummySite" in version "stable.dwk/v1"`.
Once it exists, the API serves `/apis/stable.dwk/v1/dummysites`, the path the controller watches.

Three things in the file matter: `group: stable.dwk` plus the names (`kind: DummySite`, `plural: dummysites`) decide
the URL and the kind; `scope: Namespaced` means a DummySite lives in a namespace like a Pod; and `versions[0].schema`
is an OpenAPI schema, so the API server validates your objects: a `DummySite` without `website_url` is rejected,
not stored broken. `additionalPrinterColumns` adds the `WEBSITE` column to `kubectl get`.

`manifests/dummysite-crd.yaml`

```yaml
apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: dummysites.stable.dwk
spec:
  group: stable.dwk
  scope: Namespaced
  names:
    kind: DummySite
    plural: dummysites
    singular: dummysite
    # "ds" looks like the obvious alias, but it is already DaemonSet's shortname and
    # the built-in wins: `kubectl get ds` lists daemonsets and `kubectl delete ds/x`
    # deletes a daemonset. Check `kubectl api-resources` before choosing one.
    shortNames:
      - dummy
  versions:
    - name: v1
      served: true
      storage: true
      schema:
        openAPIV3Schema:
          type: object
          properties:
            spec:
              type: object
              properties:
                website_url:
                  type: string
              required:
                - website_url
      additionalPrinterColumns:
        - name: Website
          type: string
          description: The URL this DummySite copies
          jsonPath: .spec.website_url
```

```bash
kubectl apply -f manifests/dummysite-crd.yaml
```

```
customresourcedefinition.apiextensions.k8s.io/dummysites.stable.dwk created
```

```bash
kubectl get crd dummysites.stable.dwk
```

```
NAME                    CREATED AT
dummysites.stable.dwk   2026-09-22T18:51:06Z
```

## Step 2 — build both images and hand them to the cluster

From `part5/5.1`. Each `docker build` takes its **context** from the folder you name and reads `Dockerfile` from
inside it, so the two files must sit where the Step 0 listing shows them:

```bash
docker build -t dummysite-server:5.1 dummysite-server
docker build -t dummysite-controller:5.1 dummysite-controller
k3d image import dummysite-server:5.1 dummysite-controller:5.1 -c mycluster
```

Both images are compiled in their builder stages, so the first build is the slow one: the Rust server builds its
own tree (`axum`, `reqwest`), the Go controller downloads and compiles `client-go`. Later builds reuse Docker's layer
cache as long as the dependency files (`Cargo.toml`/`Cargo.lock`, `go.mod`/`go.sum`) stay unchanged.

```bash
docker images | grep dummysite
```

```
dummysite-controller:5.1   de3401dd8653   212MB
dummysite-server:5.1       b45e5bbcb669   135MB
```

```bash
k3d image import dummysite-server:5.1 dummysite-controller:5.1 -c mycluster
```

```
INFO[0008] Successfully imported 2 image(s) into 1 cluster(s)
```

## Step 3 — who the controller is, and what it may do

The controller runs with a ServiceAccount, and RBAC decides what it can touch. Two choices follow from how it is written:

- a **ClusterRole** and **ClusterRoleBinding**, not a Role and RoleBinding, because the controller watches DummySites in every namespace (the informer uses `metav1.NamespaceAll`) while running in `default`; a RoleBinding would restrict it to one namespace;
- the rules are deliberately thin: `get`, `list`, `watch` on dummysites, `create` on deployments and services, nothing else. No `delete` verb; the garbage collector deletes through the ownerReferences.

`manifests/dummysite-rbac.yaml`

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: dummysite-controller-account
---
kind: ClusterRole
apiVersion: rbac.authorization.k8s.io/v1
metadata:
  name: dummysite-controller-role
rules:
  # The controller watches DummySite objects and creates the workload, nothing else.
  # Deleting is not here on purpose: the created objects carry an ownerReference, so
  # Kubernetes' garbage collector removes them and the controller needs no delete verb.
  - apiGroups: ["stable.dwk"]
    resources: ["dummysites"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["apps"]
    resources: ["deployments"]
    verbs: ["create"]
  - apiGroups: [""]
    resources: ["services"]
    verbs: ["create"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: dummysite-controller-rolebinding
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: dummysite-controller-role
subjects:
  - kind: ServiceAccount
    name: dummysite-controller-account
    namespace: default
```

```bash
kubectl config use-context k3d-mycluster
kubectl apply -f manifests/dummysite-rbac.yaml
```

```
serviceaccount/dummysite-controller-account created
clusterrole.rbac.authorization.k8s.io/dummysite-controller-role created
clusterrolebinding.rbac.authorization.k8s.io/dummysite-controller-rolebinding created
```

## Step 4 — the controller Deployment

The Deployment is ordinary apart from `serviceAccountName`, which gives the pod the Step 3 account.
`SERVER_IMAGE` is an environment variable so the image it creates is set in one place.

`manifests/dummysite-controller.yaml`

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dummysite-controller-dep
  namespace: default
spec:
  replicas: 1
  selector:
    matchLabels:
      app: dummysite-controller
  template:
    metadata:
      labels:
        app: dummysite-controller
    spec:
      serviceAccountName: dummysite-controller-account
      containers:
        - name: dummysite-controller
          image: dummysite-controller:5.1
          env:
            - name: SERVER_IMAGE
              value: dummysite-server:5.1
```

```bash
kubectl apply -f manifests/dummysite-controller.yaml
kubectl rollout status deploy/dummysite-controller-dep --timeout=120s
```

```
deployment.apps/dummysite-controller-dep created
Waiting for deployment "dummysite-controller-dep" rollout to finish: 0 out of 1 new replicas have been updated...
Waiting for deployment "dummysite-controller-dep" rollout to finish: 0 of 1 updated replicas are available...
deployment "dummysite-controller-dep" successfully rolled out
```

While you are here, pin down **which** image is running. A tag can point at an older build than the source you changed: a failed `docker build` leaves the previous image in place, and it behaves identically, so the
tag alone proves nothing:

```bash
kubectl get pod -l app=dummysite-controller \
  -o jsonpath='{.items[0].spec.containers[0].image}{"  "}{.items[0].status.containerStatuses[0].imageID}{"\n"}'
```

```
dummysite-controller:5.1  sha256:330c60719b52298e32392215bf1f36969a16f941fc8fe9c4c79638d7a778b424
```

That digest is the local image's `docker image inspect` id; if it is not, the cluster runs something older than you built.

## Step 5 — a DummySite, and the controller reacting

`manifests/dummysite-example.yaml`

```yaml
apiVersion: stable.dwk/v1
kind: DummySite
metadata:
  name: example
spec:
  website_url: https://example.com/
```

```bash
kubectl apply -f manifests/dummysite-example.yaml
kubectl get dummy
kubectl get dummysites -o wide
```

```
dummysite.stable.dwk/example created

NAME      WEBSITE
example   https://example.com/
NAME      WEBSITE
example   https://example.com/
```

`WEBSITE` comes from `additionalPrinterColumns` in the CRD, and `get dummy` from the short name.
Now the controller's log for the same moment, the reconciliation:

```bash
kubectl logs -l app=dummysite-controller --tail=5
```

```
watching dummysites.stable.dwk for DummySite objects
[add] default/example -> https://example.com/
created deployment example-dep
created service example-svc
```

And the objects that no file here contains:

```bash
kubectl get deploy,svc
```

```
NAME                                       READY   UP-TO-DATE   AVAILABLE   AGE
deployment.apps/dummysite-controller-dep   1/1     1            1           21s
deployment.apps/example-dep                1/1     1            1           11s

NAME                  TYPE        CLUSTER-IP     EXTERNAL-IP   PORT(S)   AGE
service/example-svc   ClusterIP   10.43.67.150   <none>        80/TCP    12s
service/kubernetes    ClusterIP   10.43.0.1      <none>        443/TCP   54d
```

## Step 6 — proof that the copy is really served

Three checks, because "it works" is a claim and each pins a different part of it. First, the owned objects
name their owner, which makes Step 7 work:

```bash
kubectl get deploy example-dep -o jsonpath='{.metadata.ownerReferences[0].kind}/{.metadata.ownerReferences[0].name} blockOwnerDeletion={.metadata.ownerReferences[0].blockOwnerDeletion}{"\n"}'
```

```
DummySite/example blockOwnerDeletion=true
```

Then the server's own log, showing the fetch and the copy's size:

```bash
kubectl logs -l app=dummysite-example --tail=3
```

```
dummysite-server for https://example.com/ listening on port 3000
fetched https://example.com/ -> 559 characters
```

Finally the page itself, asked for **from inside the cluster** so the request goes through the Service by name. That proves the Service and its DNS entry, not just a running pod:

```bash
kubectl run curl-test --rm -i --restart=Never --image=curlimages/curl:8.10.1 -- -sS --max-time 15 http://example-svc | head -3
```

```
<!doctype html><html lang="en"><head><title>Example Domain</title><link rel="icon" href="data:,"><meta name="viewport" content="width=device-width, initial-scale=1"><style>body{background:#eee;width:60vw;margin:15vh auto;font-family:system-ui,sans-serif}h1{font-size:1.5em}div{opacity:0.8}a:link,a:visited{color:#348}</style></head><body><div><h1>Example Domain</h1><p>This domain is for use in documentation examples without needing permission. Avoid use in operations.</p><p><a href="https://iana.org/domains/example">Learn more</a></p></div></body></html>
```

That is the acceptance test: a DummySite with `https://example.com/` produced a copy of that page, served by resources the controller created. A more complex site comes out imperfect, and the page allows that (Wikipedia's CSS breaks, as it says).

To look at the page in your own browser, forward a local port to the Service: `example-svc` is a `ClusterIP`, so it
has no address outside the cluster and this lab sets up no ingress:

```bash
kubectl port-forward svc/example-svc 8080:80
```

Then open **http://localhost:8080** (leave it running; `Ctrl-C` when you are done). The name
`http://example-svc` resolves only inside the cluster, so it will not work in your browser.

![alt text](image.png)

## Step 7 — delete it, and watch Kubernetes clean up

Delete the resource, not its children, and use a name that cannot be taken for a daemonset:

```bash
kubectl delete dummysites/example
kubectl get deploy,svc
```

```
dummysite.stable.dwk "example" deleted from default namespace
NAME                                       READY   UP-TO-DATE   AVAILABLE   AGE
deployment.apps/dummysite-controller-dep   1/1     1            1           33s

NAME                 TYPE        CLUSTER-IP   EXTERNAL-IP   PORT(S)   AGE
service/kubernetes   ClusterIP   10.43.0.1    <none>        443/TCP   54d
```

`example-dep` and `example-svc` are gone, and the controller never had a delete verb or a line of cleanup code. Its
log records the event; the removal is the garbage collector following the `ownerReference`:

```bash
kubectl logs -l app=dummysite-controller --tail=2
```

```
[delete] default/example — its Deployment and Service follow it out (ownerReferences)
```

## P.S.

- A CRD is only a data shape. Everything interesting (creating, watching, deleting) is the controller, and the
  controller is a loop: *what does this object ask for, and what is missing?*
- Prefer ownerReferences over cleanup code. The garbage collector is already running; your delete logic is what
  will be wrong after a restart.
- RBAC is part of the design: the verbs you grant are the actions your controller can take, so thin rules (and no
  `delete`) say something about how it works, not a formality.
- Take the material's advice where it points at a language: Go for the CRD controller (`client-go` follows from the
  page's recommendation), the project's Rust everywhere else. The same three ideas
  (watch, reconcile, own) show up in both.
- Verify the claim, not the symptom: a running pod is not a served page, which is why Step 6 fetches through
  the Service.