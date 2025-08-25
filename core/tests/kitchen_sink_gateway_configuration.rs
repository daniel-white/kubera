use std::net::SocketAddr;
use url::Url;
use vg_core::gateways::Gateway;
use vg_core::http::filters::access_control::{
    HttpAccessControlClients, HttpAccessControlFilter, HttpAccessControlFilterRef,
};
use vg_core::http::filters::client_addrs::{HttpClientAddrsFilter, HttpClientAddrsFilterRef};
use vg_core::http::filters::error_response::{
    HttpErrorResponseFilter, HttpErrorResponseFilterRef, HttpProblemDetailErrorResponse,
};
use vg_core::http::filters::header_modifier::HttpHeaderModifierFilter;
use vg_core::http::filters::redirect::{HttpRedirectFilter, HttpRedirectPathRewrite};
use vg_core::http::filters::static_response::{HttpStaticResponseBody, HttpStaticResponseFilter};
use vg_core::ipc::IpcConfiguration;

#[test]
fn kitchen_sink_gateway_configuration() {
    use http::{HeaderName, HeaderValue, StatusCode};
    use std::net::IpAddr;
    use std::num::NonZeroU16;
    use std::str::FromStr;
    use vg_core::http::filters::access_control::HttpAccessControlEffect;
    use vg_core::http::filters::error_response::HttpErrorResponseKind;
    use vg_core::http::filters::redirect::HttpRedirectKind;
    use vg_core::http::listeners::{HttpFilterDefinition, HttpListenerFilter};
    use vg_core::net::Port;

    // Build IPC configuration
    let mut ipc_config = IpcConfiguration::builder();
    let ipc: SocketAddr = "127.0.0.1:9000".parse().expect("Invalid IPC address");
    ipc_config.addr(ipc);
    let ipc_config = ipc_config.build();

    // Representative values for filters
    let mut header_mod = HttpHeaderModifierFilter::builder();
    header_mod
        .add_header("x-add-header", HeaderValue::from_static("add-value"))
        .set_header("x-set-header", HeaderValue::from_static("set-value"))
        .remove_header(HeaderName::from_static("x-remove-header"));
    let header_mod = header_mod.build();

    let redirect = HttpRedirectFilter::builder()
        .kind(HttpRedirectKind::Temporary)
        .path(HttpRedirectPathRewrite::PrefixMatch("/v2".to_string()))
        .build();

    let access_clients = HttpAccessControlClients::builder()
        .ips(vec![IpAddr::from_str("127.0.0.1").unwrap()])
        .build();

    let access_control = HttpAccessControlFilter::builder()
        .key("access-key")
        .effect(HttpAccessControlEffect::Allow)
        .clients(access_clients)
        .build();

    let mut client_addrs = HttpClientAddrsFilter::builder();
    client_addrs
        .key("client-key")
        .trust_header(HeaderName::from_static("x-real-ip"));
    let client_addrs = client_addrs.build();

    let error_response = HttpErrorResponseFilter::builder()
        .key("error-key")
        .kind(HttpErrorResponseKind::Html)
        .build();

    let error_response_problem_detail = HttpErrorResponseFilter::builder()
        .key("error-key-problem-detail")
        .kind(HttpErrorResponseKind::ProblemDetail)
        .problem_detail(
            HttpProblemDetailErrorResponse::builder()
                .authority(Url::from_str("https://example.com/").unwrap())
                .build(),
        )
        .build();

    let static_body = HttpStaticResponseBody::builder()
        .key("body-key")
        .content_type(HeaderValue::from_static("text/plain"))
        .build();
    let static_response = HttpStaticResponseFilter::builder()
        .key("static-key")
        .status_code(StatusCode::OK)
        .body(static_body)
        .build();

    let access_control_ref = HttpAccessControlFilterRef::builder()
        .key(access_control.key().clone())
        .build();

    let client_addrs_ref = HttpClientAddrsFilterRef::builder()
        .key(client_addrs.key().clone())
        .build();

    let error_response_ref = HttpErrorResponseFilterRef::builder()
        .key(error_response.key().clone())
        .build();

    let pd_error_response_ref = HttpErrorResponseFilterRef::builder()
        .key(error_response_problem_detail.key().clone())
        .build();

    // Build Gateway with everything
    let mut gateway_builder = Gateway::builder();
    gateway_builder.with_ipc(ipc_config);
    gateway_builder.with_http_listener(|listener| {
        listener
            .name("main-listener")
            .port(Port::new(NonZeroU16::new(8080).unwrap()))
            .add_filter(HttpListenerFilter::UpstreamRequestHeaderModifier(
                header_mod.clone(),
            ))
            .add_filter(HttpListenerFilter::ResponseHeaderModifier(
                header_mod.clone(),
            ))
            .add_filter(HttpListenerFilter::Redirect(redirect.clone()))
            .add_filter(HttpListenerFilter::AccessControl(access_control_ref))
            .add_filter(HttpListenerFilter::ClientAddrs(client_addrs_ref))
            .add_filter(HttpListenerFilter::ErrorResponse(error_response_ref))
            .add_filter(HttpListenerFilter::ErrorResponse(pd_error_response_ref))
            .add_filter_definition(HttpFilterDefinition::UpstreamRequestHeaderModifier(
                header_mod.clone(),
            ))
            .add_filter_definition(HttpFilterDefinition::ResponseHeaderModifier(
                header_mod.clone(),
            ))
            .add_filter_definition(HttpFilterDefinition::Redirect(redirect.clone()))
            .add_filter_definition(HttpFilterDefinition::StaticResponse(
                static_response.clone(),
            ))
            .add_filter_definition(HttpFilterDefinition::AccessControl(access_control.clone()))
            .add_filter_definition(HttpFilterDefinition::ClientAddrs(client_addrs.clone()))
            .add_filter_definition(HttpFilterDefinition::ErrorResponse(error_response.clone()))
            .add_filter_definition(HttpFilterDefinition::ErrorResponse(
                error_response_problem_detail.clone(),
            ))
            .add_route("my-route", |route| {
                route
                    .add_host_header_match(|h| {
                        h.with_suffix(".example.com");
                    })
                    .add_rule("my-rule", |rule| {
                        rule.add_match(|m| {
                            m.add_exact_header(
                                HeaderName::from_static("content-type"),
                                HeaderValue::from_static("application/json"),
                            );
                        });
                    });
            });
    });
    let actual = gateway_builder.build();

    let expected = serde_yaml::from_str(include_str!("cases/kitchen_sink_configuration.yaml"))
        .expect("Failed to deserialize expected YAML");

    assert_eq!(actual, expected);
}
