use crate::controllers::static_response_bodies_cache::StaticResponseBodiesCache;
use bytes::Bytes;
use getset::{CloneGetters, CopyGetters, Getters};
use http::header::{CONTENT_LENGTH, CONTENT_TYPE, SERVER};
use http::StatusCode;
use pingora::http::ResponseHeader;
use pingora::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};
use typed_builder::TypedBuilder;
pub(crate) use vg_core::config::gateway::types::net::StaticResponse;
use vg_core::config::gateway::types::GatewayConfiguration;
use vg_core::continue_on;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_macros::await_ready;

/// Filter for handling static HTTP responses based on StaticResponse configuration.
///
/// This filter intercepts requests matching configured routes and returns static responses
/// without forwarding to upstream services. It supports configurable status codes and
/// response bodies with custom content types.
///
/// # Features
///
/// - **Status code control**: Return any valid HTTP status code
/// - **Content type support**: Set appropriate Content-Type headers for response bodies
/// - **Body content**: Support for static response bodies with configurable identifiers
/// - **Route integration**: Works seamlessly with Gateway API route filters
///
/// # Usage
///
/// The filter is automatically applied when a route contains an `ExtStaticResponse` filter
/// with a key that matches a configured static response in the gateway configuration.
///
/// # Examples
///
/// ```rust,ignore
/// // The filter will look up static responses by key and return the configured response
/// let filter = StaticResponseFilter::new(static_responses_map);
///
/// // When a request matches a route with ext_static_response.key = "maintenance"
/// // The filter will return the configured status code and body for that key
/// ```
#[derive(CloneGetters, TypedBuilder)]
pub struct StaticResponseFilter {
    #[getset(get_clone = "pub")]
    responses: Arc<HashMap<String, StaticResponse>>,
    #[getset(get_clone = "pub")]
    static_response_bodies: StaticResponseBodiesCache,
}

#[derive(CopyGetters, TypedBuilder)]
pub struct FullStaticResponse {
    #[getset(get_copy = "pub")]
    status_code: StatusCode,
    #[getset(get = "pub")]
    #[builder(setter(into))]
    version_key: String,
    body: Option<FullStaticResponseBody>,
}

impl FullStaticResponse {
    pub fn body(&self) -> Option<&FullStaticResponseBody> {
        self.body.as_ref()
    }
}

#[derive(TypedBuilder, CloneGetters, Getters)]
pub struct FullStaticResponseBody {
    #[getset(get = "pub")]
    content_type: String,
    #[getset(get = "pub")]
    identifier: String,
    #[getset(get_clone = "pub")]
    content: Arc<Bytes>,
}

impl StaticResponseFilter {
    /// Attempts to generate a static response for the given key.
    ///
    /// # Arguments
    ///
    /// * `key` - The static response key to look up
    ///
    /// # Returns
    ///
    /// * `Some(StaticResponse)` if a matching static response configuration is found
    /// * `None` if no matching configuration exists for the given key
    async fn get_full_response(&self, key: &str) -> Option<FullStaticResponse> {
        let response = self.responses.get(key)?;

        let builder = FullStaticResponse::builder()
            .status_code(StatusCode::from_u16(*response.status_code()).unwrap())
            .version_key(response.version_key());

        let response = match response.body() {
            Some(body) => {
                // TODO retrieve from control plane cache instead of generating new content
                let (content_type, content) = self.static_response_bodies.get(key).await?;
                let body = FullStaticResponseBody::builder()
                    .content_type(content_type)
                    .identifier(body.identifier().clone())
                    .content(content)
                    .build();
                builder.body(Some(body)).build()
            }
            None => builder.body(None).build(),
        };

        Some(response)
    }

    /// Applies the static response filter to a Pingora session.
    ///
    /// This method generates the appropriate HTTP response and writes it directly
    /// to the session, bypassing any upstream processing.
    ///
    /// # Arguments
    ///
    /// * `session` - The Pingora HTTP session to write the response to
    /// * `key` - The static response key to look up and apply
    ///
    /// # Returns
    ///
    /// * `Ok(true)` if a static response was applied successfully
    /// * `Ok(false)` if no matching static response configuration was found
    /// * `Err(...)` if there was an error writing the response
    pub async fn apply_to_session(&self, session: &mut Session, key: &str) -> Result<bool> {
        if let Some(static_response) = self.get_full_response(key).await {
            debug!(
                "Applying static response for key: {} with status: {}",
                key,
                static_response.status_code()
            );

            let response = http::Response::builder()
                .status(static_response.status_code())
                .header(SERVER, "Vale Gateway");

            let response = match static_response.body() {
                Some(body) => response
                    .header(CONTENT_TYPE, body.content_type())
                    .header(CONTENT_LENGTH, body.content().len())
                    .body(Some(body.content().as_ref().clone()))
                    .expect("Failed to build static response"),
                None => response
                    .body(None)
                    .expect("Failed to build static response without body"),
            };

            let (headers, body) = response.into_parts();
            let response_header: ResponseHeader = headers.into();
            session
                .write_response_header(Box::new(response_header), false)
                .await?;
            session.write_response_body(body, true).await?;

            Ok(true)
        } else {
            warn!("Static response key '{}' not found in configuration", key);
            Ok(false)
        }
    }
}

/// Collector function that monitors gateway configuration changes and builds static response data.
///
/// This function creates a background task that watches for configuration changes and maintains
/// an up-to-date map of static response configurations indexed by their keys.
pub fn static_responses(
    task_builder: &TaskBuilder,
    gateway_configuration_rx: &Receiver<GatewayConfiguration>,
) -> Receiver<Arc<HashMap<String, StaticResponse>>> {
    let (tx, rx) = signal();
    let gateway_configuration_rx = gateway_configuration_rx.clone();

    task_builder
        .new_task(stringify!(collect_static_responses))
        .spawn(async move {
            loop {
                await_ready!(gateway_configuration_rx)
                    .and_then(async |gateway_configuration| {
                        if let Some(static_responses) = gateway_configuration.static_responses() {
                            let responses_map: HashMap<String, StaticResponse> = static_responses
                                .responses()
                                .iter()
                                .map(|static_response| {
                                    (static_response.key().clone(), static_response.clone())
                                })
                                .collect();

                            tx.set(Arc::new(responses_map)).await;
                        } else {
                            tx.set(Arc::new(HashMap::new())).await;
                        }
                    })
                    .run()
                    .await;

                continue_on!(gateway_configuration_rx.changed());
            }
        });

    rx
}
