use super::Match;
use super::score::HttpRouteRuleMatchesScore;
use crate::util::get_regex;
use tracing::{debug, instrument};

#[derive(Debug, PartialEq, Clone)]
pub enum PathMatch {
    Exact(String),
    Prefix(String),
    RegularExpression(String),
}

impl Default for PathMatch {
    fn default() -> Self {
        Self::Prefix("/".to_string())
    }
}

impl Match<&str> for PathMatch {
    #[instrument(
        skip(self, score, path),
        level = "debug",
        name = "PathMatch::matches"
        fields(matcher = ?self)
    )]
    fn matches(&self, score: &HttpRouteRuleMatchesScore, path: &&str) -> bool {
        let is_match = match self {
            PathMatch::Exact(expected_path) => expected_path == path,
            PathMatch::Prefix(prefix) => path.starts_with(prefix),
            PathMatch::RegularExpression(pattern) => {
                let regex = get_regex(pattern);
                regex.is_match(path)
            }
        };
        if is_match {
            debug!("Path matched");
            score.path(self);
        }
        is_match
    }
}
