# 5.7 — Deploy to serverless

The exercise: make the **Ping pong** service of the Log Output app run serverless.

Log Output stays a Deployment with a writer and a reader over a shared `emptyDir`; Ping pong becomes a **Knative
Service**. Knative then owns its Deployment, Service, ReplicaSet and pod, starts a pod when someone calls it, and
stops it when nobody does.

Platform: Knative Serving **v1.23.0** on Kubernetes **v1.34.1** (k3d).

## Step 0 — the platform

This is exercise 5.6's territory: the cluster and Knative Serving. If the `knative` cluster from 5.6 already has Serving installed, skip to Step 1. Otherwise, the short version (details, and the trap in installing the core, are in the 5.6 lab):

```bash
$ k3d cluster create knative --port 8082:30080@agent:0 -p 8081:80@loadbalancer --agents 2 --k3s-arg "--disable=traefik@server:0" --image rancher/k3s:v1.34.1-k3s1
$ kubectl apply -f https://github.com/knative/serving/releases/download/knative-v1.23.0/serving-crds.yaml
$ kubectl apply -f https://github.com/knative/serving/releases/download/knative-v1.23.0/serving-core.yaml
$ kubectl apply -f https://github.com/knative/net-kourier/releases/download/knative-v1.23.0/kourier.yaml
$ kubectl patch configmap/config-network -n knative-serving --type merge --patch '{"data":{"ingress-class":"kourier.ingress.networking.knative.dev"}}'
$ kubectl apply -f https://github.com/knative/serving/releases/download/knative-v1.23.0/serving-default-domain.yaml
$ kubectl -n knative-serving logs job/default-domain --tail=1
```

Apply `serving-core.yaml` again if the first leaves pods crashing: the CRDs need a moment to be
published, and the core asks for one while applying.

The `ingress-class` value must be the **full** `kourier.ingress.networking.knative.dev`, not `kourier`. The short form leaves the Kourier controller silent, logging no error, and `kubectl get ksvc` sits at `READY Unknown / IngressNotConfigured` while the app is healthy. The end of this file has the receipts.

The `default-domain` job is the guide's Magic DNS option: it writes the cluster's ingress IP into
`config-domain`, the source of the `sslip.io` in every URL below. Its last log line says what it did:

```console
$ kubectl -n knative-serving logs job/default-domain --tail=1
{"level":"info","ts":1790338181.502145,"logger":"fallback.default-domain","caller":"default-domain/main.go:238","msg":"Updated default domain to: 172.21.0.3.sslip.io"}
```

## Step 1 — two images

Both applications keep the runtime contract Knative wants: they read `PORT` and log to stdout, and the port is
the only thing they take from the environment. Build and hand both images to the cluster:

```bash
$ cd part5/5.7
$ docker build -t dev.local/ping-pong:5.7 ping-pong/
$ docker build -t dev.local/log-output:5.7 log-output/
$ k3d image import dev.local/ping-pong:5.7 dev.local/log-output:5.7 -c knative
```

```console
$ docker images --format '{{.Repository}}:{{.Tag}}  {{.ID}}  {{.Size}}' | grep dev.local
dev.local/ping-pong:5.7  e9bbc923bf0f  130MB
dev.local/log-output:5.7  c7500c588514  135MB

$ k3d image import dev.local/ping-pong:5.7 dev.local/log-output:5.7 -c knative
INFO[0013] Successfully imported image(s)
INFO[0013] Successfully imported 2 image(s) into 1 cluster(s)
```

The Dockerfile is the same for both, and it is why `Cargo.lock` matters:

```dockerfile
FROM rust:1.85-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/ping-pong /usr/local/bin/ping-pong
EXPOSE 3000
CMD ["/usr/local/bin/ping-pong"]
```

`COPY Cargo.lock*` does not fail on a missing lock file: it copies nothing, and cargo resolves dependencies from scratch, which can pick versions the pinned compiler cannot build. Both applications ship their lock files,
so the builds above succeeded; the block below is the first attempt before `log-output/Cargo.lock` was added.
Reproduce it by moving the lock away:

```console
$ mv log-output/Cargo.lock /tmp/ && docker build -t dev.local/log-output:5.7 log-output/
error: rustc 1.85.1 is not supported by the following packages:
  icu_collections@2.3.0 requires rustc 1.88
  icu_locale_core@2.3.0 requires rustc 1.88
  icu_normalizer@2.3.0 requires rustc 1.88
  ... (twelve package lines in the real output, trimmed here)
Either upgrade rustc or select compatible dependency versions with
`cargo update <name>@<current-ver> --precise <compatible-ver>`

$ mv /tmp/Cargo.lock log-output/Cargo.lock
$ docker build -t dev.local/log-output:5.7 log-output/   # and it builds again
```

