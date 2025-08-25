use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use url::Url;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Hash, Eq)]
#[serde(transparent)]
pub struct HttpErrorResponseFilterKey(String);

impl<S: AsRef<str>> From<S> for HttpErrorResponseFilterKey {
    fn from(value: S) -> Self {
        Self(value.as_ref().to_string())
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TypedBuilder, Getters,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpErrorResponseFilterRef {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    key: HttpErrorResponseFilterKey,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HttpErrorResponseKind {
    Empty,
    #[default]
    Html,
    ProblemDetail,
}

#[derive(
    Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters, TypedBuilder, Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpErrorResponseFilter {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    key: HttpErrorResponseFilterKey,

    #[getset(get = "pub")]
    kind: HttpErrorResponseKind,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub")]
    #[builder(default, setter(strip_option))]
    problem_detail: Option<HttpProblemDetailErrorResponse>,
}

#[derive(
    Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Eq, Getters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpProblemDetailErrorResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub")]
    #[builder(setter(into, strip_option), default)]
    #[schemars(schema_with = "crate::schemars::url")]
    authority: Option<Url>,
}
