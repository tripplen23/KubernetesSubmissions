## 0. Prerequisites: a running cluster

You need a Kubernetes cluster and a working kubeconfig (kubectl must reach the API server).

```bash
kubectl cluster-info 
```

If it doesn't, create a local cluster first. Require Docker

```bash
# install
curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash

# create cluster
k3d cluster create

# verify
kubectl cluster-info
```

### Switching between clusters

```bash
kubectl config get-contexts          # list contexts
kubectl config use-context <name>    # e.g. kind-dev, k3d-dev, minikube
kubectl config current-context       # show current
```

## 1. Build and push Docker image

```bash
cd log_output
docker build -t YOUR_DOCKERHUB_USERNAME/log-output:latest .
docker push YOUR_DOCKERHUB_USERNAME/log-output:latest
```

## 2. Update deployment.yaml with your image name
Edit manifests/deployment.yaml, replace YOUR_DOCKERHUB_USERNAME

## 3. Deploy to Kubernetes
kubectl apply -f manifests/deployment.yaml

## 4. Verify it's running
kubectl get pods
kubectl logs -f <pod-name>