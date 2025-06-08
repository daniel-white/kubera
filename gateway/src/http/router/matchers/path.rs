use super::Matcher;
use super::score::Score;
use crate::util::get_regex;

#[derive(Debug, PartialEq, Clone)]
pub enum PathMatcher {
    Exact(String),
    Prefix(String),
    RegularExpression(String),
}

impl Default for PathMatcher {
    fn default() -> Self {
        PathMatcher::Prefix("/".to_string())
    }
}

impl PathMatcher {
    pub fn is_default(&self) -> bool {
        matches!(self, PathMatcher::Prefix(prefix) if prefix == "/")
    }
}

impl Matcher<&str> for PathMatcher {
    fn matches(&self, score: &Score, path: &&str) -> bool {
        let is_match = match self {
            PathMatcher::Exact(expected_path) => expected_path == path,
            PathMatcher::Prefix(prefix) => path.starts_with(prefix),
            PathMatcher::RegularExpression(pattern) => {
                let regex = get_regex(pattern);
                regex.is_match(path)
            }
        };
        if is_match {
            score.score_path(self);
        }
        is_match
    }
}
