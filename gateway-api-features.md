# Gateway API HTTP Features - Kubera Support Status

This document provides a comprehensive overview of Gateway API HTTP features and their current implementation status in
Kubera Gateway.

## Core HTTP Routing Features

### HTTPRoute Resource

| Feature               | Status          | Description                                        | Conformance Level | Test Coverage | Level of Effort |
|-----------------------|-----------------|----------------------------------------------------|-------------------|---------------|-----------------|
| **HTTPRoute CRD**     | ✅ **Supported** | Basic HTTPRoute resource definition and processing | ⭐ **Core**        | 🟡 **Medium** | Complete        |
| **Parent References** | ✅ **Supported** | Attaching routes to Gateway listeners              | ⭐ **Core**        | 🟡 **Medium** | Complete        |
| **Multiple Rules**    | ✅ **Supported** | Multiple routing rules within a single HTTPRoute   | ⭐ **Core**        | 🟡 **Medium** | Complete        |

## Path Matching

| Feature               | Status          | Description                           | Documentation                                                                                                            | Conformance Level              | Test Coverage | Level of Effort |
|-----------------------|-----------------|---------------------------------------|--------------------------------------------------------------------------------------------------------------------------|--------------------------------|---------------|-----------------|
| **PathPrefix**        | ✅ **Supported** | Match requests with path prefix       | [Gateway API Path Matching](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.HTTPPathMatch) | ⭐ **Core**                     | 🟡 **Medium** | Complete        |
| **Exact Path**        | ✅ **Supported** | Match exact path only                 | [Gateway API Path Matching](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.HTTPPathMatch) | ⭐ **Core**                     | 🟡 **Medium** | Complete        |
| **RegularExpression** | ✅ **Supported** | Match paths using regular expressions | [Gateway API Path Matching](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.HTTPPathMatch) | 🔧 **Implementation Specific** | 🟡 **Medium** | Complete        |

### Implementation Notes

- Path matching is fully implemented with all Gateway API path match types
- Exact matching uses direct string comparison
- Prefix matching uses `starts_with()` logic
- RegularExpression matching uses a regex engine for pattern matching
- Proper scoring and instrumentation for match tracking

## Header Matching

| Feature                      | Status          | Description                        | Documentation                                                                                                                | Conformance Level              | Test Coverage | Level of Effort |
|------------------------------|-----------------|------------------------------------|------------------------------------------------------------------------------------------------------------------------------|--------------------------------|---------------|-----------------|
| **Exact Header Match**       | ✅ **Supported** | Match headers with exact values    | [Gateway API Header Matching](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.HTTPHeaderMatch) | ⭐ **Core**                     | 🟡 **Medium** | Complete        |
| **RegularExpression Header** | ✅ **Supported** | Match headers using regex patterns | [Gateway API Header Matching](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.HTTPHeaderMatch) | 🔧 **Implementation Specific** | 🟡 **Medium** | Complete        |

### Implementation Notes

- Header matching is fully implemented with both exact and regex support
- Supports multiple header matches (all must pass)
- Proper HTTP header handling with HeaderMap, HeaderName, and HeaderValue
- Regex matching uses a regex engine for pattern matching

## Query Parameter Matching

| Feature                     | Status          | Description                              | Documentation                                                                                                                   | Conformance Level              | Test Coverage | Level of Effort |
|-----------------------------|-----------------|------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------|--------------------------------|---------------|-----------------|
| **Exact Query Match**       | ✅ **Supported** | Match query parameters with exact values | [Gateway API Query Matching](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.HTTPQueryParamMatch) | 🟠 **Extended**                | 🟡 **Medium** | Complete        |
| **RegularExpression Query** | ✅ **Supported** | Match query parameters using regex       | [Gateway API Query Matching](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.HTTPQueryParamMatch) | 🔧 **Implementation Specific** | 🟡 **Medium** | Complete        |

### Implementation Notes

- Query parameter matching is fully implemented with both exact and regex support
- Supports multiple query parameter matches (all must pass)
- Regex matching uses a regex engine for pattern matching

## HTTP Method Matching

| Feature             | Status          | Description                                   | Documentation                                                                                                               | Conformance Level | Test Coverage | Level of Effort |
|---------------------|-----------------|-----------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------|-------------------|---------------|-----------------|
| **Method Matching** | ✅ **Supported** | Match specific HTTP methods (GET, POST, etc.) | [Gateway API Method Matching](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.HTTPRouteMatch) | 🟠 **Extended**   | 🟡 **Medium** | Complete        |

## HTTP Filters

### Header Modification

