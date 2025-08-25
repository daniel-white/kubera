use gateway_api::apis::standard::httproutes::{
    HTTPRouteRulesFiltersRequestHeaderModifier, HTTPRouteRulesFiltersRequestRedirect,
    HTTPRouteRulesFiltersRequestRedirectPath,
};
use thiserror::Error;
use vg_core::config::gateway::types::http::filters::{
    PathRewrite, PathRewriteType, RequestHeaderModifier, RequestHeaderModifierBuilder,
    RequestRedirect,
};

#[derive(Debug, Error)]
pub enum FilterConversionError {
    #[error("Invalid header name: {0}")]
    InvalidHeaderName(String),
    #[error("Invalid header value: {0}")]
    #[allow(dead_code)] // Future use for header validation
    InvalidHeaderValue(String),
}

/// Convert Gateway API `RequestRedirect` to Vale Gateway `RequestRedirect`
pub fn convert_request_redirect(
    gw_redirect: &HTTPRouteRulesFiltersRequestRedirect,
) -> RequestRedirect {
    let mut redirect = RequestRedirect {
        scheme: gw_redirect.scheme.as_ref().map(|s| {
            use gateway_api::httproutes::HTTPRouteRulesFiltersRequestRedirectScheme;
            match s {
                HTTPRouteRulesFiltersRequestRedirectScheme::Http => "http".to_string(),
                HTTPRouteRulesFiltersRequestRedirectScheme::Https => "https".to_string(),
            }
        }),
        hostname: gw_redirect.hostname.clone(),
        port: gw_redirect.port.map(|p| u16::try_from(p).unwrap_or(80)),
        path: None,
        status_code: gw_redirect
            .status_code
            .map(|code| u16::try_from(code).unwrap_or(302)),
    };

    // Convert path rewriting if present
    if let Some(path_config) = &gw_redirect.path {
        redirect.path = Some(convert_path_rewrite(path_config));
    }

    redirect
}

/// Convert Gateway API path rewrite configuration
fn convert_path_rewrite(gw_path: &HTTPRouteRulesFiltersRequestRedirectPath) -> PathRewrite {
    use gateway_api::apis::standard::httproutes::HTTPRouteRulesFiltersRequestRedirectPathType;

    match gw_path.r#type {
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
    }
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
            value: "vale-gateway".to_string(),
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

        // Convert to Vale Gateway RequestHeaderModifier
        let result = convert_request_header_modifier(&gw_modifier);
        assert!(result.is_ok(), "conversion should succeed");

        if let Ok(Some(modifier)) = result {
            // Check set headers
            assert!(modifier.set().is_some());
            if let Some(set) = modifier.set().as_ref() {
                assert_eq!(set.len(), 1);
                assert_eq!(set[0].name, "X-Gateway");
                assert_eq!(set[0].value, "vale-gateway");
            }

            // Check add headers
            assert!(modifier.add().is_some());
            if let Some(add) = modifier.add().as_ref() {
                assert_eq!(add.len(), 1);
                assert_eq!(add[0].name, "X-Request-ID");
                assert_eq!(add[0].value, "123");
            }

            // Check remove headers
            assert!(modifier.remove().is_some());
            if let Some(remove) = modifier.remove().as_ref() {
                assert_eq!(remove.len(), 1);
                assert_eq!(remove[0], "X-Debug");
            }
        } else {
            // Test failure: conversion should have returned Ok(Some(modifier))
            assert!(result.is_ok());
            if let Ok(opt) = result {
                assert!(opt.is_some(), "Expected modifier to be Some, got None");
            }
        }
    }
}
