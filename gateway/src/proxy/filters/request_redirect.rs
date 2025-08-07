use http::{StatusCode, Uri};
use kubera_core::config::gateway::types::http::filters::RequestRedirect;
use kubera_core::sync::signal::{signal, Receiver};
use kubera_core::task::Builder as TaskBuilder;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error, warn};
use url::Url;

/// Filter for handling HTTP request redirects based on RequestRedirect configuration.
///
/// This filter supports the Gateway API `RequestRedirect` filter specification, including
/// the `replacePrefixMatch` functionality that allows redirecting requests while replacing
/// the matched prefix with a new one.
///
/// # Supported Features
///
/// - **Scheme redirection**: Change HTTP to HTTPS or vice versa
/// - **Hostname redirection**: Redirect to a different host
/// - **Port redirection**: Redirect to a different port
/// - **Path rewriting**: Support for both `ReplaceFullPath` and `ReplacePrefixMatch`
/// - **Status code control**: Configurable redirect status codes (301, 302, etc.)
/// - **Query preservation**: Query parameters are preserved in redirects
///
/// # Path Rewriting Types
///
/// ## ReplaceFullPath
/// Replaces the entire request path with a fixed value.
///
/// ## ReplacePrefixMatch
/// Replaces the matched prefix from the route with a new prefix, preserving the rest
/// of the path. This only works with prefix-based route matching.
///
/// # Examples
///
/// ```rust,ignore
/// use kubera_core::config::gateway::types::http::filters::{RequestRedirect, PathRewrite, PathRewriteType};
///
/// // Basic redirect to HTTPS
/// let redirect = RequestRedirect {
///     scheme: Some("https".to_string()),
///     hostname: None,
///     port: None,
///     path: None,
///     status_code: Some(301),
/// };
///
/// // Redirect with prefix replacement
/// let redirect = RequestRedirect {
///     scheme: Some("https".to_string()),
///     hostname: Some("api.example.com".to_string()),
///     port: None,
///     path: Some(PathRewrite {
///         rewrite_type: PathRewriteType::ReplacePrefixMatch,
///         replace_full_path: None,
///         replace_prefix_match: Some("/v2".to_string()),
///     }),
///     status_code: Some(301),
/// };
///
/// let filter = RequestRedirectFilter::new(redirect);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RequestRedirectFilter {
    redirect: Arc<RequestRedirect>,
}

/// Context information about the matched route, used for prefix replacement.
///
/// This struct carries information from the routing system about how the request
/// was matched, which is essential for implementing `replacePrefixMatch` correctly.
///
/// # Fields
///
/// * `matched_prefix` - The prefix that was matched from the route configuration.
///   This is only populated for prefix-based route matches and is used for
///   `replacePrefixMatch` functionality.
///
/// # Examples
///
/// ```rust,ignore
/// // For a route with prefix "/api/v1" matching request "/api/v1/users"
/// let context = RouteMatchContext {
///     matched_prefix: Some("/api/v1".to_string()),
/// };
///
/// // With replacePrefixMatch configured as "/v2"
/// // The result would be "/v2/users"
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RouteMatchContext {
    /// The prefix that was matched from the route configuration.
    ///
    /// - Some("/prefix") for prefix matches where the request path starts with "/prefix"
    /// - None for exact matches, regex matches, or when no context is available
    pub matched_prefix: Option<String>,
}

impl RequestRedirectFilter {
    /// Creates a new request redirect filter.
    ///
    /// # Arguments
    ///
    /// * `redirect` - The redirect configuration specifying how to redirect requests
    pub fn new(redirect: RequestRedirect) -> Self {
        Self {
            redirect: Arc::new(redirect),
        }
    }

    /// Check if this request should be redirected and return the redirect response.
    ///
    /// This is a convenience method that uses empty route context (no matched prefix).
    /// For full `replacePrefixMatch` support, use `should_redirect_with_context`.
    ///
    /// # Arguments
    ///
    /// * `request_uri` - The original request URI to potentially redirect
    ///
    /// # Returns
    ///
    /// Some(RedirectResponse) if a redirect should be performed, None otherwise.
    /// Currently always returns Some since this filter always redirects when configured.
    #[allow(dead_code)] // Public API for convenience
    pub fn should_redirect(&self, request_uri: &Uri) -> Option<RedirectResponse> {
        self.should_redirect_with_context(
            request_uri,
            &RouteMatchContext {
                matched_prefix: None,
            },
        )
    }