| Feature                    | Status          | Description                       | Documentation                                                                                                              | Conformance Level | Test Coverage | Level of Effort |
|----------------------------|-----------------|-----------------------------------|----------------------------------------------------------------------------------------------------------------------------|-------------------|---------------|-----------------|
| **RequestHeaderModifier**  | ✅ **Supported** | Set, add, remove request headers  | [Request Header Modifier](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.HTTPHeaderFilter)  | ⭐ **Core**        | 🟡 **Medium** | Complete        |
| **ResponseHeaderModifier** | ✅ **Supported** | Set, add, remove response headers | [Response Header Modifier](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.HTTPHeaderFilter) | 🟠 **Extended**   | 🟡 **Medium** | Complete        |

### Traffic Management

| Feature             | Status          | Description                              | Documentation                                                                                                               | Conformance Level              | Test Coverage | Level of Effort      |
|---------------------|-----------------|------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------|--------------------------------|---------------|----------------------|
| **RequestRedirect** | ✅ **Supported** | HTTP redirects (301, 302)                | [Request Redirect](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.HTTPRequestRedirectFilter) | 🟠 **Extended**                | 🟢 **High**   | Complete             |
| **URLRewrite**      | ✅ **Supported** | Rewrite URLs before forwarding           | [URL Rewrite](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.HTTPURLRewriteFilter)           | 🟠 **Extended**                | 🟢 **High**   | Complete             |
| **StaticResponse**  | ✅ **Supported** | Return static responses without upstream | Custom Kubera extension for maintenance pages, error responses, and testing                                                 | 🔧 **Implementation Specific** | 🟡 **Medium** | Complete             |
| **RequestMirror**   | 🏗️ **Defined** | Mirror requests to additional backends   | [Request Mirror](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.HTTPRequestMirrorFilter)     | 🟠 **Extended**                | 🔴 **None**   | **High** (3-4 weeks) |

### Implementation Notes

- **RequestRedirect**: Fully implemented with Gateway API to Kubera config conversion
    - Supports scheme redirection (HTTP to HTTPS)
    - Hostname and port redirection
    - Path rewriting (full path replacement and prefix matching)
    - Status codes 301 (permanent) and 302 (temporary) redirect
    - Proper URL construction using `url::Url` type
    - Complete test coverage with 6/6 tests passing
- **URLRewrite**: Fully implemented with Gateway API to Kubera config conversion
    - Supports hostname rewriting for internal service routing
    - Path rewriting (full path replacement and prefix matching)
    - Query parameter preservation during rewrites
    - Reuses RouteMatchContext from redirect implementation for consistency
    - Proper Pingora integration with request header modifications
    - Complete test coverage with 9/9 tests passing
    - Applied after redirect checks but before upstream forwarding
- **StaticResponse**: Fully implemented as a custom Kubera extension for maintenance pages and error responses
    - Supports configurable HTTP status codes (200, 404, 503, etc.)
    - Custom response bodies with configurable Content-Type headers
    - Key-based lookup system for response configuration management
    - Integrated with Gateway API filter chain with highest precedence
    - Proper Pingora session integration with direct response writing
    - Uses identifier field as response body content (extensible for file loading)
    - Comprehensive logging and error handling with debug/warn levels
    - Applied before redirect and rewrite filters in the processing pipeline
- **RequestMirror**: Basic filter framework is in place with placeholder structures
- Request mirroring needs Pingora integration for async request duplication

## Backend References

| Feature                        | Status              | Description                            | Documentation                                                                                                  | Conformance Level | Test Coverage | Level of Effort        |
|--------------------------------|---------------------|----------------------------------------|----------------------------------------------------------------------------------------------------------------|-------------------|---------------|------------------------|
| **Service Backend**            | ✅ **Supported**     | Route to Kubernetes Services           | [Backend References](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.BackendRef) | ⭐ **Core**        | 🟢 **High**   | Complete               |
| **Weight-based Routing**       | ❌ **Not Supported** | Distribute traffic by weight           | [Backend References](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.BackendRef) | 🟠 **Extended**   | 🔴 **None**   | **Medium** (2-3 weeks) |
| **Cross-namespace References** | ❌ **Not Supported** | Reference services in other namespaces | [Backend References](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.BackendRef) | 🟠 **Extended**   | 🔴 **None**   | **Low** (1 week)       |

## Advanced Features

### Traffic Splitting

