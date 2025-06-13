use super::Match;
use super::score::HttpRouteRuleMatchesScore;
use http::Method;
use tracing::{debug, instrument};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct MethodMatch {
    pub(super) method: Method,
}

impl Match<Method> for MethodMatch {
    #[instrument(
        skip(self, score, method),
        level = "debug",
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
