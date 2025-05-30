set shell := ["bash", "-c"]
set dotenv-load := true

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

@build-controlplane: minikube-start
    eval $(minikube docker-env) && \
    docker build -t kubera-controlplane:latest -f ./controlplane/Dockerfile .

@build-proxy: minikube-start
    eval $(minikube docker-env) && \
    docker build -t kubera-proxy:latest -f ./proxy/Dockerfile .

@build-crds:
    cargo build --release
    mkdir -p helm/crds
    cp -r target/release/crds helm/

@build: build-controlplane build-proxy build-crds
