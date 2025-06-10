use super::score::Score;
use super::Matcher;
use crate::util::get_regex;
use tracing::{debug, instrument};

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

impl Matcher<&str> for PathMatcher {
    #[instrument(
        skip(self, score, path),
        level = "debug",
        name = "PathMatcher::matches"
        fields(matcher = ?self)
    )]
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
            debug!("Path matched");
            score.score_path(self);
        }
        is_match
    }
}
