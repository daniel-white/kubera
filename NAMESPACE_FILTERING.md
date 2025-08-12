# HTTP Route Namespace Filtering Implementation

This document describes the implementation of namespace filtering for HTTP routes based on the Gateway CRD's
`allowedRoutes` configuration.

## Overview

The namespace filtering feature restricts which HTTP routes can attach to a Gateway based on the Gateway's
`allowedRoutes.namespaces.from` configuration. This is a standard Gateway API feature that provides security boundaries
by controlling cross-namespace route attachment.

## Implementation Details

### Core Components

1. **Modified HTTP Route Filter** (`control_plane/src/controllers/filters/http_routes.rs`)
    - Enhanced the existing `filter_http_routes` function to include namespace filtering
    - Added `is_http_route_allowed_by_gateway` helper function

2. **Comprehensive Tests** (`tests/control_plane_tests.rs`)
    - Added thorough test coverage for all namespace filtering scenarios
    - Tests cover `Same`, `All`, `Selector`, and unknown policy values

3. **Demo Example** (`examples/vale-gateway-echo-namespace-filtering.yaml`)
    - Demonstrates namespace filtering in action
    - Includes both allowed and rejected route scenarios

### Supported Policies

The implementation supports the standard Gateway API namespace policies:

#### `Same` (Default)

- Only allows HTTP routes from the same namespace as the Gateway
- This is the default behavior when `allowedRoutes` is not specified
- Provides the most restrictive security boundary

#### `All`

- Allows HTTP routes from any namespace to attach to the Gateway
- Provides maximum flexibility but reduced security isolation

#### `Selector`

- Placeholder for namespace label selector filtering
- Currently logs a warning and allows the route (to be implemented in future)
- Would allow routes from namespaces matching specific label selectors

### Configuration Examples

#### Restrictive Gateway (Same namespace only)

```yaml
apiVersion: gateway.networking.k8s.io/v1beta1
kind: Gateway
metadata:
  name: restrictive-gateway
  namespace: default
spec:
  gatewayClassName: vale-gateway
  listeners:
    - name: http
      protocol: HTTP
      port: 80
      allowedRoutes:
        namespaces:
          from: Same  # Only routes from 'default' namespace
```

#### Permissive Gateway (All namespaces)

```yaml
apiVersion: gateway.networking.k8s.io/v1beta1
kind: Gateway
metadata:
  name: permissive-gateway
  namespace: default
spec:
  gatewayClassName: vale-gateway
  listeners:
    - name: http
      protocol: HTTP
      port: 80
      allowedRoutes:
        namespaces:
          from: All  # Routes from any namespace allowed
```

### Behavior Matrix

| Gateway Namespace | Route Namespace | Policy     | Allowed     | Notes                              |
|-------------------|-----------------|------------|-------------|------------------------------------|
| `default`         | `default`       | `Same`     | ✅ Yes       | Same namespace                     |
| `default`         | `other`         | `Same`     | ❌ No        | Different namespace                |
| `default`         | `default`       | `All`      | ✅ Yes       | All namespaces allowed             |
| `default`         | `other`         | `All`      | ✅ Yes       | All namespaces allowed             |
| `default`         | `other`         | `Selector` | ⚠️ Allowed* | *Not yet implemented, logs warning |
| `default`         | `other`         | `Unknown`  | ❌ No        | Unknown policy values rejected     |

### Logging and Observability

The implementation provides comprehensive logging:

- **INFO**: When routes are allowed and processed
- **DEBUG**: When routes are rejected with specific reasons
- **WARN**: For unimplemented features (Selector) or invalid configurations

Example log outputs:

```
INFO HTTPRoute object.ref=default/echo-route-allowed matches an active Vale Gateway and is allowed by allowedRoutes configuration
DEBUG HTTPRoute default/echo-route-rejected from namespace other rejected by Gateway default/vale-gateway listener http: allowedRoutes.namespaces.from=Same
WARN Namespace selector filtering not yet implemented for Gateway default/vale-gateway listener http, allowing HTTPRoute other/test-route for now
```

### Testing

The implementation includes comprehensive tests covering:

1. **Same namespace policy**: Routes in same namespace are allowed
2. **Cross-namespace rejection**: Routes from different namespaces are rejected with `Same` policy
3. **All namespaces policy**: Routes from any namespace are allowed with `All` policy
4. **Selector policy**: Currently allows routes with warning (placeholder for future implementation)
5. **Unknown policy**: Routes are rejected for unknown policy values
6. **Error handling**: Routes without namespaces are properly rejected

Run tests with:

```bash
cargo test control_plane_tests
```

### Demo Usage

The namespace filtering demo is available at `examples/vale-gateway-echo-namespace-filtering.yaml`.

Deploy the demo:

```bash
kubectl apply -f examples/vale-gateway-echo-namespace-filtering.yaml
```

This creates:

- Two namespaces (`default` and `other-namespace`)
- Two Gateways with different policies
- Three HTTP routes demonstrating allowed/rejected scenarios

Expected behavior:

- `echo-route-allowed` (same namespace): ✅ Processed by Vale Gateway
- `echo-route-rejected` (different namespace): ❌ Filtered out by control plane
- `echo-route-permissive-allowed` (different namespace, permissive Gateway): ✅ Processed by Vale Gateway

### Security Implications

This feature provides important security boundaries:

1. **Namespace Isolation**: Prevents routes from unauthorized namespaces from hijacking traffic
2. **Principle of Least Privilege**: Default `Same` policy provides maximum security
3. **Explicit Configuration**: Administrators must explicitly allow cross-namespace routing
4. **Audit Trail**: All filtering decisions are logged for security auditing

### Future Enhancements

1. **Namespace Selector Implementation**: Support for label-based namespace selection
2. **Route Status Updates**: Update HTTPRoute status to indicate rejection reasons
3. **Metrics**: Add Prometheus metrics for route filtering decisions
4. **Webhook Integration**: Optional admission controller for early validation

### Architecture Notes

The namespace filtering is implemented entirely within the control plane:

- **Early Filtering**: Routes are filtered before configuration generation
- **No Runtime Impact**: Rejected routes never reach the gateway runtime
- **Controller Pattern**: Uses the existing reactive controller architecture
- **Signal-based**: Integrates with the existing signal-based state management

This ensures that unauthorized routes are completely excluded from the system, providing both security and performance
benefits.