| Feature                   | Status              | Description                            | Documentation                                                                  | Conformance Level | Test Coverage | Level of Effort        |
|---------------------------|---------------------|----------------------------------------|--------------------------------------------------------------------------------|-------------------|---------------|------------------------|
| **Multiple Backend Refs** | ❌ **Not Supported** | Split traffic across multiple services | [Traffic Splitting](https://gateway-api.sigs.k8s.io/guides/traffic-splitting/) | 🟠 **Extended**   | 🔴 **None**   | **Medium** (2-3 weeks) |
| **Weighted Traffic**      | ❌ **Not Supported** | Weighted load balancing                | [Traffic Splitting](https://gateway-api.sigs.k8s.io/guides/traffic-splitting/) | 🟠 **Extended**   | 🔴 **None**   | **Medium** (2-3 weeks) |

### Timeouts and Retries

| Feature             | Status              | Description                | Documentation                                              | Conformance Level   | Test Coverage | Level of Effort        |
|---------------------|---------------------|----------------------------|------------------------------------------------------------|---------------------|---------------|------------------------|
| **Request Timeout** | ❌ **Not Supported** | Configure request timeouts | [Timeouts](https://gateway-api.sigs.k8s.io/geps/gep-1742/) | 🧪 **Experimental** | 🔴 **None**   | **Low** (1 week)       |
| **Retry Policy**    | ❌ **Not Supported** | Automatic request retries  | [GEP-1731](https://gateway-api.sigs.k8s.io/geps/gep-1731/) | 🧪 **Experimental** | 🔴 **None**   | **Medium** (2-3 weeks) |

### Extension Points

| Feature                 | Status          | Description              | Documentation                                                                                                  | Conformance Level | Test Coverage | Level of Effort      |
|-------------------------|-----------------|--------------------------|----------------------------------------------------------------------------------------------------------------|-------------------|---------------|----------------------|
| **ExtensionRef Filter** | 🏗️ **Defined** | Custom filter extensions | [Extension Points](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.ExtensionRef) | 🟠 **Extended**   | 🔴 **None**   | **High** (4-6 weeks) |

## Protocol Features

### HTTP Version Support

| Feature      | Status              | Description               | Documentation                                                                                                  | Conformance Level | Test Coverage | Level of Effort                 |
|--------------|---------------------|---------------------------|----------------------------------------------------------------------------------------------------------------|-------------------|---------------|---------------------------------|
| **HTTP/1.1** | ✅ **Supported**     | HTTP/1.1 protocol support | [Protocol Support](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.ProtocolType) | ⭐ **Core**        | 🟢 **High**   | Complete                        |
| **HTTP/2**   | 🚧 **Unknown**      | HTTP/2 protocol support   | [Protocol Support](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.ProtocolType) | 🟠 **Extended**   | 🔴 **None**   | **Medium** (depends on Pingora) |
| **HTTP/3**   | ❌ **Not Supported** | HTTP/3 protocol support   | [Protocol Support](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.ProtocolType) | 🟠 **Extended**   | 🔴 **None**   | **High** (depends on Pingora)   |

## Gateway Features

### Listeners

| Feature                | Status              | Description                    | Documentation                                                                                               | Conformance Level | Test Coverage | Level of Effort        |
|------------------------|---------------------|--------------------------------|-------------------------------------------------------------------------------------------------------------|-------------------|---------------|------------------------|
| **HTTP Listener**      | ✅ **Supported**     | Basic HTTP listeners           | [Gateway Listeners](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.Listener) | ⭐ **Core**        | 🟢 **High**   | Complete               |
| **HTTPS Listener**     | ❌ **Not Supported** | TLS-terminated HTTPS           | [Gateway Listeners](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.Listener) | ⭐ **Core**        | 🔴 **None**   | **High** (3-4 weeks)   |
| **Multiple Listeners** | 🚧 **Unknown**      | Multiple listeners per Gateway | [Gateway Listeners](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.Listener) | ⭐ **Core**        | 🟡 **Medium** | **Medium** (2-3 weeks) |

### TLS Features

| Feature             | Status              | Description                 | Documentation                                                    | Conformance Level | Test Coverage | Level of Effort        |
|---------------------|---------------------|-----------------------------|------------------------------------------------------------------|-------------------|---------------|------------------------|
| **TLS Termination** | ❌ **Not Supported** | Terminate TLS at gateway    | [TLS Configuration](https://gateway-api.sigs.k8s.io/guides/tls/) | ⭐ **Core**        | 🔴 **None**   | **High** (4-5 weeks)   |
| **TLS Passthrough** | ❌ **Not Supported** | Pass TLS through to backend | [TLS Configuration](https://gateway-api.sigs.k8s.io/guides/tls/) | 🟠 **Extended**   | 🔴 **None**   | **Medium** (2-3 weeks) |
| **SNI Routing**     | ❌ **Not Supported** | Route based on SNI          | [TLS Configuration](https://gateway-api.sigs.k8s.io/guides/tls/) | 🟠 **Extended**   | 🔴 **None**   | **High** (3-4 weeks)   |

## Status and Observability

### Route Status

| Feature                     | Status              | Description                       | Documentation                                                                                                  | Conformance Level | Test Coverage | Level of Effort  |
|-----------------------------|---------------------|-----------------------------------|----------------------------------------------------------------------------------------------------------------|-------------------|---------------|------------------|
| **Route Status Reporting**  | 🚧 **Partial**      | Report route acceptance/rejection | [Status Conditions](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.RouteStatus) | ⭐ **Core**        | 🟡 **Medium** | **Low** (1 week) |
| **Detailed Error Messages** | ❌ **Not Supported** | Detailed validation errors        | [Status Conditions](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.RouteStatus) | ⭐ **Core**        | 🔴 **None**   | **Low** (1 week) |

### Gateway Status

| Feature             | Status              | Description                | Documentation                                                                                                 | Conformance Level | Test Coverage | Level of Effort  |
|---------------------|---------------------|----------------------------|---------------------------------------------------------------------------------------------------------------|-------------------|---------------|------------------|
| **Gateway Status**  | 🚧 **Partial**      | Report gateway readiness   | [Gateway Status](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.GatewayStatus) | ⭐ **Core**        | 🟡 **Medium** | **Low** (1 week) |
| **Listener Status** | ❌ **Not Supported** | Individual listener status | [Gateway Status](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1.GatewayStatus) | ⭐ **Core**        | 🔴 **None**   | **Low** (1 week) |

## Future/Experimental Features

### Gateway API Extensions

| Feature       | Status              | Description           | Documentation                                                                                              | Conformance Level   | Test Coverage | Level of Effort      |
|---------------|---------------------|-----------------------|------------------------------------------------------------------------------------------------------------|---------------------|---------------|----------------------|
| **GRPCRoute** | ❌ **Not Supported** | gRPC-specific routing | [GRPCRoute](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1alpha2.GRPCRoute) | 🧪 **Experimental** | 🔴 **None**   | **High** (4-6 weeks) |
| **TCPRoute**  | ❌ **Not Supported** | TCP-level routing     | [TCPRoute](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1alpha2.TCPRoute)   | 🧪 **Experimental** | 🔴 **None**   | **High** (4-6 weeks) |
| **UDPRoute**  | ❌ **Not Supported** | UDP-level routing     | [UDPRoute](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1alpha2.UDPRoute)   | 🧪 **Experimental** | 🔴 **None**   | **High** (4-6 weeks) |
| **TLSRoute**  | ❌ **Not Supported** | TLS SNI-based routing | [TLSRoute](https://gateway-api.sigs.k8s.io/references/spec/#gateway.networking.k8s.io/v1alpha2.TLSRoute)   | 🧪 **Experimental** | 🔴 **None**   | **High** (4-6 weeks) |

## Legend

- ✅ **Supported**: Feature is fully implemented and working
- 🚧 **Partial**: Feature is partially implemented or has limitations
- 🏗️ **Defined**: Data structures exist but functionality not implemented
- ❌ **Not Supported**: Feature is not implemented

**Test Coverage Legend:**

- 🟢 **High**: Comprehensive test coverage with unit, integration, and edge case tests
- 🟡 **Medium**: Basic test coverage with some gaps in edge cases or integration scenarios
- 🔴 **None**: No test coverage or minimal testing

**Conformance Level Legend:**

- ⭐ **Core**: Required for basic Gateway API conformance - must be implemented
- 🟠 **Extended**: Additional features beyond core conformance - recommended for production use
- 🔧 **Implementation Specific**: Features that implementations may support differently
- 🧪 **Experimental**: Early-stage features that may change or be removed

## Implementation Priorities

### High Priority (Core HTTP Gateway)

1. Weight-based routing
2. Request timeout configuration
3. Route status reporting

### Medium Priority (Advanced Routing)

1. Cross-namespace backend references
2. Multiple backend refs with traffic splitting
3. HTTPS/TLS support

### Low Priority (Advanced Features)

1. RequestMirror filter
2. HTTP/2 support
3. GRPCRoute support
4. Extension points

## Development Notes

- **Pingora Integration**: Many features depend on Pingora proxy capabilities
- **Kubernetes API**: Status reporting requires enhanced controller logic
- **Performance**: Regex matching and complex routing may impact performance
- **Security**: TLS and authentication features require careful security review
- **Testing**: Each feature needs comprehensive integration testing

## References

- [Gateway API Specification](https://gateway-api.sigs.k8s.io/references/spec/)
- [Gateway API User Guides](https://gateway-api.sigs.k8s.io/guides/)
- [Gateway API Enhancement Proposals (GEPs)](https://gateway-api.sigs.k8s.io/geps/)
- [Pingora Documentation](https://github.com/cloudflare/pingora)
