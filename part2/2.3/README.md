# Exercise 2.3 — Keep them separated (Namespaces)

## Goal

> Create a namespace called **exercises** for the applications in the
> exercises. Move the **"Log output"** and **"Ping-pong"** to that
> namespace and use that in the future for all of the exercises, except
> the project that shall have a separate namespace. You can follow the
> course material using the default namespace.

## Step 1 — Create the namespace

```bash
kubectl create namespace exercises
kubectl get namespace
# NAME              STATUS   AGE
# default           Active   ...
# exercises         Active   1s
# kube-system       Active   ...
```

> **K8s concept**: a namespace is a virtual cluster inside the real
> cluster. Two services with the same name can exist in different
> namespaces without colliding.

## Step 2 — Apply and verify

```bash
kubectl apply -f manifests/

# everything is namespaced now — inspect with -n exercises
kubectl get all -n exercises
# pod/log-output-xxx    2/2  Running
# pod/ping-pong-xxx     1/1  Running
# service/log-output-svc   ClusterIP   10.43.x.x   3000/TCP
# service/ping-pong-svc    ClusterIP   10.43.x.x   3000/TCP

# Ingress works through the load balancer as before
curl -s http://localhost:8081/pingpong   # pong 0
curl -s http://localhost:8081/pingpong   # pong 1
curl -s http://localhost:8081/pingpong   # pong 2
curl -s http://localhost:8081/log | tail -1
# Ping / Pongs: 3
```

> **K8s concept**: `kubectl get pods` with no `-n` shows only the
> `default` namespace, which is now empty. Use `-n exercises` (or `-A` /
> `--all-namespaces`) to see these pods.

## Step 3 — Cross-namespace DNS (debugging pod)

The course notes that a service in another namespace is reachable as
`<service>.<namespace>`. Prove it with the busybox pod: run it in
**default** and hit the `exercises` services by fully-qualified name:

```bash
# busybox.yaml — note: NO namespace (it goes to default)
cat > busybox.yaml <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: my-busybox
  labels:
    app: my-busybox
spec:
  containers:
    - image: busybox
      command: ["sleep", "3600"]
      imagePullPolicy: IfNotPresent
      name: busybox
  restartPolicy: Always
EOF

kubectl apply -f busybox.yaml
kubectl get pod my-busybox        # default namespace, wait Running

# Fully-qualified name: service.namespace
kubectl exec -it my-busybox -- wget -qO - http://ping-pong-svc.exercises:3000/pongs
# 3

kubectl exec -it my-busybox -- wget -qO - http://log-output-svc.exercises:3000 | tail -1
# Ping / Pongs: 3

# The short name fails from another namespace (proves namespacing):
kubectl exec -it my-busybox -- wget -qO - http://ping-pong-svc:3000/pongs
# wget: bad address 'ping-pong-svc'   ← only resolvable inside `exercises`

kubectl delete pod my-busybox
```

> That is the `service.namespace` DNS form from the course material
> (`cat-pictures.ns-test`). The short name works only _inside_ the
> namespace.

## Step 4 — Clean up

```bash
kubectl delete -f manifests/
kubectl delete pod my-busybox 2>/dev/null   # if still around
kubectl get all -n exercises
# No resources found in exercises namespace
```

## P/S

1. **Namespaces isolate resources**: same app names can coexist in
   different namespaces; `kubectl` scopes to one unless you pass `-n`
   or `-A`.
2. **DNS is namespace-aware**: short `<svc>` resolves only inside its
   namespace; `<svc>.<namespace>` resolves cluster-wide.
3. **Namespaced resources need namespace-qualified references**: the
   Traefik middleware annotation switched from `default-…` to
   `exercises-…`.
4. **Organization is pure manifest work**: moving apps between
   namespaces changes no binaries or images.
