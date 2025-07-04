use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Mutex;

static REGEX_CACHE: Lazy<Mutex<HashMap<String, Regex>>> = Lazy::new(Mutex::default);

pub fn get_regex(pattern: &str) -> Regex {
    let mut map = REGEX_CACHE.lock().expect("Failed to lock regex cache");
    map.entry(pattern.to_string())
        .or_insert_with_key(|p| Regex::new(p).expect("Failed to compile regex"))
        .clone()
}