The failure is about a dependency you never chose: nothing in the Dockerfile or the `Cargo.toml` says `icu`.

Log Output's Dockerfile is the same file with one word changed: the binary is `log-output`, so both `COPY --from` and `CMD` name it. Everything else (base images, `EXPOSE 3000`, lock line) is identical.

## Step 2 — the namespace, then Log Output

The namespace goes first and on its own. Applying the whole directory at once means the API server may
not have the namespace yet when the next file arrives:

```console
$ kubectl --context k3d-knative apply -f manifests/namespace.yaml
namespace/exercises created

$ kubectl --context k3d-knative apply -f manifests/configmap.yaml -f manifests/log-output.yaml
configmap/log-output-config created
deployment.apps/log-output created
service/log-output-svc created
```

For comparison, the single-command version in the same second:

```console
$ kubectl --context k3d-knative apply -f manifests/
namespace/exercises created
Warning: Kubernetes default value is insecure, Knative may default this to secure in a future release: ...
service.serving.knative.dev/pingpong created
```

Nothing is broken there (re-applying works), but the receipts are from the two-step order.

The namespace is just a namespace this time (no `istio.io/dataplane-mode` label; this cluster has no mesh):

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: exercises
```

The ConfigMap is the information the app has shown since part 2:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: log-output-config
  namespace: exercises
data:
  information.txt: |
    this text is from file
  MESSAGE: hello world
```

And Log Output itself, unchanged except for one line: `PINGS_URL` now names the Knative Service. Its two
containers are still ordinary, in an ordinary Deployment.

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: log-output
  namespace: exercises
  labels:
    app: log-output
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
      volumes:
        - name: shared-logs
          emptyDir: {}
        - name: config-volume
          configMap:
            name: log-output-config
      containers:
        - name: writer
          image: dev.local/log-output:5.7
          imagePullPolicy: IfNotPresent
          env:
            - name: ROLE
              value: "writer"
            - name: FILE_PATH
              value: "/usr/src/app/files/timestamp.txt"
          volumeMounts:
            - name: shared-logs
              mountPath: /usr/src/app/files
        - name: reader
          image: dev.local/log-output:5.7
          imagePullPolicy: IfNotPresent
          ports:
            - containerPort: 3000
          env:
            - name: ROLE
              value: "reader"
            - name: PORT
              value: "3000"
            - name: FILE_PATH
              value: "/usr/src/app/files/timestamp.txt"
            # Ping pong is a Knative Service now, so it is addressed by its
            # fully qualified Service name instead of a plain Service name.
            - name: PINGS_URL
              value: "http://pingpong.exercises.svc.cluster.local/pongs"
            - name: INFO_FILE
              value: "/usr/src/app/config/information.txt"
            - name: MESSAGE
              valueFrom:
                configMapKeyRef:
                  name: log-output-config
                  key: MESSAGE
          volumeMounts:
            - name: shared-logs
              mountPath: /usr/src/app/files
            - name: config-volume
              mountPath: /usr/src/app/config
---
apiVersion: v1
kind: Service
metadata:
  name: log-output-svc
  namespace: exercises
  labels:
    app: log-output
spec:
  type: ClusterIP
  selector:
    app: log-output
  ports:
    - name: http
      port: 3000
      targetPort: 3000
      protocol: TCP
```

```console
$ kubectl --context k3d-knative wait --for=condition=Ready pod -l app=log-output -n exercises --timeout=120s
pod/log-output-5956dc699f-vc75m condition met
```

## Step 3 — Ping pong as a Knative Service

This is the exercise. One object, no Deployment, no Service, no port:

```yaml
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: pingpong
  namespace: exercises
spec:
  template:
    metadata:
      annotations:
        # Back to zero when nothing is calling: the whole point of the exercise.
        autoscaling.knative.dev/minScale: "0"
    spec:
      containers:
        - image: dev.local/ping-pong:5.7
          imagePullPolicy: IfNotPresent
```

```console
$ kubectl --context k3d-knative apply -f manifests/pingpong.yaml
Warning: Kubernetes default value is insecure, Knative may default this to secure in a future release: spec.template.spec.containers[0].securityContext.allowPrivilegeEscalation, spec.template.spec.containers[0].securityContext.capabilities, spec.template.spec.containers[0].securityContext.runAsNonRoot, spec.template.spec.containers[0].securityContext.seccompProfile
service.serving.knative.dev/pingpong created

