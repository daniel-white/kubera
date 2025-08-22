use http::StatusCode;
use std::error::Error;
use std::fmt;
use std::fmt::Display;
use std::str::FromStr;

use crate::net::Port;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenerProtocol {
    Http,
    Https,
}

impl Display for ListenerProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListenerProtocol::Http => write!(f, "http"),
            ListenerProtocol::Https => write!(f, "https"),
        }
    }
}

impl FromStr for ListenerProtocol {
    type Err = ListenerParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "http" => Ok(ListenerProtocol::Http),
            "https" => Ok(ListenerProtocol::Https),
            _ => Err(ListenerParseError::InvalidProtocol),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listener {
    pub name: String,
    pub protocol: ListenerProtocol,
    pub port: Port,
    pub path: String,
    pub expected_status: StatusCode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenerParseError {
    InvalidFormat,
    InvalidPort,
    InvalidStatus,
    InvalidProtocol,
    MissingField,
}

impl Display for ListenerParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListenerParseError::InvalidFormat => write!(f, "Invalid listener format"),
            ListenerParseError::InvalidPort => write!(f, "Invalid port number"),
            ListenerParseError::InvalidStatus => write!(f, "Invalid expected status code"),
            ListenerParseError::InvalidProtocol => write!(f, "Invalid protocol"),
            ListenerParseError::MissingField => write!(f, "Missing field in listener entry"),
        }
    }
}

impl Error for ListenerParseError {}

impl Display for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{},{},{},{},{}",
            self.name,
            self.protocol,
            self.port,
            self.path,
            self.expected_status.as_u16()
        )
    }
}

impl FromStr for Listener {
    type Err = ListenerParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fields: Vec<&str> = s.split(',').map(str::trim).collect();
        if fields.len() != 5 {
            return Err(ListenerParseError::MissingField);
        }
        let name = fields[0].to_string();
        let protocol = ListenerProtocol::from_str(fields[1])?;
        let port = Port::from_str(fields[2]).map_err(|()| ListenerParseError::InvalidPort)?;
        let path = fields[3].to_string();
        let status_u16 = fields[4]
            .parse::<u16>()
            .map_err(|_| ListenerParseError::InvalidStatus)?;
        let expected_status =
            StatusCode::from_u16(status_u16).map_err(|_| ListenerParseError::InvalidStatus)?;
        Ok(Listener {
            name,
            protocol,
            port,
            path,
            expected_status,
        })
    }
}

/// Parse a delimited string into a Vec<Listener>
pub fn parse_listeners(input: &str) -> Result<Vec<Listener>, ListenerParseError> {
    let mut listeners = Vec::new();
    for entry in input.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let listener = Listener::from_str(entry)?;
        listeners.push(listener);
    }
    Ok(listeners)
}

/// Serialize a Vec<Listener> into a delimited string
pub fn serialize_listeners(listeners: &[Listener]) -> String {
    listeners
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_listeners() {
        let input = "admin,http,8080,/healthz,200;public,https,8443,/ready,204";
        let listeners = parse_listeners(input).unwrap();
        assert_eq!(listeners.len(), 2);
        assert_eq!(listeners[0].name, "admin");
        assert_eq!(listeners[0].protocol, ListenerProtocol::Http);
        assert_eq!(listeners[0].port, Port::new(8080));
        assert_eq!(listeners[0].path, "/healthz");
        assert_eq!(listeners[0].expected_status, StatusCode::OK);
        assert_eq!(listeners[1].name, "public");
        assert_eq!(listeners[1].protocol, ListenerProtocol::Https);
        assert_eq!(listeners[1].port, Port::new(8443));
        assert_eq!(listeners[1].path, "/ready");
        assert_eq!(listeners[1].expected_status, StatusCode::NO_CONTENT);
    }

    #[test]
    fn test_parse_invalid_port() {
        let input = "admin,http,notaport,/healthz,200";
        let err = parse_listeners(input).unwrap_err();
        assert!(matches!(err, ListenerParseError::InvalidPort));
    }

    #[test]
    fn test_parse_invalid_status() {
        let input = "admin,http,8080,/healthz,notastatus";
        let err = parse_listeners(input).unwrap_err();
        assert!(matches!(err, ListenerParseError::InvalidStatus));
    }

    #[test]
    fn test_parse_invalid_protocol() {
        let input = "admin,ftp,8080,/healthz,200";
        let err = parse_listeners(input).unwrap_err();
        assert!(matches!(err, ListenerParseError::InvalidProtocol));
    }

    #[test]
    fn test_parse_missing_field() {
        let input = "admin,http,8080,/healthz";
        let err = parse_listeners(input).unwrap_err();
        assert!(matches!(err, ListenerParseError::MissingField));
    }

    #[test]
    fn test_parse_empty_entries() {
        let input = ";";
        let listeners = parse_listeners(input).unwrap();
        assert_eq!(listeners.len(), 0);
    }

    #[test]
    fn test_serialize_listeners() {
        let input = "admin,http,8080,/healthz,200;public,https,8443,/ready,204";
        let listeners = parse_listeners(input).unwrap();
        let output = serialize_listeners(&listeners);
        assert_eq!(input, output);
    }
}
