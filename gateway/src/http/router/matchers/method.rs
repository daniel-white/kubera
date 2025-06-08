use super::Matcher;
use super::score::Score;
use http::Method;
use std::collections::HashSet;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct MethodMatcher {
    pub methods: HashSet<Method>,
}

impl Matcher<Method> for MethodMatcher {
    fn matches(&self, score: &Score, method: &Method) -> bool {
        let is_match = self.methods.contains(method);
        if is_match {
            score.score_method(self);
        }
        is_match
    }
}
