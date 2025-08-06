use gateway_api::apis::standard::httproutes::{
    HTTPRouteRulesFilters, HTTPRouteRulesFiltersRequestHeaderModifier,
    HTTPRouteRulesFiltersRequestRedirect, HTTPRouteRulesFiltersRequestRedirectPath,
};
use kubera_core::config::gateway::types::http::filters::{
    PathRewrite, PathRewriteType, RequestHeaderModifier, RequestHeaderModifierBuilder,
    RequestRedirect,
};
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum FilterConversionError {
    #[error("Invalid header name: {0}")]
    InvalidHeaderName(String),
    #[allow(dead_code)]
    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(String),
}

/// Convert Gateway API `HTTPRouteRulesFilters` to Kubera `RequestHeaderModifier`
#[allow(dead_code)]
pub fn convert_gateway_api_filter(
    filter: &HTTPRouteRulesFilters,
) -> Result<Option<RequestHeaderModifier>, FilterConversionError> {
    // Check if this is a RequestHeaderModifier filter
    if let Some(request_header_modifier) = &filter.request_header_modifier {
        debug!(
            "Converting RequestHeaderModifier filter: {:?}",
            request_header_modifier
        );
        return convert_request_header_modifier(request_header_modifier);
    }

    // Check for other filter types
    if filter.request_mirror.is_some() {
        debug!("RequestMirror filter found but not supported yet");
        return Ok(None);
    }

    if filter.request_redirect.is_some() {
        debug!("RequestRedirect filter found but not supported yet");
        return Ok(None);
    }

    if filter.extension_ref.is_some() {
        debug!("ExtensionRef filter found but not supported yet");
        return Ok(None);
    }

    // Empty filter or unsupported type
    debug!("Empty or unsupported filter: {:?}", filter);
    Ok(None)
}

/// Convert Gateway API `RequestHeaderModifier` to Kubera `RequestHeaderModifier`
#[allow(dead_code)]
fn convert_request_header_modifier(
    gw_modifier: &HTTPRouteRulesFiltersRequestHeaderModifier,
) -> Result<Option<RequestHeaderModifier>, FilterConversionError> {
    let mut builder = RequestHeaderModifierBuilder::new();

    // Convert 'set' headers
    if let Some(set_headers) = &gw_modifier.set {
        for header in set_headers {
            builder
                .set_header(&header.name, &header.value)
                .map_err(|_| FilterConversionError::InvalidHeaderName(header.name.clone()))?;
        }
    }

    // Convert 'add' headers
    if let Some(add_headers) = &gw_modifier.add {
        for header in add_headers {
            builder
                .add_header(&header.name, &header.value)
                .map_err(|_| FilterConversionError::InvalidHeaderName(header.name.clone()))?;
        }
    }

    // Convert 'remove' headers
    if let Some(remove_headers) = &gw_modifier.remove {
        for name in remove_headers {
            builder
                .remove_header(name)
                .map_err(|_| FilterConversionError::InvalidHeaderName(name.clone()))?;
        }
    }

    let modifier = builder.build();

    // Only return Some if there are actual modifications
    if modifier.is_empty() {
        Ok(None)
    } else {
        Ok(Some(modifier))
    }
}

/// Convert Gateway API `RequestRedirect` to Kubera `RequestRedirect`
#[allow(dead_code)]
pub fn convert_request_redirect(
    gw_redirect: &HTTPRouteRulesFiltersRequestRedirect,
) -> Result<RequestRedirect, FilterConversionError> {
    let mut redirect = RequestRedirect {
        scheme: gw_redirect.scheme.as_ref().map(|s| {
            use gateway_api::httproutes::HTTPRouteRulesFiltersRequestRedirectScheme;
            match s {
                HTTPRouteRulesFiltersRequestRedirectScheme::Http => "http".to_string(),
                HTTPRouteRulesFiltersRequestRedirectScheme::Https => "https".to_string(),
            }
        }),
        hostname: gw_redirect.hostname.clone(),
        port: gw_redirect.port.map(|p| p as u16),
        path: None,
        status_code: gw_redirect.status_code.map(|code| code as u16),
    };

    // Convert path rewriting if present
    if let Some(path_config) = &gw_redirect.path {
        redirect.path = Some(convert_path_rewrite(path_config)?);
    }

    Ok(redirect)
}

/// Convert Gateway API path rewrite configuration
fn convert_path_rewrite(
    gw_path: &HTTPRouteRulesFiltersRequestRedirectPath,
) -> Result<PathRewrite, FilterConversionError> {
    use gateway_api::apis::standard::httproutes::HTTPRouteRulesFiltersRequestRedirectPathType;

    let path_rewrite = match gw_path.r#type {
        HTTPRouteRulesFiltersRequestRedirectPathType::ReplaceFullPath => PathRewrite {
            rewrite_type: PathRewriteType::ReplaceFullPath,
            replace_full_path: gw_path.replace_full_path.clone(),
            replace_prefix_match: None,
        },
        HTTPRouteRulesFiltersRequestRedirectPathType::ReplacePrefixMatch => PathRewrite {
            rewrite_type: PathRewriteType::ReplacePrefixMatch,
            replace_full_path: None,
            replace_prefix_match: gw_path.replace_prefix_match.clone(),
        },
    };

    Ok(path_rewrite)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_api::apis::standard::httproutes::{
        HTTPRouteRulesFiltersRequestHeaderModifierAdd,
        HTTPRouteRulesFiltersRequestHeaderModifierSet,
    };

    #[test]
    fn test_convert_request_header_modifier() {
        // Create a Gateway API RequestHeaderModifier with proper types
        let set_headers = vec![HTTPRouteRulesFiltersRequestHeaderModifierSet {
            name: "X-Gateway".to_string(),
            value: "kubera".to_string(),
        }];

        let add_headers = vec![HTTPRouteRulesFiltersRequestHeaderModifierAdd {
            name: "X-Request-ID".to_string(),
            value: "123".to_string(),
        }];

        let remove_headers = vec!["X-Debug".to_string()];

        let gw_modifier = HTTPRouteRulesFiltersRequestHeaderModifier {
            set: Some(set_headers),
            add: Some(add_headers),
            remove: Some(remove_headers),
        };

        // Convert to Kubera RequestHeaderModifier
        let result =
            convert_request_header_modifier(&gw_modifier).expect("conversion should succeed");
        assert!(result.is_some());

        let modifier = result.expect("modifier should be present");

        // Check set headers
        assert!(modifier.set().is_some());
        let set = modifier
            .set()
            .as_ref()
            .expect("set headers should be present");
        assert_eq!(set.len(), 1);
        assert_eq!(set[0].name, "X-Gateway");
        assert_eq!(set[0].value, "kubera");

        // Check add headers
        assert!(modifier.add().is_some());
        let add = modifier
            .add()
            .as_ref()
            .expect("add headers should be present");
        assert_eq!(add.len(), 1);
        assert_eq!(add[0].name, "X-Request-ID");
        assert_eq!(add[0].value, "123");

        // Check remove headers
        assert!(modifier.remove().is_some());
        let remove = modifier
            .remove()
            .as_ref()
            .expect("remove headers should be present");
        assert_eq!(remove.len(), 1);
        assert_eq!(remove[0], "X-Debug");
    }
}
