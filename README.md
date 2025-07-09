# Kubera (name TBD)

## Rust-based Kubernetes Gateway API implementation using Pingora

Kubera is a Rust-based implementation of the Kubernetes Gateway API, leveraging the Pingora proxy for efficient and
scalable traffic management. This project aims to provide a robust and flexible solution for managing ingress traffic in
Kubernetes clusters.

This project is in its infancy, and I'm continuing to experiment with the design and implementation. I don't have a goal
at this time to make it production worthy without additional help.

## Directory structure

- `api/`: Contains custom resource definitions (CRDs) for the Gateway API.
- `build/`: Contains build scripts and code generation tools.
- `control_plane/`: Contains the controller logic for managing Gateway API resources via the Kubernetes API.
- `core/`: Contains the core configuration and reusable components.
- `gateway/`: Contains the proxy implementation using Pingora.
- `helm/`: Contains a Helm chart for deploying the Kubera controller.

## Running locally

To run Kubera locally, you need to have the following prerequisites installed:

- Rust
- Docker
- Minikube or a Kubernetes cluster

### Steps to run locally

1. Build using Cargo:
   ```bash
   cargo build
   ```
2. Start Minikube:
   ```bash
   minikube start
   ```
3. Configure Docker to use Minikube's Docker daemon:
   ```bash
   eval $(minikube docker-env)
   ```
4. Build the Docker image:
   ```bash
   docker build -t kubera-controller:latest .
   ```
5. Deploy the controller to Minikube:
   ```bash
   helm upgrade --install kubera helm/
   ```
6. Add `Service`, `Deployment`, `Gateway` and `HTTPRoute` resources to your cluster (not provided in this repo yet):
   ```bash
   kubectl apply -f api/gateway.yaml
   kubectl apply -f api/http_route.yaml
   ```

## CRDs

The following CRDs are defined in the `api/` directory:

* `GatewayClassParameters`: Defines parameters for a `GatewayClass`, applies things to all `Gateway`s
* `GatewayParameters`: Defines parameters for a `Gateway`, applies to a specific `Gateway`