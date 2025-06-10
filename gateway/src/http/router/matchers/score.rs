use super::headers::HeadersMatcher;
use super::host_header::HostHeaderMatcher;
use super::method::MethodMatcher;
use super::path::PathMatcher;
use super::query_params::QueryParamsMatcher;
use std::cell::Cell;
use std::cmp::Ordering;
use tracing::instrument;

#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct Score {
    path_exact: Cell<bool>,
    path_length: Cell<Option<usize>>,
    method_match: Cell<bool>,
    headers_count: Cell<Option<usize>>,
    query_params_count: Cell<Option<usize>>,
}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Score {
    #[instrument(
        skip(self, other),
        level = "debug",
        name = "Score::cmp"
    )]
    fn cmp(&self, other: &Self) -> Ordering {
        if self == other {
            return Ordering::Equal;
        }

        match (self.path_exact.get(), other.path_exact.get()) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        };

        match (self.path_length.get(), other.path_length.get()) {
            (Some(len1), Some(len2)) => match len1.cmp(&len2) {
                Ordering::Less => return Ordering::Greater,
                Ordering::Greater => return Ordering::Less,
                Ordering::Equal => {}
            },
            (Some(_), None) => return Ordering::Less,
            _ => {}
        };

        match (self.method_match.get(), other.method_match.get()) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        };

        match (self.headers_count.get(), other.headers_count.get()) {
            (Some(count1), Some(count2)) => match count1.cmp(&count2) {
                Ordering::Less => return Ordering::Greater,
                Ordering::Greater => return Ordering::Less,
                Ordering::Equal => {}
            },
            (Some(_), None) => return Ordering::Less,
            _ => {}
        };

        match (
            self.query_params_count.get(),
            other.query_params_count.get(),
        ) {
            (Some(count1), Some(count2)) => match count1.cmp(&count2) {
                Ordering::Less => return Ordering::Greater,
                Ordering::Greater => return Ordering::Less,
                Ordering::Equal => {}
            },
            (Some(_), None) => return Ordering::Less,
            _ => {}
        };

        Ordering::Equal
    }
}

impl Score {
    pub fn score_host_header(&self, _matcher: &HostHeaderMatcher) {
        // Host header match is mandatory, so we don't need to score it
    }

    pub fn score_path(&self, matcher: &PathMatcher) {
        match matcher {
            PathMatcher::Exact(_) => {
                self.path_exact.replace(true);
            }
            PathMatcher::Prefix(prefix) => {
                self.path_length.replace(Some(prefix.len()));
            }
            PathMatcher::RegularExpression(pattern) => {
                self.path_length.replace(Some(pattern.len() * 4));
            }
        };
    }

    pub fn score_method(&self, _matcher: &MethodMatcher) {
        self.method_match.replace(true);
    }

    pub fn score_headers(&self, headers: &HeadersMatcher) {
        self.headers_count.replace(Some(headers.matchers.len()));
    }

    pub fn score_query_params(&self, query_params: &QueryParamsMatcher) {
        self.query_params_count
            .replace(Some(query_params.matchers.len()));
    }
}
