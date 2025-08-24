use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Hash, Eq, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpHostHeaderMatch {
    #[getset(get = "pub")]
    #[serde(rename = "type")]
    kind: HttpHostHeaderMatchKind,

    #[getset(get = "pub")]
    value: String,
}

impl HttpHostHeaderMatch {
    pub fn builder() -> HttpHostHeaderMatchBuilder {
        HttpHostHeaderMatchBuilder { result: None }
    }
}

#[derive(Debug)]
pub struct HttpHostHeaderMatchBuilder {
    result: Option<HttpHostHeaderMatch>,
}

impl HttpHostHeaderMatchBuilder {
    pub fn build(self) -> HttpHostHeaderMatch {
        self.result.expect("HttpHostHeaderMatch is not fully built")
    }

    pub fn exactly<H: Into<String>>(&mut self, host: H) -> &mut Self {
        self.result = Some(HttpHostHeaderMatch {
            kind: HttpHostHeaderMatchKind::Exact,
            value: host.into(),
        });
        self
    }

    pub fn with_suffix<S: Into<String>>(&mut self, suffix: S) -> &mut Self {
        self.result = Some(HttpHostHeaderMatch {
            kind: HttpHostHeaderMatchKind::Suffix,
            value: suffix.into(),
        });
        self
    }
}

#[derive(Validate, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HttpHostHeaderMatchKind {
    Exact,
    Suffix,
}
