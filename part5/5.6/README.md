# 5.6 — Trying serverless

The exercise asks for Knative Serving on k3d, then for its guide's three examples: a Knative Service, traffic splitting, and autoscaling. The platform here is Knative Serving **v1.23.0** on Kubernetes **v1.34.1**, the version the exercise's cluster command pins.

What the lab carries: the app in `app/` (Rust, one file, built to Knative's runtime contract), five manifests, and the receipts below from an actual run.

Every receipt is a `console` block: lines starting with `$` are commands, everything else is what they printed, so paste the `$` lines and not the output. Receipts come after the command that creates the state they show, so run each step in order.

## Step 0 — the cluster

The exercise's own command, with the reason for each flag: `--image rancher/k3s:v1.34.1-k3s1` is the Kubernetes the current Knative wants, `--disable=traefik` clears the ingress path for Kourier, `-p 8081:80@loadbalancer` is how the host reaches the Knative gateway, and `--port 8082:30080@agent:0` is the service port the exercise's own curl
example uses. The name is k3d's first argument: leave it out and the cluster is `k3s-default`, disagreeing with every receipt below.

```bash
k3d cluster create knative --port 8082:30080@agent:0 -p 8081:80@loadbalancer --agents 2 \
  --k3s-arg "--disable=traefik@server:0" --image rancher/k3s:v1.34.1-k3s1
```

```console
$ kubectl --context k3d-knative get nodes
NAME                   STATUS   ROLES           AGE   VERSION
k3d-knative-agent-0    Ready    <none>          10s   v1.34.1+k3s1
k3d-knative-agent-1    Ready    <none>          10s   v1.34.1+k3s1
k3d-knative-server-0   Ready    control-plane   15s   v1.34.1+k3s1
```

If a cluster of that name already exists (the receipts below came from one), `k3d cluster delete knative` first, so the command above is the one you actually run — or keep it and start from Step 1 instead.

If the create fails on its last step with `Bind for 0.0.0.0:8081 failed: port is already allocated`, another container already publishes 8081 — on this machine, the cluster an earlier lab left running. `docker ps --filter
publish=8081` names it and `k3d cluster delete <name>` frees the port; k3d rolls the failed attempt back by itself.

Every command below is run with `--context k3d-knative`; the receipts leave the flag out for width.

## Step 1 — Knative Serving, Kourier and Magic DNS

Four applies and one patch, in the order the Knative install guide uses: the CRDs, the core, Kourier as the network layer, and then the `default-domain` job, which is the guide's "Magic DNS (sslip.io)" option.

```bash
kubectl apply -f https://github.com/knative/serving/releases/download/knative-v1.23.0/serving-crds.yaml
kubectl apply -f https://github.com/knative/serving/releases/download/knative-v1.23.0/serving-core.yaml
kubectl apply -f https://github.com/knative/net-kourier/releases/download/knative-v1.23.0/kourier.yaml
kubectl patch configmap/config-network -n knative-serving --type merge \
  -p '{"data":{"ingress-class":"kourier.ingress.networking.knative.dev"}}'
kubectl apply -f https://github.com/knative/serving/releases/download/knative-v1.23.0/serving-default-domain.yaml
```

The trap: applying the CRDs and the core together leaves the core half-installed, because the API server has not published `caching.internal.knative.dev/v1alpha1` when the core asks for it. Applying the core a second time is the whole fix. (The exercise's screenshot shows a different failure, pods in `CrashLoopBackOff`; either way, read the message, not the colour.)

```console
$ kubectl apply -f https://github.com/knative/serving/releases/download/knative-v1.23.0/serving-core.yaml
error: resource mapping not found for name: "queue-proxy" namespace: "knative-serving" from "https://github.com/knative/serving/releases/download/knative-v1.23.0/serving-core.yaml": no matches for kind "Image" in version "caching.internal.knative.dev/v1alpha1"
ensure CRDs are installed first

$ kubectl apply -f https://github.com/knative/serving/releases/download/knative-v1.23.0/serving-core.yaml   # again
mutatingwebhookconfiguration.admissionregistration.k8s.io/webhook.serving.knative.dev configured
validatingwebhookconfiguration.admissionregistration.k8s.io/validation.webhook.serving.knative.dev configured
```

All six pods are then ready, and the `default-domain` job is a `Completed` pod rather than a service:

```console
$ kubectl -n knative-serving get pods
NAME                                      READY   STATUS      RESTARTS   AGE
activator-56d698c974-qsg44                1/1     Running     0          28m
autoscaler-69fcf466cb-b4nch               1/1     Running     0          28m
controller-5857b6bf55-ftf5c               1/1     Running     0          28m
default-domain-99bzg                      0/1     Completed   0          26m
net-kourier-controller-5d74dfdd6f-hj2g9   1/1     Running     0          28m
webhook-5cdb4c6879-wczc6                  1/1     Running     0          28m
```

The domain comes from the ingress address, which is why Magic DNS works on a cluster with no DNS server of its own:

```console
$ kubectl -n knative-serving logs job/default-domain | tail -1
{"level":"info","msg":"Updated default domain to: 172.21.0.3.sslip.io"}

$ kubectl -n kourier-system get svc kourier
NAME      TYPE           CLUSTER-IP     EXTERNAL-IP                        PORT(S)
kourier   LoadBalancer   10.43.37.122   172.21.0.3,172.21.0.4,172.21.0.5   80:31122/TCP,443:30568/TCP
```

## Step 2 — the app, written against the runtime contract

Knative's runtime contract is short: stateless, configured from the environment, listening on the injected `PORT`, logging to stdout, leaving when asked. The app in `app/` is that contract in one file, plus a second route the autoscaling example needs.

Knative resolves every image tag to a digest **against its registry** before creating a pod, so a bare `hello:5.6` means `docker.io/library/hello:5.6`, Docker Hub answers 401, and the revision never starts. Worth seeing once, in its own namespace so the real Service's numbering stays clean:

`manifests/hello-bare-image.yaml`

```yaml
# The before picture for Step 2: the same Service, with the image left as a bare
# name. Knative resolves that name against its registry before anything reaches
# the node, so the revision never starts — apply this into a namespace of its
# own (`kubectl apply -n trap -f ...`) and delete the namespace afterwards.
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: hello
spec:
  template:
    spec:
      containers:
        - image: hello:5.6
```

```bash
kubectl create namespace trap
kubectl apply -n trap -f manifests/hello-bare-image.yaml
sleep 15   # give the controller a moment to mark the revision
kubectl -n trap get revision -o jsonpath='{range .items[*]}{.metadata.name}{"	"}{.status.conditions[?(@.type=="Ready")].message}{"\n"}{end}'
kubectl -n trap get ksvc
kubectl delete namespace trap
```

![A terminal screenshot of the Step 2 trap from start to finish: creating the `trap` namespace, applying `manifests/hello-bare-image.yaml` with its securityContext `Warning:` line, `sleep 15`, the revision's `failed to resolve image to digest … 401 Unauthorized` message, `kubectl -n trap get ksvc` reporting `READY False` with reason `RevisionMissing`, and deleting the namespace](./assets/image.png)

The `Warning:` about `securityContext` shows up on every apply here — Knative asking the manifest to be explicit about hardening, about Kubernetes' own defaults, so it is noise. With no Service at all, the jsonpath above prints nothing instead; `kubectl get revisions` is the form that says why: `No resources found in default namespace.`

The fix is the prefix Knative skips resolving for — `dev.local` — so the image is imported under that name and the manifest asks for that name:

```bash
docker build -t hello:5.6 app
docker tag hello:5.6 dev.local/hello:5.6
k3d image import dev.local/hello:5.6 -c knative
```

```console
$ docker exec k3d-knative-server-0 crictl images | grep dev.local
IMAGE                     TAG                 IMAGE ID            SIZE
dev.local/hello           5.6                 76d7fe32255f2       32.5MB
```

## Step 3 — example 1: a Knative Service

One object, and the platform writes the rest.

`manifests/hello.yaml`

```yaml
# A Knative Service: the app, and none of the Deployment, Service, Ingress or
# autoscaler you would have written yourself. The platform creates all of them
# from this, and it is what makes "scale to zero" possible.
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: hello
  namespace: default
spec:
  template:
    spec:
      containers:
        # The dev.local prefix is not decoration: Knative resolves every image
        # tag to a digest against its registry before the pod is created, and
        # a bare `hello:5.6` means docker.io/library/hello — Docker Hub answers
        # 401 and the revision never starts (ContainerMissing). dev.local is on
        # Knative's list of registries it skips resolving.
        - image: dev.local/hello:5.6
          imagePullPolicy: IfNotPresent
          # The runtime contract's other half: configuration is environment,
          # so one image can answer as many revisions.
          env:
            - name: TARGET
              value: "Knative 5.6"
```

```console
$ kubectl apply -f manifests/hello.yaml
service.serving.knative.dev/hello created

$ kubectl get ksvc
NAME    URL                                        LATESTCREATED   LATESTREADY   READY   REASON
hello   http://hello.default.172.21.0.3.sslip.io   hello-00001     hello-00001   True
```

What Knative created for that one object — Deployment, two Services, ReplicaSet, pod — none of it written by hand (names and hashes differ per run; the shape does not):

```console
$ kubectl get deploy,svc,rs,pods -l serving.knative.dev/service=hello
NAME                                     READY   UP-TO-DATE   AVAILABLE   AGE
deployment.apps/hello-00001-deployment   1/1     1            1           3m43s

NAME                          TYPE           CLUSTER-IP      EXTERNAL-IP                                         PORT(S)                                     AGE
service/hello                 ExternalName   <none>          kourier-internal.kourier-system.svc.cluster.local   80/TCP                                      3m32s
service/hello-00001           ClusterIP      10.43.228.192   <none>                                              80/TCP,443/TCP                              3m42s
service/hello-00001-private   ClusterIP      10.43.169.22    <none>                                              80/TCP,443/TCP,9090/TCP,9091/TCP,8012/TCP   3m42s

NAME                                                DESIRED   CURRENT   READY   AGE
replicaset.apps/hello-00001-deployment-6884596465   1         1         1       3m43s

NAME                                          READY   STATUS    RESTARTS   AGE
pod/hello-00001-deployment-6884596465-dxwnq   2/2     Running   0          9s
```

The pod that runs a serverless app has two containers, and the second one is the subject of the previous exercise:

```console
$ kubectl get pods -l serving.knative.dev/revision=hello-00001 -o jsonpath='{range .items[*]}{.metadata.name}{"  "}{range .spec.containers[*]}{.name}{" "}{end}{"\n"}{end}'
hello-00001-deployment-b7469c96-w5xlh  user-container queue-proxy
```

Calling it from the host uses the URL `kubectl get ksvc` reports as the `Host` header — the exercise's own trick, with the port the cluster was created with:

```console
$ H=$(kubectl get ksvc hello -o jsonpath='{.status.url}' | sed 's|http://||')
$ curl -H "Host: $H" http://localhost:8081
Hello Knative 5.6! (answered by hello-00001)

$ curl -H "Host: $H" "http://localhost:8081/work?ms=250"
slept 250ms

$ kubectl logs -l serving.knative.dev/revision=hello-00001 -c user-container --tail=1
listening on 8080, TARGET=Knative 5.6, K_REVISION=hello-00001
```

The last line is the contract in practice: `PORT` was injected, `TARGET` came from the manifest, and `K_REVISION` came from the platform, not from the app.

### Scale to zero

No traffic means no pods at all, and the next request pays for starting one:

```console
$ kubectl get pods
No resources found in default namespace.

$ kubectl get revisions
NAME          CONFIG NAME   GENERATION   READY   REASON   ACTUAL REPLICAS   DESIRED REPLICAS
hello-00001   hello         1            True             0                 0

$ H=$(kubectl get ksvc hello -o jsonpath='{.status.url}' | sed 's|http://||')
$ curl -s -o /dev/null -w "%{http_code} in %{time_total}s\n" -H "Host: $H" http://localhost:8081
200 in 1.858889s

$ curl -s -o /dev/null -w "%{http_code} in %{time_total}s\n" -H "Host: $H" http://localhost:8081
200 in 0.004166s
```

The first request is held by the activator while a pod starts; the second one goes straight to it. That 1.9 seconds is the cold start, and it is what Knative buys with "zero replicas when nobody is asking".

Reading logs in that state finds nothing, because there is no pod to read: `kubectl logs -l serving.knative.dev/revision=hello-00001 -c user-container --tail=1` answers `No resources found in default
namespace.` until a request wakes the revision.

## Step 4 — example 2: traffic splitting

The second revision is made by changing the revision template — the same image, a different environment — and the `traffic` block decides how the requests divide.

`manifests/hello-split.yaml`

```yaml
# The same Service, with the environment changed: applying it creates a second
# revision rather than restarting the first one, and the traffic block decides
# how requests are divided between them.
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: hello
  namespace: default
spec:
  template:
    spec:
      containers:
        - image: dev.local/hello:5.6
          imagePullPolicy: IfNotPresent
          env:
            - name: TARGET
              value: "the newer revision"
  traffic:
    # Revision names are the lab's own (`kubectl get revisions` lists yours):
    # the first two are hello-00001 and hello-00002.
    - revisionName: hello-00001
      percent: 50
    - revisionName: hello-00002
      percent: 50
```

```console
$ kubectl apply -f manifests/hello-split.yaml
service.serving.knative.dev/hello configured

$ kubectl get revisions
NAME          CONFIG NAME   GENERATION   READY   REASON   ACTUAL REPLICAS   DESIRED REPLICAS
hello-00001   hello         1            True             1                 1
hello-00002   hello         2            True             1                 1

$ kubectl get ksvc hello -o jsonpath='{range .status.traffic[*]}{.revisionName}{"  "}{.percent}{"%  latestRevision="}{.latestRevision}{"\n"}{end}'
hello-00001  50%  latestRevision=false
hello-00002  50%  latestRevision=false
```

The second revision is a whole second set of objects: its own Deployment, two Services, ReplicaSet and pod — the Step 3 command again, now twice over (ages and hashes from the run that produced this lab):

```console
$ kubectl get deploy,svc,rs,pods -l serving.knative.dev/service=hello
NAME                                     READY   UP-TO-DATE   AVAILABLE   AGE
deployment.apps/hello-00001-deployment   1/1     1            1           8m2s
deployment.apps/hello-00002-deployment   1/1     1            1           7m41s

NAME                          TYPE           CLUSTER-IP     EXTERNAL-IP                                         PORT(S)                                     AGE
service/hello                 ExternalName   <none>         kourier-internal.kourier-system.svc.cluster.local   80/TCP                                      7m54s
service/hello-00001           ClusterIP      10.43.136.6    <none>                                              80/TCP,443/TCP                              8m2s
service/hello-00001-private   ClusterIP      10.43.97.136   <none>                                              80/TCP,443/TCP,9090/TCP,9091/TCP,8012/TCP   8m2s
service/hello-00002           ClusterIP      10.43.171.80   <none>                                              80/TCP,443/TCP                              7m41s
service/hello-00002-private   ClusterIP      10.43.48.71    <none>                                              80/TCP,443/TCP,9090/TCP,9091/TCP,8012/TCP   7m41s

NAME                                                DESIRED   CURRENT   READY   AGE
replicaset.apps/hello-00001-deployment-b7469c96     1         1         1       8m2s
replicaset.apps/hello-00002-deployment-6f96c7fd5c   1         1         1       7m41s

NAME                                          READY   STATUS    RESTARTS   AGE
pod/hello-00001-deployment-b7469c96-w5xlh     2/2     Running   0          33s
pod/hello-00002-deployment-6f96c7fd5c-w9mvl   2/2     Running   0          12s
```

Twenty requests, counted by the revision that answered them:

```console
$ H=$(kubectl get ksvc hello -o jsonpath='{.status.url}' | sed 's|http://||')
$ for i in $(seq 1 20); do curl -s -H "Host: $H" http://localhost:8081; done | sed 's/.*answered by //; s/)//' | sort | uniq -c
     8 hello-00001
     12 hello-00002
```

The answers alternate, which is easier to read in a short sample than the count is:

```console
$ H=$(kubectl get ksvc hello -o jsonpath='{.status.url}' | sed 's|http://||')
$ for i in 1 2 3 4; do curl -s -H "Host: $H" http://localhost:8081; done
Hello the newer revision! (answered by hello-00002)
Hello Knative 5.6! (answered by hello-00001)
Hello Knative 5.6! (answered by hello-00001)
Hello the newer revision! (answered by hello-00002)
```

A 90/10 split is the same object with two percentages changed:

```console
$ H=$(kubectl get ksvc hello -o jsonpath='{.status.url}' | sed 's|http://||')
$ for i in $(seq 1 20); do curl -s -H "Host: $H" http://localhost:8081; done | sed 's/.*answered by //; s/)//' | sort | uniq -c
     20 hello-00001
```

Twenty requests landing on one revision is what a 10% share looks like when the sample is that small: the probability of seeing no `hello-00002` in twenty draws is still about one in eight.

## Step 5 — example 3: autoscaling

`manifests/hello-autoscale.yaml`

```yaml
# A second Service on purpose: the autoscaling demo changes the revision
# template, and a changed template is a new revision — which would disturb the
# two revisions the traffic-splitting demo splits between.
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: hello-autoscale
  namespace: default
spec:
  template:
    metadata:
      annotations:
        # The Knative autoscaling docs' own example: one in-flight request per
        # pod, so concurrency produces replicas instead of a queue.
        autoscaling.knative.dev/target: "1"
    spec:
      containers:
        - image: dev.local/hello:5.6
          imagePullPolicy: IfNotPresent
          env:
            - name: TARGET
              value: "autoscaling"
```

A **second** Service on purpose — changing the revision template creates a revision, and the two revisions the split above uses are worth keeping untouched.

```console
$ kubectl apply -f manifests/hello-autoscale.yaml
Warning: Kubernetes default value is insecure, Knative may default this to secure in a future release: spec.template.spec.containers[0].securityContext.allowPrivilegeEscalation, spec.template.spec.containers[0].securityContext.capabilities, spec.template.spec.containers[0].securityContext.runAsNonRoot, spec.template.spec.containers[0].securityContext.seccompProfile
service.serving.knative.dev/hello-autoscale created

$ kubectl get revision -l serving.knative.dev/service=hello-autoscale -o jsonpath='{range .items[*]}{.metadata.name}{"  target="}{.metadata.annotations.autoscaling\.knative\.dev/target}{"\n"}{end}'
hello-autoscale-00001  target=1
```

`manifests/load-generator.yaml`

```yaml
# Six clients, each holding one 300 ms request at a time, for a minute: a Job
# with parallelism, so the load is one object instead of six hand-written
# commands. What the autoscaler follows is requests in flight, not clients,
# which is the point of the example — six clients on the `/` route (milliseconds
# per answer) would not move it.
apiVersion: batch/v1
kind: Job
metadata:
  name: load
spec:
  parallelism: 6
  completions: 6
  template:
    spec:
      restartPolicy: Never
      containers:
        - name: client
          image: busybox:1.36
          command:
            - sh
            - -c
            - |
              end=$(( $(date +%s) + 60 ))
              while [ "$(date +%s)" -lt "$end" ]; do
                wget -q -O- "http://hello-autoscale.default.svc.cluster.local/work?ms=300" >/dev/null 2>&1
              done
```

```console
$ kubectl apply -f manifests/load-generator.yaml
job.batch/load created
```

Six clients then call `/work?ms=300` for a minute, and the replica count is sampled while they run:

```console
$ for t in 10 20 30 40 50; do sleep 10; n=$(kubectl get pods -l serving.knative.dev/service=hello-autoscale --no-headers | grep -c Running); r=$(kubectl get revision -l serving.knative.dev/service=hello-autoscale -o jsonpath='{.items[0].status.actualReplicas}'); echo "t=${t}s  Running=$n  actualReplicas=$r"; done
t=10s  Running=9  actualReplicas=9
t=20s  Running=9  actualReplicas=9
t=30s  Running=9  actualReplicas=9
t=40s  Running=9  actualReplicas=9
t=50s  Running=9  actualReplicas=9
```

Nine pods for six clients: the autoscaler follows requests *in flight*, not clients, and each client holds one 300 ms request open, which is what the queue-proxy reports as concurrency. The same six clients on `/`, where answers take milliseconds, barely move it — twelve clients there kept three pods, the same rule from the other side.

When the clients stop, the replicas go back to zero on their own — the Job reaches `6/6` after its minute and Knative does the rest (`kubectl delete job load` clears it out for a rerun):

```console
$ kubectl get revisions
NAME                    CONFIG NAME       GENERATION   READY   REASON   ACTUAL REPLICAS   DESIRED REPLICAS
hello-00001             hello             1            True             0                 0
hello-00002             hello             2            True             0                 0
hello-autoscale-00001   hello-autoscale   1            True             0                 0
```

## P.S.

- A Knative Service is mostly a Deployment you did not write: the platform creates the Deployment, the Services, the public route, the queue-proxy sidecar and the autoscaler, and keeps the revision history.
- Scale to zero is a cold start per revision: the first request after idle waits for a pod, the second does not. `minScale` keeps one warm instead.
- Local images need `dev.local/` and `imagePullPolicy: IfNotPresent`: Knative resolves tags against a registry before Kubernetes looks in its own store.
- The revision template is the unit of change (environment, annotations, image); `traffic` is the only thing that should differ between two deploys of the same code.
- The runtime contract is why this app would run on Cloud Run unchanged: stateless, env config, `PORT`, stdout, SIGTERM drain.