    /// Check if this request should be redirected with route match context.
    ///
    /// This method supports full `replacePrefixMatch` functionality by using the
    /// matched prefix information from the routing system.
    ///
    /// # Arguments
    ///
    /// * `request_uri` - The original request URI to potentially redirect
    /// * `context` - Route match context containing matched prefix information
    ///
    /// # Returns
    ///
    /// Some(RedirectResponse) with the redirect location and status code.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let context = RouteMatchContext {
    ///     matched_prefix: Some("/api/v1".to_string()),
    /// };
    ///
    /// let original_uri = Uri::from_str("http://example.com/api/v1/users").unwrap();
    /// let redirect = filter.should_redirect_with_context(&original_uri, &context);
    ///
    /// // With replacePrefixMatch: "/v2", this would redirect to:
    /// // "https://api.example.com/v2/users"
    /// ```
    pub fn should_redirect_with_context(
        &self,
        request_uri: &Uri,
        context: &RouteMatchContext,
    ) -> Option<RedirectResponse> {
        // Build the redirect URL based on the configuration
        let redirect_uri = self.build_redirect_url(request_uri, context);

        // Determine the status code (default to 302 if not specified)
        let status_code = self
            .redirect
            .status_code
            .and_then(|code| StatusCode::from_u16(code).ok())
            .unwrap_or(StatusCode::FOUND); // 302 Found is the default

        Some(RedirectResponse {
            location: redirect_uri.to_string(),
            status_code,
        })
    }

    /// Build the redirect URL based on the configuration and original request URI
    fn build_redirect_url(&self, original_uri: &Uri, context: &RouteMatchContext) -> Uri {
        let mut redirect_url = Url::parse("http://localhost").unwrap();

        // Scheme (protocol)
        let scheme = self
            .redirect
            .scheme
            .as_deref()
            .unwrap_or_else(|| original_uri.scheme_str().unwrap_or("http"));
        redirect_url.set_scheme(scheme).unwrap();

        // Hostname
        let hostname = self
            .redirect
            .hostname
            .as_deref()
            .unwrap_or_else(|| original_uri.host().unwrap_or("localhost"));
        redirect_url.set_host(Some(hostname)).unwrap();

        // Port
        if let Some(port) = self.redirect.port {
            // Only add port if it's not the default for the scheme
            let default_port = match scheme {
                "https" => 443,
                "http" => 80,
                _ => 80,
            };
            if port != default_port {
                redirect_url.set_port(Some(port)).unwrap();
            }
        } else if let Some(port) = original_uri.port_u16() {
            redirect_url.set_port(Some(port)).unwrap();
        }

        // Path
        let path = if let Some(path_config) = &self.redirect.path {
            self.build_redirect_path(original_uri.path(), path_config, context)
        } else {
            original_uri.path().to_string()
        };
        redirect_url.set_path(&path);

        // Query string (preserve from original request)
        if let Some(query) = original_uri.query() {
            redirect_url.set_query(Some(query));
        }

        debug!("Built redirect URI: {} -> {}", original_uri, redirect_url);

        // Convert Url back to Uri
        Uri::from_str(redirect_url.as_str()).unwrap_or_else(|_| {
            warn!("Failed to convert redirect URL to URI, using fallback");
            Uri::from_str("http://localhost/").unwrap()
        })
    }

    /// Build the redirect path based on the configuration
    fn build_redirect_path(
        &self,
        original_path: &str,
        path_config: &kubera_core::config::gateway::types::http::filters::PathRewrite,
        context: &RouteMatchContext,
    ) -> String {
        use kubera_core::config::gateway::types::http::filters::PathRewriteType;

        match path_config.rewrite_type {
            PathRewriteType::ReplaceFullPath => path_config
                .replace_full_path
                .as_deref()
                .unwrap_or("/")
                .to_string(),
            PathRewriteType::ReplacePrefixMatch => {
                if let Some(replacement) = &path_config.replace_prefix_match {
                    self.replace_prefix_match(original_path, replacement, context)
                } else {
                    warn!("ReplacePrefixMatch configured but no replacement value provided");
                    original_path.to_string()
                }
            }
        }
    }

