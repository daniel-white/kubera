use std::sync::Arc;
use bytes::Bytes;
use http::{HeaderValue, Response, StatusCode};
use http::header::CONTENT_TYPE;
use typed_builder::TypedBuilder;
use vg_core::http::filters::static_response::{HttpStaticResponseBodyKey, HttpStaticResponseFilterKey};
use crate::http::filters::static_response::body_cache::HttpStaticResponseFilterBodyCacheClient;

#[derive(Debug, PartialEq, Eq, TypedBuilder)]
pub struct HttpStaticResponseFilterHandler {
    key: HttpStaticResponseFilterKey,
    status_code: StatusCode,
    body: Option<HttpStaticResponseFilterBody>
}

#[derive(Debug, PartialEq, Eq, TypedBuilder)]
pub struct HttpStaticResponseFilterBody {
    key: HttpStaticResponseBodyKey,
    content_type: HeaderValue,
    cache: HttpStaticResponseFilterBodyCacheClient,
}

impl HttpStaticResponseFilterHandler {
    pub async fn generate_response(&self) -> Response<Option<Arc<Bytes>>> {
        let mut response = Response::builder()
            .status(self.status_code);
        
        let response = match self.body.as_ref() {
            Some(body) => {
                response = response.header(CONTENT_TYPE, &body.content_type);
                let body = body.cache.get(&body.key).await;
                response.body(body)
            }
            None => {
                response.body(None)
            }
        };
        
        response.expect("Failed to build response")
    }
}