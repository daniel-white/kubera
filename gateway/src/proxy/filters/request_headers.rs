use http::{HeaderMap, HeaderName, HeaderValue};
use kubera_core::config::gateway::types::http::filters::RequestHeaderModifier;
use kubera_core::continue_on;
use kubera_core::sync::signal::{signal, Receiver};
use kubera_core::task::Builder as TaskBuilder;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, warn};

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

    /// Apply header modifications to the given header map
    pub fn apply_to_headers(&self, headers: &mut HeaderMap) {
        // Remove headers first
        if let Some(remove_headers) = self.modifier.remove() {
            for header_name in remove_headers {
                if let Ok(name) = HeaderName::from_str(header_name) {
                    headers.remove(&name);
                    debug!("Removed header: {}", header_name);
                } else {
                    warn!("Invalid header name for removal: {}", header_name);
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
                        debug!("Set header: {} = {}", header.name, header.value);
                    }
                    (Err(e), _) => {
                        warn!("Invalid header name '{}': {}", header.name, e);
                    }
                    (_, Err(e)) => {
                        warn!("Invalid header value for '{}': {}", header.name, e);
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
                        debug!("Added header: {} = {}", header.name, header.value);
                    }
                    (Err(e), _) => {
                        warn!("Invalid header name '{}': {}", header.name, e);
                    }
                    (_, Err(e)) => {
                        warn!("Invalid header value for '{}': {}", header.name, e);
                    }
                }
            }
        }
    }

    /// Apply header modifications to Pingora's RequestHeader
    pub fn apply_to_pingora_headers(
        &self,
        headers: &mut pingora::http::RequestHeader,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Remove headers first
        if let Some(remove_headers) = self.modifier.remove() {
            for header_name in remove_headers {
                headers.remove_header(header_name);
                debug!("Removed header: {}", header_name);
            }
        }

        // Set headers (replace existing)
        if let Some(set_headers) = self.modifier.set() {
            for header in set_headers {
                match HeaderValue::from_str(&header.value) {
                    Ok(value) => {
                        if let Err(e) = headers.insert_header(header.name.clone(), value) {
                            warn!("Failed to set header '{}': {}", header.name, e);
                        } else {
                            debug!("Set header: {} = {}", header.name, header.value);
                        }
                    }
                    Err(e) => {
                        warn!("Invalid header value for '{}': {}", header.name, e);
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
                            warn!("Failed to add header '{}': {}", header.name, e);
                        } else {
                            debug!("Added header: {} = {}", header.name, header.value);
                        }
                    }
                    Err(e) => {
                        warn!("Invalid header value for '{}': {}", header.name, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get the underlying modifier
    pub fn modifier(&self) -> &RequestHeaderModifier {
        &self.modifier
    }
}

/// Create a reactive filter that responds to RequestHeaderModifier configuration changes
pub fn request_header_filter(
    task_builder: &TaskBuilder,
    modifier_rx: &Receiver<Option<RequestHeaderModifier>>,
) -> Receiver<Option<RequestHeaderFilter>> {
    let (tx, rx) = signal();
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
    use kubera_core::config::gateway::types::http::filters::RequestHeaderModifierBuilder;

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
        filter.apply_to_headers(&mut headers);

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

        filter.apply_to_headers(&mut headers);

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