    /// Replace the matched prefix with the configured replacement
    fn replace_prefix_match(
        &self,
        original_path: &str,
        replacement: &str,
        context: &RouteMatchContext,
    ) -> String {
        if let Some(matched_prefix) = &context.matched_prefix {
            // Ensure the original path actually starts with the matched prefix
            if original_path.starts_with(matched_prefix) {
                // Replace the matched prefix with the replacement
                let remaining_path = &original_path[matched_prefix.len()..];

                // Ensure proper path concatenation
                let new_path = if replacement.ends_with('/') && remaining_path.starts_with('/') {
                    // Avoid double slashes: /api/ + /v1/users -> /api/v1/users
                    format!("{}{}", replacement.trim_end_matches('/'), remaining_path)
                } else if !replacement.ends_with('/')
                    && !remaining_path.starts_with('/')
                    && !remaining_path.is_empty()
                {
                    // Ensure there's a slash between segments: /api + v1/users -> /api/v1/users
                    format!("{replacement}/{remaining_path}")
                } else {
                    // Direct concatenation is fine
                    format!("{replacement}{remaining_path}")
                };

                debug!(
                    "Replaced prefix '{}' with '{}' in path '{}' -> '{}'",
                    matched_prefix, replacement, original_path, new_path
                );

                new_path
            } else {
                warn!(
                    "Original path '{}' does not start with matched prefix '{}', returning original path",
                    original_path, matched_prefix
                );
                original_path.to_string()
            }
        } else {
            warn!(
                "ReplacePrefixMatch requested but no matched prefix provided in context, using replacement as-is"
            );
            // Fall back to using the replacement as the full path
            replacement.to_string()
        }
    }

    /// Apply redirect logic to Pingora's request context
    #[allow(dead_code)] // Public API for convenience
    pub fn apply_to_pingora_request(
        &self,
        request_header: &pingora::http::RequestHeader,
    ) -> Result<Option<RedirectResponse>, Box<dyn std::error::Error + Send + Sync>> {
        self.apply_to_pingora_request_with_context(
            request_header,
            &RouteMatchContext {
                matched_prefix: None,
            },
        )
    }

