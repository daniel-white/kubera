use std::sync::Arc;
use vg_core::config::gateway::types::http::filters::ResponseHeaderModifier;
use vg_core::continue_on;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;

use super::headers::{apply_header_modifications, HeaderOperations};

/// Filter for modifying response headers based on ResponseHeaderModifier configuration
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseHeaderFilter {
    modifier: Arc<ResponseHeaderModifier>,
}

impl ResponseHeaderFilter {
    pub fn new(modifier: ResponseHeaderModifier) -> Self {
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
            "response",
        )
    }

    /// Get the underlying header modifier configuration
    #[allow(dead_code)] // Public API for configuration access
    pub fn modifier(&self) -> &ResponseHeaderModifier {
        &self.modifier
    }
}

/// Create a reactive filter that responds to ResponseHeaderModifier configuration changes
#[allow(dead_code)] // Public API for future configuration watching
pub fn response_header_filter(
    task_builder: &TaskBuilder,
    modifier_rx: &Receiver<Option<ResponseHeaderModifier>>,
) -> Receiver<Option<ResponseHeaderFilter>> {
    let (tx, rx) = signal();
    let modifier_rx = modifier_rx.clone();

    task_builder
        .new_task(stringify!(response_header_filter))
        .spawn(async move {
            loop {
                if let Some(modifier) = modifier_rx.get().await {
                    let filter = modifier.map(ResponseHeaderFilter::new);
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
    use vg_core::config::gateway::types::http::filters::ResponseHeaderModifierBuilder;

    #[test]
    fn test_response_header_modification() {
        // Create a modifier that sets, adds, and removes headers
        let mut builder = ResponseHeaderModifierBuilder::new();
        builder
            .set_header("X-Custom-Response", "custom-value")
            .unwrap();
        builder
            .add_header("X-Additional-Response", "additional-value")
            .unwrap();
        builder.remove_header("X-Remove-Response").unwrap();
        let modifier = builder.build();

        let filter = ResponseHeaderFilter::new(modifier);

        // Create initial headers
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Existing-Response",
            HeaderValue::from_static("existing-value"),
        );
        headers.insert("X-Remove-Response", HeaderValue::from_static("remove-this"));
        headers.insert(
            "X-Additional-Response",
            HeaderValue::from_static("original-value"),
        );

        // Apply the filter
        filter.apply_to_headers(&mut headers).unwrap();

        // Verify results
        assert_eq!(headers.get("X-Custom-Response").unwrap(), "custom-value");
        assert!(headers.get("X-Remove-Response").is_none());
        assert_eq!(
            headers.get("X-Existing-Response").unwrap(),
            "existing-value"
        );

        // X-Additional-Response should have both values (original + added)
        let additional_values: Vec<_> = headers
            .get_all("X-Additional-Response")
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect();
        assert!(additional_values.contains(&"original-value"));
        assert!(additional_values.contains(&"additional-value"));
    }

    #[test]
    fn test_empty_response_header_modifier() {
        let modifier = ResponseHeaderModifier::default();
        let filter = ResponseHeaderFilter::new(modifier);

        let mut headers = HeaderMap::new();
        headers.insert("X-Existing", HeaderValue::from_static("existing-value"));

        // Apply empty filter - should not change anything
        filter.apply_to_headers(&mut headers).unwrap();

        assert_eq!(headers.get("X-Existing").unwrap(), "existing-value");
        assert_eq!(headers.len(), 1);
    }

    #[test]
    fn test_response_header_modifier_builder() {
        let mut builder = ResponseHeaderModifierBuilder::new();

        // Test successful operations
        assert!(
            builder
                .set_header("Content-Type", "application/json")
                .is_ok()
        );
        assert!(builder.add_header("X-Custom", "value1").is_ok());
        assert!(builder.remove_header("X-Remove").is_ok());

        let modifier = builder.build();

        assert!(modifier.set().is_some());
        assert!(modifier.add().is_some());
        assert!(modifier.remove().is_some());
        assert!(!modifier.is_empty());

        // Test error cases
        let mut error_builder = ResponseHeaderModifierBuilder::new();
        assert!(error_builder.set_header("", "value").is_err()); // Empty name
        assert!(error_builder.set_header("valid", "").is_err()); // Empty value
        assert!(error_builder.remove_header("").is_err()); // Empty name
    }

    #[test]
    fn test_response_header_modifier_is_empty() {
        let empty_modifier = ResponseHeaderModifier::default();
        assert!(empty_modifier.is_empty());

        let mut builder = ResponseHeaderModifierBuilder::new();
        builder.set_header("X-Test", "value").unwrap();
        let non_empty_modifier = builder.build();
        assert!(!non_empty_modifier.is_empty());
    }
}
