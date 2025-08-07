use http::{HeaderMap, HeaderName, HeaderValue};
use kubera_core::config::gateway::types::http::filters::ResponseHeaderModifier;
use kubera_core::continue_on;
use kubera_core::sync::signal::{signal, Receiver};
use kubera_core::task::Builder as TaskBuilder;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, warn};

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

    /// Apply header modifications to the given header map
    pub fn apply_to_headers(&self, headers: &mut HeaderMap) {
        // Remove headers first
        if let Some(remove_headers) = self.modifier.remove() {
            for header_name in remove_headers {
                if let Ok(name) = HeaderName::from_str(header_name) {
                    headers.remove(&name);
                    debug!("Removed response header: {}", header_name);
                } else {
                    warn!("Invalid response header name for removal: {}", header_name);
                }
            }
        }

        // Set headers (replace existing)
        if let Some(set_headers) = self.modifier.set() {
            for header in set_headers {
                match (
                    HeaderName::from_str(&header.name),
                    HeaderValue::from_str(&header.value),
                ) {
                    (Ok(name), Ok(value)) => {
                        headers.insert(name, value);
                        debug!("Set response header: {} = {}", header.name, header.value);
                    }
                    (Err(e), _) => {
                        warn!("Invalid response header name '{}': {}", header.name, e);
                    }
                    (_, Err(e)) => {
                        warn!("Invalid response header value for '{}': {}", header.name, e);
                    }
                }
            }
        }

        // Add headers (append to existing)
        if let Some(add_headers) = self.modifier.add() {
            for header in add_headers {
                match (
                    HeaderName::from_str(&header.name),
                    HeaderValue::from_str(&header.value),
                ) {
                    (Ok(name), Ok(value)) => {
                        headers.append(name, value);
                        debug!("Added response header: {} = {}", header.name, header.value);
                    }
                    (Err(e), _) => {
                        warn!("Invalid response header name '{}': {}", header.name, e);
                    }
                    (_, Err(e)) => {
                        warn!("Invalid response header value for '{}': {}", header.name, e);
                    }
                }
            }
        }
    }

    /// Apply header modifications to Pingora's ResponseHeader
    pub fn apply_to_pingora_headers(
        &self,
        headers: &mut pingora::http::ResponseHeader,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Remove headers first
        if let Some(remove_headers) = self.modifier.remove() {
            for header_name in remove_headers {
                headers.remove_header(header_name);
                debug!("Removed response header: {}", header_name);
            }
        }

        // Set headers (replace existing)
        if let Some(set_headers) = self.modifier.set() {
            for header in set_headers {
                match HeaderValue::from_str(&header.value) {
                    Ok(value) => {
                        if let Err(e) = headers.insert_header(header.name.clone(), value) {
                            warn!("Failed to set response header '{}': {}", header.name, e);
                        } else {
                            debug!("Set response header: {} = {}", header.name, header.value);
                        }
                    }
                    Err(e) => {
                        warn!("Invalid response header value for '{}': {}", header.name, e);
                    }
                }
            }
        }

        // Add headers (append to existing)
        if let Some(add_headers) = self.modifier.add() {
            for header in add_headers {
                match HeaderValue::from_str(&header.value) {
                    Ok(value) => {
                        if let Err(e) = headers.append_header(header.name.clone(), value) {
                            warn!("Failed to add response header '{}': {}", header.name, e);
                        } else {
                            debug!("Added response header: {} = {}", header.name, header.value);
                        }
                    }
                    Err(e) => {
                        warn!("Invalid response header value for '{}': {}", header.name, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get the underlying modifier
    pub fn modifier(&self) -> &ResponseHeaderModifier {
        &self.modifier
    }
}

/// Create a reactive filter that responds to ResponseHeaderModifier configuration changes
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
    use kubera_core::config::gateway::types::http::filters::ResponseHeaderModifierBuilder;

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
        filter.apply_to_headers(&mut headers);

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
        filter.apply_to_headers(&mut headers);

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
