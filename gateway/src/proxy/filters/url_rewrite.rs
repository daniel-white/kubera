use http::Uri;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error, warn};
use url::Url;
use vg_core::config::gateway::types::http::filters::URLRewrite;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;

use super::request_redirect::RouteMatchContext;

/// Filter for handling HTTP URL rewriting based on URLRewrite configuration.
///
/// This filter supports the Gateway API `URLRewrite` filter specification, allowing
/// modification of the request URL before it's sent to the upstream backend.
/// Unlike redirects, URL rewrites modify the request internally without sending
/// a redirect response to the client.
///
/// # Supported Features
///
/// - **Hostname rewriting**: Change the target hostname for upstream requests
/// - **Path rewriting**: Support for both `ReplaceFullPath` and `ReplacePrefixMatch`
/// - **Query preservation**: Query parameters are preserved during rewrites
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
/// use vg_core::config::gateway::types::http::filters::{URLRewrite, PathRewrite, PathRewriteType};
///
/// // Basic path rewrite
/// let rewrite = URLRewrite {
///     hostname: Some("internal-api.example.com".to_string()),
///     path: Some(PathRewrite {
///         rewrite_type: PathRewriteType::ReplaceFullPath,
///         replace_full_path: Some("/v2/api".to_string()),
///         replace_prefix_match: None,
///     }),
/// };
///
/// // Prefix-based rewrite
/// let rewrite = URLRewrite {
///     hostname: None,
///     path: Some(PathRewrite {
///         rewrite_type: PathRewriteType::ReplacePrefixMatch,
///         replace_full_path: None,
///         replace_prefix_match: Some("/internal".to_string()),
///     }),
/// };
///
/// let filter = URLRewriteFilter::new(rewrite);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct URLRewriteFilter {
    rewrite: Arc<URLRewrite>,
}

/// Result of applying a URL rewrite filter
#[derive(Debug, Clone, PartialEq)]
pub struct RewriteResult {
    /// The rewritten URI
    pub uri: Uri,
    /// Whether the URI was actually modified
    pub modified: bool,
}

impl URLRewriteFilter {
    /// Creates a new URL rewrite filter.
    ///
    /// # Arguments
    ///
    /// * `rewrite` - The URL rewrite configuration
    pub fn new(rewrite: URLRewrite) -> Self {
        Self {
            rewrite: Arc::new(rewrite),
        }
    }

    /// Apply URL rewrite to the request URI.
    ///
    /// This is a convenience method that uses empty route context (no matched prefix).
    /// For full `replacePrefixMatch` support, use `rewrite_with_context`.
    ///
    /// # Arguments
    ///
    /// * `request_uri` - The original request URI to rewrite
    ///
    /// # Returns
    ///
    /// RewriteResult containing the rewritten URI and whether it was modified.
    #[allow(dead_code)] // Public API for future use
    pub fn rewrite(&self, request_uri: &Uri) -> RewriteResult {
        self.rewrite_with_context(
            request_uri,
            &RouteMatchContext {
                matched_prefix: None,
            },
        )
    }

    /// Apply URL rewrite with route match context.
    ///
    /// This method supports full `replacePrefixMatch` functionality by using the
    /// matched prefix information from the routing system.
    ///
    /// # Arguments
    ///
    /// * `request_uri` - The original request URI to rewrite
    /// * `context` - Route match context containing matched prefix information
    ///
    /// # Returns
    ///
    /// RewriteResult containing the rewritten URI and whether it was modified.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let context = RouteMatchContext {
    ///     matched_prefix: Some("/api/v1".to_string()),
    /// };
    ///
    /// let original_uri = Uri::from_str("http://example.com/api/v1/users").unwrap();
    /// let result = filter.rewrite_with_context(&original_uri, &context);
    ///
    /// // With replacePrefixMatch: "/internal", this would rewrite to:
    /// // "http://internal-api.example.com/internal/users"
    /// ```
    pub fn rewrite_with_context(
        &self,
        request_uri: &Uri,
        context: &RouteMatchContext,
    ) -> RewriteResult {
        let rewritten_uri = self.build_rewrite_url(request_uri, context);
        let modified = rewritten_uri != *request_uri;

        RewriteResult {
            uri: rewritten_uri,
            modified,
        }
    }

