use crate::CaseInsensitiveString;
use getset::Getters;
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::borrow::Cow;
use std::fmt::Display;
use std::num::NonZeroU16;
use std::str::FromStr;

#[derive(
    Validate, Getters, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Hash,
)]
pub struct Port(#[getset(get = "pub")] NonZeroU16);

impl Port {
    pub fn new<P: Into<Port>>(port: P) -> Self {
        port.into()
    }
}

impl From<NonZeroU16> for Port {
    fn from(port: NonZeroU16) -> Self {
        Self(port)
    }
}

impl From<Port> for u16 {
    fn from(port: Port) -> Self {
        port.0.into()
    }
}

impl From<Port> for NonZeroU16 {
    fn from(port: Port) -> Self {
        port.0
    }
}

impl Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Port {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<NonZeroU16>() {
            Ok(port) => Ok(Self::new(port)),
            Err(_) => Err(()),
        }
    }
}

#[derive(Validate, Getters, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hostname(
    #[validate(min_length = 1)]
    #[validate(max_length = 253)]
    #[validate(pattern = "^\\.?[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$")]
    CaseInsensitiveString,
);

impl Hostname {
    pub fn new<S: AsRef<str>>(s: S) -> Self {
        Self(CaseInsensitiveString::new(s))
    }

    pub fn ends_with(&self, suffix: &Hostname) -> bool {
        self.0.ends_with(&suffix.0)
    }
}

impl From<Hostname> for CaseInsensitiveString {
    fn from(hostname: Hostname) -> Self {
        hostname.0
    }
}

impl From<Hostname> for String {
    fn from(hostname: Hostname) -> Self {
        hostname.0.to_string()
    }
}

impl From<&str> for Hostname {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl AsRef<str> for Hostname {
    fn as_ref(&self) -> &str {
        self.0.0.as_ref()
    }
}

impl JsonSchema for Hostname {
    fn schema_name() -> Cow<'static, str> {
        Cow::from(stringify!(Hostname))
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "format": "hostname",
            "minLength": 1,
            "maxLength": 253,
            "pattern": "^\\.?[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assertables::assert_ok;
    use proptest::prelude::*;
    use rstest::*;
    use serde_valid::Validate;

    // Port tests
    #[rstest]
    #[case(80, true)]
    #[case(443, true)]
    #[case(8080, true)]
    #[case(1, true)]
    #[case(65535, true)]
    fn test_port_creation_valid(#[case] port_num: u16, #[case] should_be_valid: bool) {
        let port = Port::new(port_num);
        assert_eq!(port.validate().is_ok(), should_be_valid);
        assert_eq!(port.0, port_num); // Direct field access
    }

    #[test]
    fn test_port_validation_boundaries() {
        let port_min = Port::new(1);
        let port_max = Port::new(65535);

        assert!(port_min.validate().is_ok());
        assert!(port_max.validate().is_ok());
    }

    #[test]
    fn test_port_from_u16() {
        let port: Port = 8080.into();
        assert_eq!(port.0, 8080); // Direct field access
        assert!(port.validate().is_ok());
    }

    #[test]
    fn test_port_to_u16() {
        let port = Port::new(3000);
        let port_num: u16 = port.into();
        assert_eq!(port_num, 3000);
    }

    #[test]
    fn test_port_display() {
        let port = Port::new(8080);
        assert_eq!(port.to_string(), "8080");
    }

