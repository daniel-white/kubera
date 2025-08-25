use http::{HeaderMap, HeaderName, HeaderValue};
use std::str::FromStr;
use tracing::{debug, warn};

/// Trait for abstracting header operations across different header types
pub trait HeaderOperations {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Remove a header by name
    fn remove_header(&mut self, name: &str);

    /// Insert/replace a header
    fn insert_header(&mut self, name: &str, value: HeaderValue) -> Result<(), Self::Error>;

    /// Append a header value
    fn append_header(&mut self, name: &str, value: HeaderValue) -> Result<(), Self::Error>;
}

/// Implementation for standard HTTP HeaderMap
impl HeaderOperations for HeaderMap {
    type Error = http::header::InvalidHeaderName;

    fn remove_header(&mut self, name: &str) {
        if let Ok(header_name) = HeaderName::from_str(name) {
            self.remove(&header_name);
        }
    }

    fn insert_header(&mut self, name: &str, value: HeaderValue) -> Result<(), Self::Error> {
        let header_name = HeaderName::from_str(name)?;
        self.insert(header_name, value);
        Ok(())
    }

    fn append_header(&mut self, name: &str, value: HeaderValue) -> Result<(), Self::Error> {
        let header_name = HeaderName::from_str(name)?;
        self.append(header_name, value);
        Ok(())
    }
}

/// Custom error type for Pingora header operations
#[derive(Debug)]
pub struct PingoraHeaderError(String);

impl std::fmt::Display for PingoraHeaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pingora header error: {}", self.0)
    }
}

impl std::error::Error for PingoraHeaderError {}

/// Implementation for Pingora RequestHeader
impl HeaderOperations for pingora::http::RequestHeader {
    type Error = PingoraHeaderError;

    fn remove_header(&mut self, name: &str) {
        pingora::http::RequestHeader::remove_header(self, name);
    }

    fn insert_header(&mut self, name: &str, value: HeaderValue) -> Result<(), Self::Error> {
        pingora::http::RequestHeader::insert_header(self, name.to_string(), value)
            .map_err(|e| PingoraHeaderError(format!("Failed to insert header: {e}")))?;
        Ok(())
    }

    fn append_header(&mut self, name: &str, value: HeaderValue) -> Result<(), Self::Error> {
        pingora::http::RequestHeader::append_header(self, name.to_string(), value)
            .map_err(|e| PingoraHeaderError(format!("Failed to append header: {e}")))?;
        Ok(())
    }
}

/// Implementation for Pingora ResponseHeader
impl HeaderOperations for pingora::http::ResponseHeader {
    type Error = PingoraHeaderError;

    fn remove_header(&mut self, name: &str) {
        pingora::http::ResponseHeader::remove_header(self, name);
    }

    fn insert_header(&mut self, name: &str, value: HeaderValue) -> Result<(), Self::Error> {
        pingora::http::ResponseHeader::insert_header(self, name.to_string(), value)
            .map_err(|e| PingoraHeaderError(format!("Failed to insert header: {e}")))?;
        Ok(())
    }

    fn append_header(&mut self, name: &str, value: HeaderValue) -> Result<(), Self::Error> {
        pingora::http::ResponseHeader::append_header(self, name.to_string(), value)
            .map_err(|e| PingoraHeaderError(format!("Failed to append header: {e}")))?;
        Ok(())
    }
}

