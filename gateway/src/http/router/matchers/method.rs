use super::score::Score;
use super::Matcher;
use http::Method;
use std::collections::HashSet;
use tracing::{debug, instrument};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct MethodMatcher {
    pub methods: HashSet<Method>,
}

impl Matcher<Method> for MethodMatcher {
    #[instrument(
        skip(self, score, method),
        level = "debug",
        name = "MethodMatcher::matches"
        fields(matcher = ?self.methods)
    )]
    fn matches(&self, score: &Score, method: &Method) -> bool {
        let is_match = self.methods.contains(method);
        if is_match {
            debug!("Method matched: {}", method);
            score.score_method(self);
        }
        is_match
    }
}
