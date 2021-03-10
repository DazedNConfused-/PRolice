use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Custom wrapper for a GitHub's paginated response.
pub struct Page<T> {
    pub items: Vec<T>,
    pub incomplete_results: Option<bool>,
    pub total_count: Option<u64>,
    pub next: Option<Url>,
    pub prev: Option<Url>,
    pub first: Option<Url>,
    pub last: Option<Url>,
}