$ kubectl --context k3d-knative get ksvc -n exercises
NAME       URL                                             LATESTCREATED    LATESTREADY      READY   REASON
pingpong   http://pingpong.exercises.172.21.0.3.sslip.io   pingpong-00001   pingpong-00001   True

$ kubectl --context k3d-knative get pods -n exercises
NAME                                        READY   STATUS    RESTARTS   AGE
log-output-5956dc699f-vc75m                 2/2     Running   0          5s
pingpong-00001-deployment-b698c585b-l6dkg   2/2     Running   0          18s
```

That second pod lasts about a minute. Nothing has called ping-pong, so the autoscaler takes it back to zero on
its own: run the command a little later and you see `log-output` alone, because the receipt was taken seconds
after the apply. That is not a broken step; `kubectl get revisions -n exercises` still lists `pingpong-00001`,
and any request brings a pod back. Step 6 measures exactly this; to see both pods, run the command right after the
apply or wake the revision with the curl in Step 4.

Two things to read there. The app has no `PORT` and no `containerPort`: Knative injects `PORT`
and the app reads it, the runtime contract working. And the pod's two containers: `user-container` is the Rust
binary, `queue-proxy` is Knative's, and the URL it printed is the one the ingress answers.

The `Warning:` is Knative asking the manifest to be explicit about hardening; the values it lists are Kubernetes' defaults, so it is noise.

## Step 4 — Log Output calls it

What Knative created for that one object, and what the FQDN in `PINGS_URL` points at:

```console
$ kubectl --context k3d-knative get deploy,svc,rs -n exercises
NAME                                        READY   UP-TO-DATE   AVAILABLE   AGE
deployment.apps/log-output                  1/1     1            1           5m12s
deployment.apps/pingpong-00001-deployment   0/0     0            0           5m24s

NAME                             TYPE           CLUSTER-IP      EXTERNAL-IP                                         PORT(S)                                     AGE
service/log-output-svc           ClusterIP      10.43.172.243   <none>                                              3000/TCP                                    5m12s
service/pingpong                 ExternalName   <none>          kourier-internal.kourier-system.svc.cluster.local   80/TCP                                      5m13s
service/pingpong-00001           ClusterIP      10.43.153.57    <none>                                              80/TCP,443/TCP                              5m24s
service/pingpong-00001-private   ClusterIP      10.43.145.181   <none>                                              80/TCP,443/TCP,9090/TCP,9091/TCP,8012/TCP   5m24s

NAME                                                  DESIRED   CURRENT   READY   AGE
replicaset.apps/log-output-5956dc699f                 1         1         1       5m12s
replicaset.apps/pingpong-00001-deployment-b698c585b   0         0         0       5m24s
```

Note `service/pingpong`: an **ExternalName** pointing at `kourier-internal` in `kourier-system`.
The name your app resolves is not a pod address any more; it is Knative's ingress. That matters next.

Put a temporary pod in the namespace and call it. It is only a client; nothing depends on its name:

```bash
$ kubectl --context k3d-knative run curl57 -n exercises --restart=Never --image=curlimages/curl --command -- sh -c 'for i in 1 2 3; do curl -s http://pingpong.exercises.svc.cluster.local/pingpong; echo; done; echo "--- /pongs via the FQDN:"; curl -s http://pingpong.exercises.svc.cluster.local/pongs; echo; echo "--- plain Service name, http://pingpong/pongs:"; curl -s -o /dev/null -w "HTTP %{http_code}\n" http://pingpong/pongs; echo "--- log-output page:"; curl -s http://log-output-svc:3000/'
$ kubectl --context k3d-knative logs -n exercises curl57
```

```console
$ kubectl --context k3d-knative logs -n exercises curl57
pong 0
pong 1
pong 2
--- /pongs via the FQDN:
3
--- plain Service name, http://pingpong/pongs:
HTTP 404
--- log-output page:
file content: this text is from file
env variable: MESSAGE=hello world
2026-09-25T10:52:35.665115781+00:00: 620f8b3f-41ef-4957-bd6e-881f1cd80ef3
2026-09-25T10:52:40.657695811+00:00: 620f8b3f-41ef-4957-bd6e-881f1cd80ef3
2026-09-25T10:52:45.658618334+00:00: 620f8b3f-41ef-4957-bd6e-881f1cd80ef3
... one line every 5 seconds, trimmed here ...
Ping / Pongs: 3
```

The three `pong` lines are the calls reaching the pod Knative started for them. `Ping / Pongs: 3` is Log Output
reading the same counter over the FQDN: it was already running, and the serverless backend answered it like any
other client. Its page still shows everything from part 2: the ConfigMap file, `MESSAGE`, the writer's timestamp lines.

The middle line is the exercise's tip, and easy to skip.

## Step 5 — the address that does not work

`http://pingpong/pongs` (the plain Service name part 2 used) is a **404**, and the response says who answered:

```bash
$ kubectl --context k3d-knative run curl57b -n exercises --restart=Never --image=curlimages/curl --command -- sh -c 'curl -s -i http://pingpong/pongs | head -7'
$ kubectl --context k3d-knative logs -n exercises curl57b
```

```console
$ kubectl --context k3d-knative logs -n exercises curl57b
HTTP/1.1 404 Not Found
date: Fri, 25 Sep 2026 10:58:06 GMT
server: envoy
content-length: 0
```

`server: envoy` is Kourier: the ingress answered, not the app. The reason is the ExternalName from Step 4. The
Service name resolves to the ingress, and the ingress picks a route by the **Host** header. The request went out
with `Host: pingpong`, a Host Knative does not know, so nothing matched and it answered 404. The fully qualified
name `pingpong.exercises.svc.cluster.local` is a Host it knows, which is what "this avoids host-routing issues in
the Knative setup" means in the exercise.

The same difference shows from the host. With the Host header matching the Knative Service, the
loadbalancer on port 8081 is enough; without it, the same 404 as above:

```bash
$ curl -s -H "Host: pingpong.exercises.172.21.0.3.sslip.io" http://localhost:8081/pingpong
$ curl -s -H "Host: pingpong.exercises.172.21.0.3.sslip.io" http://localhost:8081/pongs
$ curl -s -o /dev/null -w "no Host header: HTTP %{http_code}\n" http://localhost:8081/pongs
```

```console
pong 0
1
no Host header: HTTP 404
```

The hostname comes from `kubectl get ksvc` (the exercise's own note); the IP part is your cluster's. The same
sentence covers the exercise's optional URLRewrite tip, which is for a cluster with a Gateway in front. This one
uses Kourier, so the Host header is how you address it.

## Step 6 — back to zero

Stop calling it and wait. Nothing is deployed differently; it is the `minScale: "0"` annotation and the autoscaler's idle timer.

```bash
$ sleep 150
$ kubectl --context k3d-knative get pods -n exercises
$ kubectl --context k3d-knative get revisions -n exercises
```

```console
$ kubectl --context k3d-knative get pods -n exercises
NAME                                        READY   STATUS        RESTARTS   AGE
curl57                                      0/1     Completed     0          4m31s
log-output-5956dc699f-vc75m                 2/2     Running       0          5m11s
pingpong-00001-deployment-b698c585b-l6dkg   1/2     Terminating   0          5m24s

$ kubectl --context k3d-knative get revisions -n exercises
NAME             CONFIG NAME   GENERATION   READY   REASON   ACTUAL REPLICAS   DESIRED REPLICAS
pingpong-00001   pingpong      1            True             0                 0
```

The revision is still there (the address and the History, not a running thing), but `ACTUAL REPLICAS` is
0, the Deployment is `0/0`, and the pod is on its way out. Only Log Output, an ordinary Deployment, stays up.

Then call it again and watch what comes back. The first request pays for the pod; the counter, in pod memory, does not come back with it:

```bash
$ kubectl --context k3d-knative run curl57c -n exercises --restart=Never --image=curlimages/curl --command -- sh -c 'curl -s -w "  cold: %{time_total}s\n" -o /dev/null http://pingpong.exercises.svc.cluster.local/pingpong; echo -n "  counter after cold: "; curl -s http://pingpong.exercises.svc.cluster.local/pongs; echo; curl -s -w "  warm: %{time_total}s\n" -o /dev/null http://pingpong.exercises.svc.cluster.local/pingpong'
$ kubectl --context k3d-knative logs -n exercises curl57c
```

```console
$ kubectl --context k3d-knative get revisions -n exercises
pingpong-00001   pingpong   1     True         0     0

$ kubectl --context k3d-knative logs -n exercises curl57c
  cold: 3.203534s
  counter after cold: 1
  warm: 0.030092s

$ kubectl --context k3d-knative get revisions -n exercises
pingpong-00001   pingpong   1     True         1     1
```

`counter after cold: 1` is the number after that one `/pingpong` call: it was 3 in Step 4, and the pod holding that 3 is gone. Ping pong satisfies Knative's contract in the ways that matter (stateless from the platform's
point of view, `PORT`, stdout), but its whole *content* is one counter in memory, and a counter in memory is
state. That is what serverless costs you: 3.2 seconds when someone arrives after a quiet spell, 0.03 seconds when
the pod is warm, and your state gone with the pod. Real ping pongs live in a database for the same reason.