/// Generic function to apply header modifications to any type implementing HeaderOperations
pub fn apply_header_modifications<H: HeaderOperations>(
    headers: &mut H,
    remove_headers: Option<&Vec<String>>,
    set_headers: Option<&Vec<vg_core::config::gateway::types::http::filters::HTTPHeader>>,
    add_headers: Option<&Vec<vg_core::config::gateway::types::http::filters::HTTPHeader>>,
    header_type: &str, // "request" or "response" for logging
) -> Result<(), H::Error> {
    // Remove headers first
    if let Some(remove_headers) = remove_headers {
        for header_name in remove_headers {
            headers.remove_header(header_name);
            debug!("Removed {} header: {}", header_type, header_name);
        }
    }

    // Set headers (replace existing)
    if let Some(set_headers) = set_headers {
        for header in set_headers {
            match HeaderValue::from_str(&header.value) {
                Ok(value) => {
                    if let Err(e) = headers.insert_header(&header.name, value) {
                        warn!(
                            "Failed to set {} header '{}': {}",
                            header_type, header.name, e
                        );
                    } else {
                        debug!(
                            "Set {} header: {} = {}",
                            header_type, header.name, header.value
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        "Invalid {} header value for '{}': {}",
                        header_type, header.name, e
                    );
                }
            }
        }
    }

    // Add headers (append to existing)
    if let Some(add_headers) = add_headers {
        for header in add_headers {
            match HeaderValue::from_str(&header.value) {
                Ok(value) => {
                    if let Err(e) = headers.append_header(&header.name, value) {
                        warn!(
                            "Failed to add {} header '{}': {}",
                            header_type, header.name, e
                        );
                    } else {
                        debug!(
                            "Added {} header: {} = {}",
                            header_type, header.name, header.value
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        "Invalid {} header value for '{}': {}",
                        header_type, header.name, e
                    );
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_map_operations() {
        let mut headers = HeaderMap::new();

        // Test insert_header
        let value = HeaderValue::from_static("test-value");
        assert!(headers.insert_header("X-Test", value).is_ok());
        assert_eq!(headers.get("X-Test").unwrap(), "test-value");

        // Test append_header
        let value2 = HeaderValue::from_static("test-value2");
        assert!(headers.append_header("X-Test", value2).is_ok());
        assert_eq!(headers.get_all("X-Test").iter().count(), 2);

        // Test remove_header
        headers.remove_header("X-Test");
        assert!(headers.get("X-Test").is_none());

        // Test invalid header name
        let result = headers.insert_header("", HeaderValue::from_static("value"));
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_header_modifications_set() {
        let mut headers = HeaderMap::new();
        let test_headers = create_test_headers();

        let result =
            apply_header_modifications(&mut headers, None, Some(&test_headers), None, "test");

        assert!(result.is_ok());
        assert_eq!(headers.get("X-Test-Header").unwrap(), "test-value");
        assert_eq!(headers.get("X-Another-Header").unwrap(), "another-value");
    }

    #[test]
    fn test_apply_header_modifications_add() {
        let mut headers = HeaderMap::new();
        // Pre-populate with existing header
        headers.insert("X-Test-Header", HeaderValue::from_static("existing-value"));

        let test_headers = vec![HTTPHeader {
            name: "X-Test-Header".to_string(),
            value: "added-value".to_string(),
        }];

        let result =
            apply_header_modifications(&mut headers, None, None, Some(&test_headers), "test");

        assert!(result.is_ok());
        // Should have both values
        let values: Vec<_> = headers.get_all("X-Test-Header").iter().collect();
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_apply_header_modifications_remove() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Remove-Me", HeaderValue::from_static("value"));
        headers.insert("X-Keep-Me", HeaderValue::from_static("value"));

        let remove_headers = vec!["X-Remove-Me".to_string()];

        let result =
            apply_header_modifications(&mut headers, Some(&remove_headers), None, None, "test");

        assert!(result.is_ok());
        assert!(headers.get("X-Remove-Me").is_none());
        assert!(headers.get("X-Keep-Me").is_some());
    }

    #[test]
    fn test_apply_header_modifications_all_operations() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Remove-Me", HeaderValue::from_static("remove-this"));
        headers.insert("X-Append-To", HeaderValue::from_static("original"));
        headers.insert("X-Replace-Me", HeaderValue::from_static("old-value"));

        let remove_headers = vec!["X-Remove-Me".to_string()];
        let set_headers = vec![HTTPHeader {
            name: "X-Replace-Me".to_string(),
            value: "new-value".to_string(),
        }];
        let add_headers = vec![HTTPHeader {
            name: "X-Append-To".to_string(),
            value: "appended".to_string(),
        }];

        let result = apply_header_modifications(
            &mut headers,
            Some(&remove_headers),
            Some(&set_headers),
            Some(&add_headers),
            "test",
        );

        assert!(result.is_ok());

        // Verify remove operation
        assert!(headers.get("X-Remove-Me").is_none());

        // Verify set operation (should replace)
        assert_eq!(headers.get("X-Replace-Me").unwrap(), "new-value");

        // Verify add operation (should have both values)
        let append_values: Vec<_> = headers.get_all("X-Append-To").iter().collect();
        assert_eq!(append_values.len(), 2);
    }

    #[test]
    fn test_apply_header_modifications_empty_operations() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Existing", HeaderValue::from_static("value"));

        let result = apply_header_modifications(&mut headers, None, None, None, "test");

        assert!(result.is_ok());
        // Headers should be unchanged
        assert_eq!(headers.len(), 1);
        assert_eq!(headers.get("X-Existing").unwrap(), "value");
    }

    #[test]
    fn test_apply_header_modifications_invalid_header_value() {
        let mut headers = HeaderMap::new();

        // Create header with invalid characters that would fail HeaderValue::from_str
        let invalid_headers = vec![HTTPHeader {
            name: "X-Test".to_string(),
            value: "invalid\x00value".to_string(), // null byte is invalid in HTTP header values
        }];

        let result =
            apply_header_modifications(&mut headers, None, Some(&invalid_headers), None, "test");

        // Should complete without error even though header value is invalid
        assert!(result.is_ok());
        // Invalid header should not be added
        assert!(headers.get("X-Test").is_none());
    }

    #[test]
    fn test_pingora_header_error() {
        let error = PingoraHeaderError("test error".to_string());
        assert_eq!(error.to_string(), "Pingora header error: test error");

        // Test that it implements the Error trait
        let _: &dyn std::error::Error = &error;
    }
}
