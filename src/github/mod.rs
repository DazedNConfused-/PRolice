//! GitHub's wrappers & miscellaneous utilities. Everything ranging from a custom [`GitHubConnectionPoolManager`](client::pool::GitHubConnectionPool)
//! to an *opinionated* [`Repository`](octocrab::models::Repository)/[`PullRequest`](octocrab::models::pulls::PullRequest)
//! [`Analyzer`](utils::analyzer::Analyzer) is found in this module.

pub mod json;

pub mod client;

pub mod utils;