(The timeout numbers are from the pod's own clock and include the whole HTTP round trip; they move with the machine, the order of magnitude does not.)

## When the ingress does not come up

Two ways the platform can look broken while your manifests are fine, both hit while writing this lab. The shared
first symptom: `kubectl get ksvc` shows `READY Unknown` with reason `IngressNotConfigured` while
`ConfigurationsReady` is `True`, so the app, its Deployment and pods are healthy and only routing is missing.

**The ingress class is not the one Kourier answers.** The class in `config-network` has to be the full
`kourier.ingress.networking.knative.dev`. With the short `kourier`, the controller stays silent and looks like a
broken cluster:

```console
$ kubectl -n knative-serving get configmap config-network -o jsonpath='{.data.ingress-class}'
kourier.ingress.networking.knative.dev

$ kubectl get ksvc -n exercises
NAME       URL                                           LATESTCREATED    LATESTREADY      READY     REASON
pingpong   http://pingpong.exercises.svc.cluster.local   pingpong-00001   pingpong-00001   Unknown   IngressNotConfigured

$ kubectl logs -n knative-serving -l app=net-kourier-controller --tail=3
... "message":"Starting controller and workers" ...
... "message":"Started workers" ...
... "message":"Successfully acquired lease" ...
```

No error, no reconcile, and the URL has lost its `sslip.io` part too, because a Route that is not exposed falls
back to the cluster-local domain. Fixing the value is enough: the controller picks the existing Ingress up on its
own:

```console
$ kubectl patch configmap/config-network -n knative-serving --type merge --patch '{"data":{"ingress-class":"kourier.ingress.networking.knative.dev"}}'
configmap/config-network patched

$ kubectl get ksvc -n exercises
NAME       URL                                             LATESTCREATED    LATESTREADY      READY   REASON
pingpong   http://pingpong.exercises.172.21.0.3.sslip.io   pingpong-00001   pingpong-00001   True

$ kubectl logs -n knative-serving -l app=net-kourier-controller --tail=1
... "logger":"net-kourier-controller" ... "message":"Reconcile succeeded" ...
```

**A URL that keeps the cluster-local form.** If `config-network` is right and the URL is still
`...svc.cluster.local`, the `default-domain` job never wrote the domain; its log shows only a client-config warning:

```console
$ kubectl -n knative-serving logs job/default-domain --tail=1
W0925 12:04:48.570108       1 client_config.go:682] Neither --kubeconfig nor --master was specified.  Using the inClusterConfig.  This might not work.
```

Running the job again is enough, and the URL follows:

```console
$ kubectl -n knative-serving delete job default-domain && kubectl apply -f https://github.com/knative/serving/releases/download/knative-v1.23.0/serving-default-domain.yaml
job.batch "default-domain" deleted from knative-serving namespace
job.batch/default-domain created

$ kubectl -n knative-serving logs job/default-domain --tail=1
{"level":"info","ts":1790338181.502145,"logger":"fallback.default-domain","caller":"default-domain/main.go:238","msg":"Updated default domain to: 172.21.0.3.sslip.io"}
```

## Cleanup

```bash
$ kubectl --context k3d-knative delete namespace exercises
```

## P.S.

- The exercise is one line of YAML and one line of `PINGS_URL`, and both say the same thing: serverless puts an
  ingress in front of your app, and an ingress routes by Host, so the address you have used since part 2 is no longer one.
- A Knative Service is a Deployment you did not write (Deployment, two Services, ReplicaSet, pod, queue-proxy,
  autoscaler) with a revision history on top. The revision is the unit of change; every edit to the template makes
  a new one.
- Scale to zero is a cold start per revision, and the state in the pod goes with it. `minScale: "1"` (or an external store) refuses that.
- Local images need the `dev.local/` prefix and `imagePullPolicy: IfNotPresent`, and a Rust build needs its
  `Cargo.lock`: a missing lock is not a build error, it is a different dependency set.
- Nothing in the apps changed to become serverless. No new flag, no SDK, no framework: the contract is
  environment variables plus stdout, and both apps already followed it.
