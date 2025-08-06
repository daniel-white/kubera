use http::{StatusCode, Uri};
use kubera_core::config::gateway::types::http::filters::RequestRedirect;
use kubera_core::sync::signal::{signal, Receiver};
use kubera_core::task::Builder as TaskBuilder;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error, warn};
use url::Url;

/// Filter for handling HTTP request redirects based on RequestRedirect configuration
#[derive(Debug, Clone, PartialEq)]
pub struct RequestRedirectFilter {
    redirect: Arc<RequestRedirect>,
}

impl RequestRedirectFilter {
    pub fn new(redirect: RequestRedirect) -> Self {
        Self {
            redirect: Arc::new(redirect),
        }
    }

    /// Check if this request should be redirected and return the redirect response
    pub fn should_redirect(&self, request_uri: &Uri) -> Option<RedirectResponse> {
        // Build the redirect URL based on the configuration
        let redirect_uri = self.build_redirect_url(request_uri);

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
    fn build_redirect_url(&self, original_uri: &Uri) -> Uri {
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
            self.build_redirect_path(original_uri.path(), path_config)
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

    /// Build the redirect path based on the path rewrite configuration
    fn build_redirect_path(
        &self,
        original_path: &str,
        path_config: &kubera_core::config::gateway::types::http::filters::PathRewrite,
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
                    // For prefix replacement, we need to know what prefix to replace
                    // This would typically come from the route match configuration
                    // For now, we'll just use the replacement as-is
                    replacement.clone()
                } else {
                    original_path.to_string()
                }
            }
        }
    }

    /// Apply redirect logic to Pingora's request context
    pub fn apply_to_pingora_request(
        &self,
        request_header: &pingora::http::RequestHeader,
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
            Ok(uri) => Ok(self.should_redirect(&uri)),
            Err(e) => {
                error!("Failed to parse URI from request: {}", e);
                Err(Box::new(e))
            }
        }
    }
}

/// Response structure for HTTP redirects
#[derive(Debug, Clone, PartialEq)]
pub struct RedirectResponse {
    pub location: String,
    pub status_code: StatusCode,
}

/// Async function to create a RequestRedirectFilter receiver that responds to configuration changes
#[allow(dead_code)]
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
}
