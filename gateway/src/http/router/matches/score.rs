use super::headers::HeadersMatch;
use super::host_header::HostHeaderMatch;
use super::method::MethodMatch;
use super::path::PathMatch;
use super::query_params::QueryParamsMatch;
use std::cell::Cell;
use std::cmp::Ordering;
use tracing::instrument;

#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct MatchingScore {
    path_exact: Cell<bool>,
    path_length: Cell<Option<usize>>,
    method: Cell<bool>,
    headers_count: Cell<Option<usize>>,
    query_params_count: Cell<Option<usize>>,
}

impl PartialOrd for MatchingScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MatchingScore {
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

        match (self.method.get(), other.method.get()) {
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

impl MatchingScore {
    pub fn host_header(&self, _host_header_match: &HostHeaderMatch) {
        // Host header match is mandatory, so we don't need to score it
    }

    pub fn path(&self, path_match: &PathMatch) {
        match path_match {
            PathMatch::Exact(_) => {
                self.path_exact.replace(true);
            }
            PathMatch::Prefix(prefix) => {
                self.path_length.replace(Some(prefix.len()));
            }
            PathMatch::RegularExpression(pattern) => {
                self.path_length.replace(Some(pattern.len() * 4));
            }
        };
    }

    pub fn method(&self, _method_match: &MethodMatch) {
        self.method.replace(true);
    }

    pub fn headers(&self, _headers_match: &HeadersMatch, header_params_count: usize) {
        self.headers_count.replace(Some(header_params_count));
    }

    pub fn query_params(&self, _query_params_match: &QueryParamsMatch, query_params_count: usize) {
        self.query_params_count
            .replace(Some(query_params_count));
    }
}