    #[rstest]
    #[case("80", Ok(Port::new(80)))]
    #[case("443", Ok(Port::new(443)))]
    #[case("8080", Ok(Port::new(8080)))]
    #[case("invalid", Err(()))]
    #[case("", Err(()))]
    #[case("65536", Err(()))]
    #[case("-1", Err(()))]
    fn test_port_from_str(#[case] input: &str, #[case] expected: Result<Port, ()>) {
        let result = Port::from_str(input);
        match (result, expected) {
            (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
            (Err(()), Err(())) => {} // Both are errors, which is expected
            _ => unreachable!("Unexpected result: {result:?} vs {expected:?}"),
        }
    }

    #[test]
    fn test_port_serialization() {
        let port = Port::new(8080);
        let serialized = assert_ok!(serde_json::to_string(&port));
        assert_eq!(serialized, "8080");

        let deserialized: Port = assert_ok!(serde_json::from_str(&serialized));
        assert_eq!(port, deserialized);
    }

    #[test]
    fn test_port_equality_and_hash() {
        use std::collections::HashMap;

        let port1 = Port::new(8080);
        let port2 = Port::new(8080);
        let port3 = Port::new(3000);

        assert_eq!(port1, port2);
        assert_ne!(port1, port3);

        let mut map = HashMap::new();
        map.insert(port1, "service1");
        map.insert(port3, "service2");

        assert_eq!(map.get(&port2), Some(&"service1"));
        assert_eq!(map.len(), 2);
    }

    // Hostname tests
    #[rstest]
    #[case("example.com", true)]
    #[case("subdomain.example.com", true)]
    #[case("localhost", true)]
    #[case("api-server", true)]
    #[case("test123", true)]
    #[case("", false)] // Empty hostname
    #[case(".", false)] // Just a dot
    #[case("example..com", false)] // Double dots
    #[case("example-.com", false)] // Trailing dash
    #[case("-example.com", false)] // Leading dash
    fn test_hostname_validation(#[case] hostname: &str, #[case] should_be_valid: bool) {
        let h = Hostname::new(hostname);
        assert_eq!(
            h.validate().is_ok(),
            should_be_valid,
            "Hostname '{hostname}' validation should be {should_be_valid}"
        );
    }

    #[test]
    fn test_hostname_case_insensitive() {
        let h1 = Hostname::new("Example.COM");
        let h2 = Hostname::new("example.com");
        let h3 = Hostname::new("EXAMPLE.com");

        assert_eq!(h1, h2);
        assert_eq!(h2, h3);
        assert_eq!(h1, h3);
    }

    #[test]
    fn test_hostname_ends_with() {
        let hostname = Hostname::new("api.example.com");
        let suffix1 = Hostname::new("example.com");
        let suffix2 = Hostname::new("com");
        let suffix3 = Hostname::new("different.com");

        assert!(hostname.ends_with(&suffix1));
        assert!(hostname.ends_with(&suffix2));
        assert!(!hostname.ends_with(&suffix3));
    }

    #[test]
    fn test_hostname_conversions() {
        let hostname = Hostname::new("example.com");

        // To CaseInsensitiveString
        let cis: CaseInsensitiveString = hostname.clone().into();
        assert_eq!(cis.to_string(), "example.com");

        // To String
        let s: String = hostname.clone().into();
        assert_eq!(s, "example.com");

        // From &str
        let from_str: Hostname = "test.example.com".into();
        assert_eq!(from_str.as_ref(), "test.example.com");
    }

    #[test]
    fn test_hostname_serialization() {
        let hostname = Hostname::new("api.example.com");
        let serialized = assert_ok!(serde_json::to_string(&hostname));
        assert_eq!(serialized, "\"api.example.com\"");

        let deserialized: Hostname = assert_ok!(serde_json::from_str(&serialized));
        assert_eq!(hostname, deserialized);
    }

    #[test]
    fn test_hostname_hash() {
        use std::collections::HashMap;

        let h1 = Hostname::new("EXAMPLE.com");
        let h2 = Hostname::new("example.COM");

        let mut map = HashMap::new();
        map.insert(h1, "value1");
        map.insert(h2, "value2");

        // Should only have one entry due to case insensitivity
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&Hostname::new("Example.Com")), Some(&"value2"));
    }

    proptest! {
        #[test]
        fn test_hostname_properties(
            name in "[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?",
            domain in "[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?"
        ) {
            let hostname_str = if domain.is_empty() {
                name.clone()
            } else {
                format!("{name}.{domain}")
            };

            // Skip if too long (253 is max length)
            if hostname_str.len() > 253 {
                return Ok(());
            }

            let hostname = Hostname::new(&hostname_str);

            // Should be valid for our generated hostnames
            prop_assert!(hostname.validate().is_ok());

            // Round trip through string conversions
            let as_string: String = hostname.clone().into();
            let from_string = Hostname::new(&as_string);
            prop_assert_eq!(hostname.clone(), from_string);

            // JSON serialization roundtrip
            let json = serde_json::to_string(&hostname)?;
            let from_json: Hostname = serde_json::from_str(&json)?;
            prop_assert_eq!(hostname, from_json);
        }
    }
}
