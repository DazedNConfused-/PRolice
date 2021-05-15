//!A universal, project-wide error wrapper that is also able to retain the nested cause of an [`Error`].

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnalyzeError {
    // :# prints causes as well using anyhow's default formatting of causes
    #[error("Error during async task execution; nested = {0:#}")]
    AsyncTaskError(anyhow::Error),
    #[error("Error parsing diff for [{repo_name}/{pr_number}]; nested = {nested:#?}")]
    DiffParseError {
        repo_name: String,
        pr_number: u64,
        #[source]
        nested: anyhow::Error,
    },
    #[error("GitHub API error: {msg}; nested = {nested:#?}")]
    GitHubAPIError {
        msg: String,
        #[source]
        nested: anyhow::Error,
    },
    #[error("GitHub API response body error: {msg}; nested = {nested:#?}")]
    GitHubAPIResponseBodyError {
        msg: String,
        #[source]
        nested: anyhow::Error,
    },
    #[error("JSON parse error: {msg}; nested = {nested:#?}")]
    JsonParseError {
        msg: String,
        #[source]
        nested: anyhow::Error,
    },
    #[error("Parsed commits' JSON produced an array with zero elements! At least one commit should exist in a PR.")]
    NoCommitsFoundError,
    #[error(
        "An unrecoverable error has occurred in one or more data-fetching steps for [{repo_name}]/[{pr_number}] and operation had to be aborted mid-process; nested = {nested:#?}"
    )]
    PullRequestDataRetrievalError {
        repo_name: String,
        pr_number: u64,
        #[source]
        nested: anyhow::Error,
    },
    #[error("Incomplete data for PR #{pr_number}: {reason}")]
    PullRequestIncompleteDataError {
        reason: String,
        pr_number: u64,
    },
    #[error(
        "Could not retrieve PR#[{pr_number}] for repository [{repo_name}]; nested = {nested:#?}"
    )]
    PullRequestNotFound {
        repo_name: String,
        pr_number: u64,
        #[source]
        nested: anyhow::Error,
    },
    #[error("Repository initialization error = {0}")]
    RepositoryNotFoundError(String),
    #[error("Report-template rendering error: {msg}; nested = {nested:#?}")]
    TemplateRenderError {
        msg: String,
        #[source]
        nested: anyhow::Error,
    },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[macro_export]
/// Wraps a dynamic error type into an [`anyhow::Error`]. Useful in a plethora of cases for constructing
/// [`AnalyzeError`]s.
macro_rules! nested {
    ($source:expr) => {
        anyhow::Error::new($source)
    };
}
