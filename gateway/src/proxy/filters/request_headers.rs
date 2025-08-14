use std::sync::Arc;
use vg_core::config::gateway::types::http::filters::RequestHeaderModifier;
use vg_core::continue_on;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;

use super::headers::{apply_header_modifications, HeaderOperations};

/// Filter for modifying request headers based on RequestHeaderModifier configuration
#[derive(Debug, Clone, PartialEq)]
pub struct RequestHeaderFilter {
    modifier: Arc<RequestHeaderModifier>,
}

impl RequestHeaderFilter {
    pub fn new(modifier: RequestHeaderModifier) -> Self {
        Self {
            modifier: Arc::new(modifier),
        }
    }

    /// Apply header modifications to any header type that implements HeaderOperations
    pub fn apply_to_headers<H: HeaderOperations>(&self, headers: &mut H) -> Result<(), H::Error> {
        apply_header_modifications(
            headers,
            self.modifier.remove().as_ref(),
            self.modifier.set().as_ref(),
            self.modifier.add().as_ref(),
            "request",
        )
    }

    /// Get the underlying header modifier configuration
    #[allow(dead_code)] // Public API for configuration access
    pub fn modifier(&self) -> &RequestHeaderModifier {
        &self.modifier
    }
}

/// Create a reactive filter that responds to RequestHeaderModifier configuration changes
#[allow(dead_code)] // Public API for future configuration watching
pub fn request_header_filter(
    task_builder: &TaskBuilder,
    modifier_rx: &Receiver<Option<RequestHeaderModifier>>,
) -> Receiver<Option<RequestHeaderFilter>> {
    let (tx, rx) = signal("request_header_filter");
    let modifier_rx = modifier_rx.clone();

    task_builder
        .new_task(stringify!(request_header_filter))
        .spawn(async move {
            loop {
                if let Some(modifier) = modifier_rx.get().await {
                    let filter = modifier.map(RequestHeaderFilter::new);
                    tx.set(filter).await;
                } else {
                    tx.set(None).await;
                }

                continue_on!(modifier_rx.changed());
            }
        });

    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{HeaderMap, HeaderValue};
    use vg_core::config::gateway::types::http::filters::RequestHeaderModifierBuilder;

    #[test]
    fn test_header_modification() {
        // Create a modifier that sets, adds, and removes headers
        let mut builder = RequestHeaderModifierBuilder::new();
        builder
            .set_header("X-Custom-Header", "custom-value")
            .unwrap();
        builder
            .add_header("X-Additional", "additional-value")
            .unwrap();
        builder.remove_header("X-Remove-Me").unwrap();
        let modifier = builder.build();

        let filter = RequestHeaderFilter::new(modifier);

        // Create initial headers
        let mut headers = HeaderMap::new();
        headers.insert("X-Existing", HeaderValue::from_static("existing-value"));
        headers.insert("X-Remove-Me", HeaderValue::from_static("remove-this"));
        headers.insert("X-Additional", HeaderValue::from_static("original-value"));

        // Apply the filter
        filter.apply_to_headers(&mut headers).unwrap();

        // Verify results
        assert_eq!(headers.get("X-Custom-Header").unwrap(), "custom-value");
        assert!(headers.get("X-Remove-Me").is_none());
        assert_eq!(headers.get("X-Existing").unwrap(), "existing-value");

        // X-Additional should have both values (original + added)
        let additional_values: Vec<_> = headers.get_all("X-Additional").iter().collect();
        assert_eq!(additional_values.len(), 2);
    }

    #[test]
    fn test_empty_modifier() {
        let modifier = RequestHeaderModifier::default();
        let filter = RequestHeaderFilter::new(modifier);

        let mut headers = HeaderMap::new();
        headers.insert("X-Existing", HeaderValue::from_static("existing-value"));

        filter.apply_to_headers(&mut headers).unwrap();

        // Headers should remain unchanged
        assert_eq!(headers.get("X-Existing").unwrap(), "existing-value");
        assert_eq!(headers.len(), 1);
    }

    #[test]
    fn test_invalid_header_names() {
        let mut builder = RequestHeaderModifierBuilder::new();

        // These should fail due to invalid header names
        assert!(builder.set_header("", "value").is_err());
        assert!(builder.set_header("invalid header", "value").is_err());
        assert!(builder.add_header("", "value").is_err());
        assert!(builder.remove_header("").is_err());
    }
}
