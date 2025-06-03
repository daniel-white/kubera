use getset::Getters;
use http::request::Parts;
use std::borrow::Cow;

#[derive(Debug, Getters, Clone)]
pub struct QueryParamsExtension<'a> {
    #[getset(get = "pub")]
    query_params: Vec<(Cow<'a, str>, Cow<'a, str>)>,
}
impl<'a> QueryParamsExtension<'a> {
    pub fn from_request(request: &'a Parts) -> Option<Self> {
        request
            .uri
            .query()
            .map(|qs| url::form_urlencoded::parse(qs.as_bytes()).collect())
            .map(|query_params: Vec<(Cow<str>, Cow<str>)>| QueryParamsExtension { query_params })
    }
}
