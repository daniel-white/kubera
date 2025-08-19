use super::score::HttpRouteRuleMatchesScore;
use super::Match;
use http::Method;
use tracing::{debug, instrument};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct MethodMatch {
    pub(crate) method: Method,
}

impl Match<Method> for MethodMatch {
    #[instrument(
        skip(self, score, method),
        name = "MethodMatcher::matches"
        fields(match = ?self)
    )]
    fn matches(&self, score: &HttpRouteRuleMatchesScore, method: &Method) -> bool {
        let is_match = self.method == *method;
        if is_match {
            debug!("Method matched");
            score.method(self);
        }
        is_match
    }
}
