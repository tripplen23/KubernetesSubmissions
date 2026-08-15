# Exercise 2.4 — The project, step 9 (namespace `project`)

## Goal

> Create a namespace called **project** for the project and move
> everything related to the project to that namespace. Use the new
> namespace in the future for all the project related exercises.

## Step 1 — Create the namespace

```bash
kubectl create namespace project
kubectl get namespace
# NAME              STATUS   AGE
# default           Active   ...
# exercises         Active   ...
# project           Active   1s
```

## Step 2 — Apply and verify

```bash
docker exec k3d-mycluster-agent-0 mkdir -p /tmp/kube
kubectl apply -f manifests/pv/persistentvolume.yaml
kubectl apply -f manifests/
kubectl get all -n project
# pod/todo-app-xxx       1/1  Running
# pod/todo-backend-xxx   1/1  Running
# service/todo-app-svc      ClusterIP   10.43.x.x   3000/TCP
# service/todo-backend-svc  ClusterIP   10.43.x.x   2345/TCP

kubectl get pvc -n project
# image-claim   Bound   example-pv   1Gi   RWO

# Ingress through the load balancer
curl -s -X POST http://localhost:8081/todos -d 'content=Learn Kubernetes' -w "%{http_code}\n"
# 303
curl -s http://localhost:8081/ | grep '<span>'
# <span>Learn Kubernetes</span>
```

> **K8s concept**: the PVC binds to the cluster-scoped PV from inside
> the `project` namespace. `kubectl get pv` shows the PV (no `-n` — it's
> global); `kubectl get pvc -n project` shows the claim.

## Step 3 — Cross-namespace DNS (debugging pod)

Prove the project services are in `project` by reaching them from the
`default` namespace via their fully-qualified name:

```bash
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
kubectl get pod my-busybox     # default namespace

# Fully-qualified name: service.namespace
kubectl exec -it my-busybox -- wget -qO - http://todo-backend-svc.project:2345/todos
# []

kubectl exec -it my-busybox -- wget -qO - http://todo-app-svc.project:3000/ | grep -o '<title>[^<]*</title>'
# <title>Todo App</title>

# Short name fails from another namespace:
kubectl exec -it my-busybox -- wget -qO - http://todo-backend-svc:2345/todos
# wget: bad address 'todo-backend-svc'

kubectl delete pod my-busybox
```

## Step 4 — Clean up

```bash
kubectl delete -f manifests/
kubectl delete -f manifests/pv/persistentvolume.yaml
kubectl get all -n project
# No resources found in project namespace
```

## P/S

1. **Project in its own namespace** — todo-app + todo-backend live in
   `project`; exercise apps (log-output, ping-pong) live in `exercises`.
2. **PV vs PVC scope**: the PV is cluster-scoped (no namespace), the
   PVC is namespaced and claims the PV from inside `project`.
3. **Short vs FQDN service names**: short name works inside the
   namespace; `<svc>.<namespace>` works cluster-wide.
4. **No code change** — organization is pure manifest work.
