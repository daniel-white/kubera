use super::score::HttpRouteRuleMatchesScore;
use super::Match;
use crate::util::get_regex;
use tracing::{debug, instrument};

/// Path matching strategies for HTTP routes.
///
/// This enum defines the different ways a route can match against request paths,
/// with support for capturing matched prefix information for redirect filters.
///
/// # Variants
///
/// - `Exact(String)` - Matches the path exactly
/// - `Prefix(String)` - Matches if the request path starts with the prefix
/// - `RegularExpression(String)` - Matches using a regular expression
///
/// Only `Prefix` matches provide matched prefix information for `replacePrefixMatch`
/// redirect functionality, as exact matches and regex matches don't have meaningful
/// prefixes to replace.
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

/// Result of a path match operation, including matched prefix information.
///
/// This struct is returned by enhanced path matching operations and contains
/// both the match result and any prefix information that can be used by
/// redirect filters implementing `replacePrefixMatch`.
///
/// # Fields
///
/// * `matched` - Whether the path matched the route pattern
/// * `matched_prefix` - The actual prefix that was matched (only for Prefix matches)
///
/// # Examples
///
/// ```rust,ignore
/// // For a Prefix("/api/v1") matching path "/api/v1/users"
/// PathMatchResult {
///     matched: true,
///     matched_prefix: Some("/api/v1".to_string()),
/// }
///
/// // For an Exact("/health") matching path "/health"
/// PathMatchResult {
///     matched: true,
///     matched_prefix: None, // Exact matches don't provide prefix info
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PathMatchResult {
    /// Whether the path matched
    pub matched: bool,
    /// The actual prefix that was matched (for Prefix matches only)
    pub matched_prefix: Option<String>,
}

impl PathMatch {
    /// Get the matched prefix for this path match type.
    ///
    /// This method extracts prefix information that can be used by redirect filters
    /// implementing `replacePrefixMatch`. Only prefix matches provide this information.
    ///
    /// # Arguments
    ///
    /// * `path` - The request path to check against
    ///
    /// # Returns
    ///
    /// - `Some(prefix)` for Prefix matches where the path starts with the prefix
    /// - `None` for Exact matches, Regex matches, or non-matching Prefix patterns
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let prefix_match = PathMatch::Prefix("/api/v1".to_string());
    /// assert_eq!(
    ///     prefix_match.get_matched_prefix("/api/v1/users"),
    ///     Some("/api/v1".to_string())
    /// );
    ///
    /// let exact_match = PathMatch::Exact("/health".to_string());
    /// assert_eq!(exact_match.get_matched_prefix("/health"), None);
    /// ```
    pub fn get_matched_prefix(&self, path: &str) -> Option<String> {
        match self {
            PathMatch::Prefix(prefix) if path.starts_with(prefix) => Some(prefix.clone()),
            PathMatch::Prefix(_) => None, // Prefix that doesn't match the path
            // Only return matched prefix for Prefix matches, not Exact matches
            PathMatch::Exact(_) => None,
            PathMatch::RegularExpression(_) => None,
        }
    }

    /// Enhanced matching that returns both match result and matched prefix.
    ///
    /// This method performs the path matching operation and also captures any
    /// prefix information that can be used by redirect filters. It's the primary
    /// method used by the routing system to support `replacePrefixMatch`.
    ///
    /// # Arguments
    ///
    /// * `score` - Routing score tracker for precedence calculations
    /// * `path` - The request path to match against
    ///
    /// # Returns
    ///
    /// A `PathMatchResult` containing both the match status and any matched prefix.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let matcher = PathMatch::Prefix("/api/v1".to_string());
    /// let score = HttpRouteRuleMatchesScore::default();
    ///
    /// let result = matcher.matches_with_result(&score, &"/api/v1/users");
    /// assert!(result.matched);
    /// assert_eq!(result.matched_prefix, Some("/api/v1".to_string()));
    /// ```
    #[instrument(
        skip(self, score, path),
        name = "PathMatch::matches_with_result"
        fields(matcher = ?self)
    )]
    pub fn matches_with_result(
        &self,
        score: &HttpRouteRuleMatchesScore,
        path: &&str,
    ) -> PathMatchResult {
        let is_match = match self {
            PathMatch::Exact(expected_path) => expected_path == path,
            PathMatch::Prefix(prefix) => path.starts_with(prefix),
            PathMatch::RegularExpression(pattern) => {
                let regex = get_regex(pattern);
                regex.is_match(path)
            }
        };

        let matched_prefix = if is_match {
            self.get_matched_prefix(path)
        } else {
            None
        };

        if is_match {
            debug!("Path matched with prefix: {:?}", matched_prefix);
            score.path(self);
        }

        PathMatchResult {
            matched: is_match,
            matched_prefix,
        }
    }
}

impl Match<&str> for PathMatch {
    /// Basic path matching without prefix information capture.
    ///
    /// This method provides compatibility with the existing `Match` trait
    /// and is used when prefix information is not needed.
    ///
    /// For full functionality including prefix capture for redirects,
    /// use `matches_with_result` instead.
    #[instrument(
        skip(self, score, path),
        name = "PathMatch::matches"
        fields(matcher = ?self)
    )]
    fn matches(&self, score: &HttpRouteRuleMatchesScore, path: &&str) -> bool {
        self.matches_with_result(score, path).matched
    }
}
