set shell := ["bash", "-c"]
set dotenv-load

@minikube-start:
  minikube status > /dev/null 2>&1; status=$? || true; \
  if [[ "$status" != "0" ]]; then \
    minikube start; \
  fi;

@minikube-stop:
  minikube status > /dev/null 2>&1; status=$? || true; \
  if [[ "$status" == "0" ]]; then \
    minikube stop; \
  fi;

@build-controller: minikube-start
  eval $(minikube docker-env) && \
  docker build -t kubera-controller:latest -f ./controller/Dockerfile .

@build-gateway: minikube-start
  eval $(minikube docker-env) && \
  docker build -t kubera-gateway:latest -f ./gateway/Dockerfile .

@build-crds:
  cargo build --release
  mkdir -p helm/crds
  cp -r target/release/crds helm/

@build: build-controller build-gateway build-crds