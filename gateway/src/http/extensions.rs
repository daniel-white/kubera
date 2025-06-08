use getset::Getters;
use http::request::Parts;
use http::Uri;

#[derive(Debug, Getters, Default, Clone)]
pub struct QueryParamsExtension {
    #[getset(get = "pub")]
    query_params: Vec<(String, String)>,
}
impl QueryParamsExtension {
    pub fn from_uri(uri: &Uri) -> QueryParamsExtension {
         uri
            .query()
            .map(|qs| Self {
                query_params: url::form_urlencoded::parse(qs.as_bytes())
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            })
            .unwrap_or_default()
    }
}