    /// Build the rewritten URL based on the configuration and original request URI
    fn build_rewrite_url(&self, original_uri: &Uri, context: &RouteMatchContext) -> Uri {
        let mut rewrite_url = match Url::parse(&original_uri.to_string()) {
            Ok(url) => url,
            Err(_) => {
                // Fallback: construct URL from parts
                let scheme = original_uri.scheme_str().unwrap_or("http");
                let host = original_uri.host().unwrap_or("localhost");
                let port = original_uri
                    .port_u16()
                    .map(|p| format!(":{p}"))
                    .unwrap_or_default();
                let path_and_query = original_uri
                    .path_and_query()
                    .map(|pq| pq.as_str())
                    .unwrap_or("/");

                let url_str = format!("{scheme}://{host}{port}{path_and_query}");
                Url::parse(&url_str).unwrap_or_else(|_| Url::parse("http://localhost/").unwrap())
            }
        };

        // Hostname rewrite
        if let Some(hostname) = &self.rewrite.hostname {
            if let Err(e) = rewrite_url.set_host(Some(hostname)) {
                warn!("Failed to set hostname '{}': {}", hostname, e);
            } else {
                debug!("Rewrote hostname to: {}", hostname);
            }
        }

        // Path rewrite
        if let Some(path_config) = &self.rewrite.path {
            let new_path = self.build_rewrite_path(original_uri.path(), path_config, context);
            rewrite_url.set_path(&new_path);
            debug!(
                "Rewrote path from '{}' to '{}'",
                original_uri.path(),
                new_path
            );
        }

        // Query string is preserved automatically when parsing from original URI

        debug!("Built rewrite URI: {} -> {}", original_uri, rewrite_url);

        // Convert Url back to Uri
        Uri::from_str(rewrite_url.as_str()).unwrap_or_else(|e| {
            error!("Failed to convert rewrite URL to URI: {}", e);
            original_uri.clone() // Return original on error
        })
    }

