use std::net::IpAddr;
use http::HeaderValue;
use pingora::prelude::Session;
use tracing::warn;
use typed_builder::TypedBuilder;
use super::extractors::ClientAddrExtractorType;
use super::VALE_GATEWAY_CLIENT_IP_HEADER;

#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct ClientAddrFilterHandler {
    extractor: ClientAddrExtractorType,
}

impl ClientAddrFilterHandler {
    pub fn filter(&self, session: &mut Session) -> Option<IpAddr> {
        let extractor = self.extractor.extractor();
        if let Some(client_addr) = extractor.extract(session) {
            let headers = session.req_header_mut();
            headers
                .insert_header(VALE_GATEWAY_CLIENT_IP_HEADER,
                    HeaderValue::from_str(&client_addr.to_string()).unwrap(),
                )
                .unwrap_or_else(|err| {
                    warn!(
                        "Failed to insert header {}: {}",
                        VALE_GATEWAY_CLIENT_IP_HEADER, err
                    );
                });
            Some(client_addr)
        } else {
            let headers = session.req_header_mut();
            headers.remove_header(&VALE_GATEWAY_CLIENT_IP_HEADER); // **MUST** remove the header from the client if the address is not available
            None
        }
    }
}