    /// Apply redirect logic to Pingora's request context with route match context
    pub fn apply_to_pingora_request_with_context(
        &self,
        request_header: &pingora::http::RequestHeader,
        context: &RouteMatchContext,
    ) -> Result<Option<RedirectResponse>, Box<dyn std::error::Error + Send + Sync>> {
        // Build URI from the request header
        let uri_str = format!(
            "{}://{}{}",
            request_header
                .uri
                .scheme()
                .map(|s| s.as_str())
                .unwrap_or("http"),
            request_header.uri.host().unwrap_or("localhost"),
            request_header
                .uri
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or("/")
        );

        match Uri::from_str(&uri_str) {
            Ok(uri) => Ok(self.should_redirect_with_context(&uri, context)),
            Err(e) => {
                error!("Failed to parse URI from request: {}", e);
                Err(Box::new(e))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RedirectResponse {
    pub location: String,
    pub status_code: StatusCode,
}

#[allow(dead_code)] // Public API for future configuration watching
pub fn create_request_redirect_filter_receiver(
    task_builder: &TaskBuilder,
    initial_redirect: Option<RequestRedirect>,
) -> Receiver<Option<RequestRedirectFilter>> {
    let (sender, receiver) = signal();

    task_builder
        .new_task("request_redirect_filter_config_watcher")
        .spawn(async move {
            // Send initial configuration
            if let Some(redirect) = &initial_redirect {
                let filter = Some(RequestRedirectFilter::new(redirect.clone()));
                sender.set(filter).await;
                debug!("RequestRedirectFilter configuration updated");
            } else {
                sender.set(None).await;
            }

            // In a real implementation, this would watch for configuration changes
            // For now, we'll just send the initial configuration once and exit
        });

    receiver
}

#[cfg(test)]
mod tests {
    use super::*;
    use kubera_core::config::gateway::types::http::filters::{PathRewrite, PathRewriteType};

    #[test]
    fn test_basic_redirect() {
        let redirect_config = RequestRedirect {
            scheme: Some("https".to_string()),
            hostname: Some("example.com".to_string()),
            path: None,
            port: None,
            status_code: Some(301),
        };

        let filter = RequestRedirectFilter::new(redirect_config);
        let original_uri = Uri::from_str("http://localhost:8080/test?param=value").unwrap();

        let redirect = filter.should_redirect(&original_uri).unwrap();
        assert_eq!(
            redirect.location,
            "https://example.com:8080/test?param=value"
        );
        assert_eq!(redirect.status_code, StatusCode::MOVED_PERMANENTLY);
    }

    #[test]
    fn test_path_replacement() {
        let redirect_config = RequestRedirect {
            scheme: None,
            hostname: None,
            path: Some(PathRewrite {
                rewrite_type: PathRewriteType::ReplaceFullPath,
                replace_full_path: Some("/new-path".to_string()),
                replace_prefix_match: None,
            }),
            port: None,
            status_code: None,
        };

        let filter = RequestRedirectFilter::new(redirect_config);
        let original_uri = Uri::from_str("http://localhost/old-path").unwrap();

        let redirect = filter.should_redirect(&original_uri).unwrap();
        assert_eq!(redirect.location, "http://localhost/new-path");
        assert_eq!(redirect.status_code, StatusCode::FOUND); // Default 302
    }

    #[test]
    fn test_port_handling() {
        let redirect_config = RequestRedirect {
            scheme: Some("https".to_string()),
            hostname: Some("example.com".to_string()),
            path: None,
            port: Some(443), // Default HTTPS port
            status_code: None,
        };

        let filter = RequestRedirectFilter::new(redirect_config);
        let original_uri = Uri::from_str("http://localhost:8080/test").unwrap();

        let redirect = filter.should_redirect(&original_uri).unwrap();
        // Should not include :443 since it's the default HTTPS port
        assert_eq!(redirect.location, "https://example.com/test");
    }

    #[test]
    fn test_replace_prefix_match_basic() {
        let redirect = RequestRedirect {
            status_code: Some(301),
            scheme: None,
            hostname: None,
            port: None,
            path: Some(PathRewrite {
                rewrite_type: PathRewriteType::ReplacePrefixMatch,
                replace_full_path: None,
                replace_prefix_match: Some("/v2".to_string()),
            }),
        };

        let filter = RequestRedirectFilter::new(redirect);
        let context = RouteMatchContext {
            matched_prefix: Some("/api/v1".to_string()),
        };

        let result = filter.replace_prefix_match("/api/v1/users", "/v2", &context);
        assert_eq!(result, "/v2/users");
    }

    #[test]
    fn test_replace_prefix_match_with_trailing_slash() {
        let redirect = RequestRedirect {
            status_code: Some(301),
            scheme: None,
            hostname: None,
            port: None,
            path: Some(PathRewrite {
                rewrite_type: PathRewriteType::ReplacePrefixMatch,
                replace_full_path: None,
                replace_prefix_match: Some("/v2/".to_string()),
            }),
        };

        let filter = RequestRedirectFilter::new(redirect);
        let context = RouteMatchContext {
            matched_prefix: Some("/api/v1/".to_string()),
        };

        let result = filter.replace_prefix_match("/api/v1/users", "/v2/", &context);
        assert_eq!(result, "/v2/users");
    }

    #[test]
    fn test_replace_prefix_match_exact_match() {
        let redirect = RequestRedirect {
            status_code: Some(301),
            scheme: None,
            hostname: None,
            port: None,
            path: Some(PathRewrite {
                rewrite_type: PathRewriteType::ReplacePrefixMatch,
                replace_full_path: None,
                replace_prefix_match: Some("/v2".to_string()),
            }),
        };

        let filter = RequestRedirectFilter::new(redirect);
        let context = RouteMatchContext {
            matched_prefix: Some("/api/v1".to_string()),
        };

        let result = filter.replace_prefix_match("/api/v1", "/v2", &context);
        assert_eq!(result, "/v2");
    }

    #[test]
    fn test_replace_prefix_match_no_context() {
        let redirect = RequestRedirect {
            status_code: Some(301),
            scheme: None,
            hostname: None,
            port: None,
            path: Some(PathRewrite {
                rewrite_type: PathRewriteType::ReplacePrefixMatch,
                replace_full_path: None,
                replace_prefix_match: Some("/v2".to_string()),
            }),
        };

        let filter = RequestRedirectFilter::new(redirect);
        let context = RouteMatchContext {
            matched_prefix: None,
        };

        let result = filter.replace_prefix_match("/api/v1/users", "/v2", &context);
        assert_eq!(result, "/v2");
    }

    #[test]
    fn test_full_redirect_with_replace_prefix_match() {
        let redirect = RequestRedirect {
            status_code: Some(301),
            scheme: Some("https".to_string()),
            hostname: Some("api.example.com".to_string()),
            port: Some(443), // Explicitly set HTTPS default port to avoid showing :443
            path: Some(PathRewrite {
                rewrite_type: PathRewriteType::ReplacePrefixMatch,
                replace_full_path: None,
                replace_prefix_match: Some("/v3".to_string()),
            }),
        };

        let filter = RequestRedirectFilter::new(redirect);
        let context = RouteMatchContext {
            matched_prefix: Some("/api/v1".to_string()),
        };

        let original_uri = Uri::from_str("http://localhost:8080/api/v1/users?param=value").unwrap();
        let result = filter
            .should_redirect_with_context(&original_uri, &context)
            .unwrap();

        assert_eq!(result.status_code, StatusCode::MOVED_PERMANENTLY);
        assert_eq!(
            result.location,
            "https://api.example.com/v3/users?param=value"
        );
    }
}