    /// Build the rewritten path based on the configuration
    fn build_rewrite_path(
        &self,
        original_path: &str,
        path_config: &vg_core::config::gateway::types::http::filters::PathRewrite,
        context: &RouteMatchContext,
    ) -> String {
        use vg_core::config::gateway::types::http::filters::PathRewriteType;

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
    ///
    /// This reuses the same logic as the redirect filter for consistency
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

    /// Apply URL rewrite logic to Pingora's request context
    #[allow(dead_code)] // Public API for future use
    pub fn apply_to_pingora_request(
        &self,
        request_header: &mut pingora::http::RequestHeader,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.apply_to_pingora_request_with_context(
            request_header,
            &RouteMatchContext {
                matched_prefix: None,
            },
        )
    }

    /// Apply URL rewrite logic to Pingora's request context with route match context
    pub fn apply_to_pingora_request_with_context(
        &self,
        request_header: &mut pingora::http::RequestHeader,
        context: &RouteMatchContext,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Clone the original URI to avoid borrow checker issues
        let original_uri = request_header.uri.clone();

        let rewrite_result = self.rewrite_with_context(&original_uri, context);

        if rewrite_result.modified {
            // Update the request header with the rewritten URI using Pingora's API
            request_header.set_uri(rewrite_result.uri.clone());

            // Update host header if hostname was rewritten
            if let Some(new_host) = rewrite_result.uri.host()
                && new_host != original_uri.host().unwrap_or("")
                    && let Err(e) = request_header.insert_header("Host", new_host) {
                        warn!("Failed to update Host header: {}", e);
                    }

            debug!("Applied URL rewrite to request header");
        }

        Ok(rewrite_result.modified)
    }
}

#[allow(dead_code)] // Public API for future configuration watching
pub fn create_url_rewrite_filter_receiver(
    task_builder: &TaskBuilder,
    initial_rewrite: Option<URLRewrite>,
) -> Receiver<Option<URLRewriteFilter>> {
    let (sender, receiver) = signal();

    task_builder
        .new_task("url_rewrite_filter_config_watcher")
        .spawn(async move {
            // Send initial configuration
            if let Some(rewrite) = &initial_rewrite {
                let filter = Some(URLRewriteFilter::new(rewrite.clone()));
                sender.set(filter).await;
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
    use vg_core::config::gateway::types::http::filters::{PathRewrite, PathRewriteType};

    #[test]
    fn test_basic_hostname_rewrite() {
        let rewrite_config = URLRewrite {
            hostname: Some("internal-api.example.com".to_string()),
            path: None,
        };

        let filter = URLRewriteFilter::new(rewrite_config);
        let original_uri =
            Uri::from_str("http://external-api.example.com/test?param=value").unwrap();

        let result = filter.rewrite(&original_uri);
        assert!(result.modified);
        assert_eq!(
            result.uri.to_string(),
            "http://internal-api.example.com/test?param=value"
        );
    }

    #[test]
    fn test_path_replacement() {
        let rewrite_config = URLRewrite {
            hostname: None,
            path: Some(PathRewrite {
                rewrite_type: PathRewriteType::ReplaceFullPath,
                replace_full_path: Some("/v2/api".to_string()),
                replace_prefix_match: None,
            }),
        };

        let filter = URLRewriteFilter::new(rewrite_config);
        let original_uri = Uri::from_str("http://localhost/v1/api").unwrap();

        let result = filter.rewrite(&original_uri);
        assert!(result.modified);
        assert_eq!(result.uri.to_string(), "http://localhost/v2/api");
    }

    #[test]
    fn test_no_modification_when_same() {
        let rewrite_config = URLRewrite {
            hostname: Some("example.com".to_string()),
            path: None,
        };

        let filter = URLRewriteFilter::new(rewrite_config);
        let original_uri = Uri::from_str("http://example.com/test").unwrap();

        let result = filter.rewrite(&original_uri);
        assert!(!result.modified);
        assert_eq!(result.uri, original_uri);
    }

    #[test]
    fn test_replace_prefix_match_basic() {
        let rewrite_config = URLRewrite {
            hostname: None,
            path: Some(PathRewrite {
                rewrite_type: PathRewriteType::ReplacePrefixMatch,
                replace_full_path: None,
                replace_prefix_match: Some("/internal".to_string()),
            }),
        };

        let filter = URLRewriteFilter::new(rewrite_config);
        let context = RouteMatchContext {
            matched_prefix: Some("/api/v1".to_string()),
        };

        let original_uri = Uri::from_str("http://example.com/api/v1/users").unwrap();
        let result = filter.rewrite_with_context(&original_uri, &context);

        assert!(result.modified);
        assert_eq!(result.uri.to_string(), "http://example.com/internal/users");
    }

    #[test]
    fn test_replace_prefix_match_with_trailing_slash() {
        let rewrite_config = URLRewrite {
            hostname: None,
            path: Some(PathRewrite {
                rewrite_type: PathRewriteType::ReplacePrefixMatch,
                replace_full_path: None,
                replace_prefix_match: Some("/internal/".to_string()),
            }),
        };

        let filter = URLRewriteFilter::new(rewrite_config);
        let context = RouteMatchContext {
            matched_prefix: Some("/api/v1/".to_string()),
        };

        let original_uri = Uri::from_str("http://example.com/api/v1/users").unwrap();
        let result = filter.rewrite_with_context(&original_uri, &context);

        assert!(result.modified);
        assert_eq!(result.uri.to_string(), "http://example.com/internal/users");
    }

    #[test]
    fn test_replace_prefix_match_exact_match() {
        let rewrite_config = URLRewrite {
            hostname: None,
            path: Some(PathRewrite {
                rewrite_type: PathRewriteType::ReplacePrefixMatch,
                replace_full_path: None,
                replace_prefix_match: Some("/internal".to_string()),
            }),
        };

        let filter = URLRewriteFilter::new(rewrite_config);
        let context = RouteMatchContext {
            matched_prefix: Some("/api/v1".to_string()),
        };

        let original_uri = Uri::from_str("http://example.com/api/v1").unwrap();
        let result = filter.rewrite_with_context(&original_uri, &context);

        assert!(result.modified);
        assert_eq!(result.uri.to_string(), "http://example.com/internal");
    }

    #[test]
    fn test_combined_hostname_and_path_rewrite() {
        let rewrite_config = URLRewrite {
            hostname: Some("internal-api.example.com".to_string()),
            path: Some(PathRewrite {
                rewrite_type: PathRewriteType::ReplacePrefixMatch,
                replace_full_path: None,
                replace_prefix_match: Some("/v3".to_string()),
            }),
        };

        let filter = URLRewriteFilter::new(rewrite_config);
        let context = RouteMatchContext {
            matched_prefix: Some("/api/v1".to_string()),
        };

        let original_uri =
            Uri::from_str("http://external-api.example.com/api/v1/users?param=value").unwrap();
        let result = filter.rewrite_with_context(&original_uri, &context);

        assert!(result.modified);
        assert_eq!(
            result.uri.to_string(),
            "http://internal-api.example.com/v3/users?param=value"
        );
    }

    #[test]
    fn test_replace_prefix_match_no_context() {
        let rewrite_config = URLRewrite {
            hostname: None,
            path: Some(PathRewrite {
                rewrite_type: PathRewriteType::ReplacePrefixMatch,
                replace_full_path: None,
                replace_prefix_match: Some("/fallback".to_string()),
            }),
        };

        let filter = URLRewriteFilter::new(rewrite_config);
        let context = RouteMatchContext {
            matched_prefix: None,
        };

        let original_uri = Uri::from_str("http://example.com/api/v1/users").unwrap();
        let result = filter.rewrite_with_context(&original_uri, &context);

        assert!(result.modified);
        assert_eq!(result.uri.to_string(), "http://example.com/fallback");
    }

    #[test]
    fn test_query_parameters_preserved() {
        let rewrite_config = URLRewrite {
            hostname: Some("internal.example.com".to_string()),
            path: Some(PathRewrite {
                rewrite_type: PathRewriteType::ReplaceFullPath,
                replace_full_path: Some("/internal/api".to_string()),
                replace_prefix_match: None,
            }),
        };

        let filter = URLRewriteFilter::new(rewrite_config);
        let original_uri =
            Uri::from_str("http://external.example.com/old/path?param1=value1&param2=value2")
                .unwrap();

        let result = filter.rewrite(&original_uri);
        assert!(result.modified);
        assert_eq!(
            result.uri.to_string(),
            "http://internal.example.com/internal/api?param1=value1&param2=value2"
        );
    }
}
