//! [`Repository`] and [`PullRequest`] analyzing utilities.

use std::convert::TryFrom;

use chrono::{DateTime, Utc};
use deadpool::managed::Pool;
use futures::future::join_all;
use log::{debug, error, info, trace, warn};
use octocrab::models::issues::Comment;
use octocrab::models::pulls::PullRequest;
use octocrab::models::Repository;
use octocrab::{params, Octocrab, Page};
use reqwest::Url;
use time::Instant;
use tokio::task::JoinHandle;
use tokio::try_join;
use unidiff::PatchSet;

use prpolice_lib::prolice_trace_time;

use crate::github;
use crate::github::client::connector::{GitHubConnection, GitHubConnector};
use crate::github::client::pool::{GitHubConnectionPool, GitHubPoolError};
use crate::github::json::commit::CommitRoot;
use crate::github::json::commit_comment::CommitComment;
use crate::github::json::review::Review;
use crate::github::utils::pull_request_data::{PullRequestData, PullRequestDataResult};
use crate::github::utils::repository_data::RepositoryData;
use crate::nested;
use crate::prolice_error::AnalyzeError;

/// A builder for an [`Analyzer`] instance.
pub struct AnalyzerBuilder {
    owner: String,
    repository_name: String,
    github_personal_access_token: String,
    connection_pool: &'static GitHubConnectionPool,
}

impl GitHubConnector for AnalyzerBuilder {
    // AnalyzerBuilder uses a single connection to initialize a proper Repository instance from the
    // supplied repository_name, and thus implements GitHub connection for easier access to the pool
    fn get_connection_pool(&self) -> &GitHubConnectionPool {
        self.connection_pool
    }
}

impl AnalyzerBuilder {
    pub fn new(
        owner: &str, repository_name: &str, github_personal_access_token: &str,
        connection_pool: &'static GitHubConnectionPool,
    ) -> Self {
        AnalyzerBuilder {
            owner: owner.to_string(),
            repository_name: repository_name.to_string(),
            github_personal_access_token: github_personal_access_token.to_string(),
            connection_pool,
        }
    }

    /// Instantiates a new [`Analyzer`] instance under the given `owner` - which can be either an individual
    /// or an organization - and for the target `repository_name`.
    ///
    /// In the case of private organizations (or if you want to analyze private repositories belonging
    /// to an individual); the underlying `connection_pool` must have been instantiated with a
    /// [personal access token](https://docs.github.com/en/github/authenticating-to-github/creating-a-personal-access-token)
    /// that has read access for the intended target(s).
    pub async fn init(&self) -> Result<Analyzer, AnalyzeError> {
        debug!("Initializing Analyzer for {}:{}...", self.owner, self.repository_name);

        let github_connection = self.get_github_client().await;

        let repository_page = github_connection
            .orgs(&self.owner)
            .list_repos()
            .repo_type(params::repos::Type::All)
            .sort(params::repos::Sort::Pushed)
            .send()
            .await;

        if let Ok(repository_page) = repository_page {
            // we found the owner as an organization; now we will query the target repository...

            let repository = repository_page
                .items
                .into_iter()
                .find(|repo| repo.name.eq_ignore_ascii_case(&self.repository_name));

            return if let Some(repository) = repository {
                Ok(Analyzer::new(
                    &self.owner,
                    repository,
                    &self.github_personal_access_token,
                    &self.connection_pool,
                ))
            } else {
                Err(AnalyzeError::RepositoryNotFoundError(format!(
                    "Could not find repository [{}] under organization [{}] (is it misspelled?)",
                    &self.repository_name, &self.owner
                )))
            };
        }

        debug!("Could not find repository [{}] under owner [{}] as an organization. Retrying search as individual user...",  &self.repository_name, &self.owner);

        let personal_repo = self.find_personal_repository(&github_connection).await?;

        if let Some(repository) = personal_repo {
            return Ok(Analyzer::new(
                &self.owner,
                repository,
                &self.github_personal_access_token,
                &self.connection_pool,
            ));
        }

        return Err(AnalyzeError::RepositoryNotFoundError(format!(
            "Could not find repository [{}] under owner [{}] (is it misspelled?)",
            &self.repository_name, &self.owner
        )));
    }

    async fn find_personal_repository(
        &self, github_connection: &GitHubConnection,
    ) -> Result<Option<Repository>, AnalyzeError> {
        let url = format!(
            "{github_base_url}search/repositories?q=user:{user}&access_token={personal_access_token}",
            github_base_url = github_connection.base_url.as_str(),
            user = self.owner,
            personal_access_token = self.github_personal_access_token
        );

        let builder = github_connection.request_builder(&url, reqwest::Method::GET);
        let response = github_connection
            .execute(builder)
            .await
            .map_err(|e| {
                trace!("Error = {:?}", e);
                AnalyzeError::GitHubAPIError {
                    msg: format!(
                        "Error searching for owner's [{}] repositories in [{}].",
                        self.owner, &url
                    ),
                    nested: nested!(e),
                }
            })
            .unwrap();

        if response.content_length().is_some() && response.content_length().unwrap() == 0 {
            warn!(
                "No content received while searching for owner's [{}] repositories in [{}].",
                self.owner, &url
            );
            return Ok(None);
        }

        let raw_response_text = response.text().await.map_err(|e| {
            trace!("Error = {:?}", e);
            AnalyzeError::GitHubAPIResponseBodyError {
                msg: format!(
                    "Error retrieving repositories' JSON for owner's [{}] repositories in [{}].",
                    self.owner, &url
                ),
                nested: nested!(e),
            }
        })?;

        let parsed_json: github::json::page::Page<Repository> =
            serde_json::from_str(&raw_response_text).map_err(|e| {
                trace!("Error = {:?}", e);
                trace!("Raw response = {}", raw_response_text);
                AnalyzeError::JsonParseError {
                    msg: format!(
                        "Error mapping repositories' JSON for owner's [{}] repositories in [{}].",
                        self.owner, &url
                    ),
                    nested: nested!(e),
                }
            })?;

        let target_repo =
            parsed_json.items.into_iter().find(|repo| repo.name == self.repository_name);

        return Ok(target_repo);
    }
}

/// A [`Repository`] and [`PullRequest`] analyzer.
pub struct Analyzer {
    owner: String,
    repository: Repository,
    github_personal_access_token: String,
    connection_pool: &'static GitHubConnectionPool,
}

impl GitHubConnector for Analyzer {
    fn get_connection_pool(&self) -> &GitHubConnectionPool {
        self.connection_pool
    }
}

impl Clone for Analyzer {
    fn clone(&self) -> Self {
        Analyzer::new(
            &self.owner,
            self.repository.clone(),
            &self.github_personal_access_token,
            self.connection_pool,
        )
    }

    fn clone_from(&mut self, source: &Self) {
        self.owner = source.owner.clone();
        self.repository = source.repository.clone();
        self.connection_pool = source.connection_pool;
    }
}

impl Analyzer {
    /// Retrieves a set amount of [`PullRequest`]s - in the form of [`PullRequestDataResult`], from
    /// this [`Analyzer`]'s [`Repository`].
    /// The number of retrieved [`PullRequest`]s is determined by the `sample_size` parameter.
    pub async fn retrieve_repo_data(&self, sample_size: u8) -> RepositoryData {
        let start = Instant::now();

        // crawl all pull-requests under repository
        let repo = self.repository();
        let github_connection = self.get_github_client().await;

        let prs = github_connection
            .pulls(&self.owner, &repo.name)
            .media_type(octocrab::params::pulls::MediaType::Full)
            .list()
            // filtering parameters
            .state(params::State::Closed)
            .sort(params::pulls::Sort::Created)
            .direction(params::Direction::Descending)
            .per_page(sample_size)
            .page(1u32)
            .send()
            .await
            .unwrap_or_else(|e| {
                error!("Could not retrieve PRs for repository [{}]. Aborting operation.", &repo.name);
                panic!(e)
            })
            .items;

        info!("Analyzing repository [{}] using a sample of [{}] PRs...", repo.name, prs.len());

        let analysis_tasks: Vec<JoinHandle<PullRequestDataResult>> = prs
            .iter()
            .map(|pr| {
                let pr = pr.clone(); // async processing needs its own unshared pr reference for the whole duration of the thread
                let child_pr_analyzer = self.clone();

                tokio::spawn(async move { child_pr_analyzer.retrieve_pr_data_from(&pr).await })
            })
            .collect();

        let results: Vec<PullRequestDataResult> = join_all(analysis_tasks)
            .await
            .into_iter()
            .map(|async_task_operation_result| {
                async_task_operation_result.unwrap_or_else(|e| {
                    error!(
                        "There was a problem during async PR-data-retrieval task. Aborting operation.",
                    );
                    trace!("Error = {:?}", e);
                    Err(AnalyzeError::AsyncTaskError(nested!(e)))
                })
            })
            .collect();
        info!("Finished fetching [{}] sample PRs for [{}].", results.len(), repo.name);

        let errors: Vec<&AnalyzeError> = results
            .iter()
            .filter(|result| result.is_err())
            .map(|pull_request_error_result| pull_request_error_result.as_ref().err().unwrap())
            .collect();

        if !errors.is_empty() {
            error!("There were [{}] PRs whose data-retrieval process ended in error and therefore could not be successfully fetched:", errors.len());
            errors.iter().for_each(|e| {
                error!("{}", e);
            });
        }

        let duration = start.elapsed();
        info!("Time elapsed retrieving data for [{}] was: {:?}", repo.name, duration);

        return results;
    }

    /// Retrieves all relevant data structures from a particular [`Repository`]'s [`PullRequest`] based
    /// on its `pr_number`.
    pub async fn retrieve_pr_data(&self, pr_number: u64) -> PullRequestDataResult {
        let start = Instant::now();

        let repo = self.repository();
        let owner = self.owner();

        info!("Analyzing repository [{}]'s PR#[{}]...", repo.name, pr_number);

        let github_connection = self.get_github_client().await;
        let pr = github_connection.pulls(owner, &repo.name).get(pr_number).await.map_err(|e| {
            error!("There was a problem during initial PR-retrieval task. Aborting operation.");
            AnalyzeError::PullRequestNotFound {
                repo_name: repo.name.to_string(),
                pr_number,
                nested: nested!(e),
            }
        })?;

        let result = self.retrieve_pr_data_from(&pr).await;

        let duration = start.elapsed();
        info!(
            "Time elapsed retrieving data for [{}]/[{}] was: {:?}",
            repo.name, pr_number, duration
        );

        return result;
    }

    /// Retrieves all relevant data structures from a particular [`Repository`]'s [`PullRequest`].
    async fn retrieve_pr_data_from(&self, pr: &PullRequest) -> PullRequestDataResult {
        let repo = self.repository();

        debug!("Retrieving data for [{}][{}]...", repo.name, pr.number);
        let start = Instant::now();

        // start with doing the analysis task(s) that don't require further remote API calls
        let main_message = Analyzer::get_pr_message(&pr);

        let merged_at = Analyzer::get_merged_date(&pr)?;
        let closed_at = Analyzer::get_closed_date(&pr)?;

        // once those are done, start preparing those task(s) that do require remote API calls
        // (they will be fired all in parallel to save time)
        let comments_fetch_task = tokio::spawn({
            trace!("Starting get_pr_comments() async task...");

            let repo_name = repo.name.clone();
            let pr_number = pr.number;
            let github_connection = self.get_github_client().await;
            let owner = self.owner.clone();

            async move {
                Analyzer::get_pr_comments(github_connection, owner, repo_name, pr_number)
                    .await
                    .unwrap()
            }
        });

        let commit_comments_fetch_task = tokio::spawn({
            trace!("Starting get_pr_commit_comments() async task...");

            let pr_review_comments_url = pr.review_comments_url.clone();
            let github_connection = self.get_github_client().await;

            async move {
                Analyzer::get_pr_commit_comments(github_connection, pr_review_comments_url)
                    .await
                    .unwrap()
            }
        });

        let reviews_fetch_task = tokio::spawn({
            trace!("Starting get_pr_reviews() async task...");

            let repo_name = repo.name.clone();
            let pr_number = pr.number;
            let github_connection = self.get_github_client().await;
            let owner = self.owner.clone();

            async move {
                Analyzer::get_pr_reviews(github_connection, owner, repo_name, pr_number)
                    .await
                    .unwrap()
            }
        });

        let diff_fetch_task = tokio::spawn({
            trace!("Starting get_pr_diff() async task...");

            let repo_name = repo.name.clone();
            let pr_number = pr.number;
            let github_connection = self.get_github_client().await;
            let owner = self.owner.clone();

            async move {
                Analyzer::get_pr_diff(github_connection, owner, repo_name, pr_number).await.unwrap()
            }
        });

        let commits_fetch_task = tokio::spawn({
            trace!("Starting get_pr_commits() async task...");

            let pr_commits_url = pr.commits_url.clone();
            let github_connection = self.get_github_client().await;

            async move { Analyzer::get_pr_commits(github_connection, pr_commits_url).await.unwrap() }
        });

        let concurrent_fetches = try_join!(
            comments_fetch_task,
            commit_comments_fetch_task,
            reviews_fetch_task,
            diff_fetch_task,
            commits_fetch_task
        );

        return match concurrent_fetches {
            Ok((
                comments_fetched,
                commit_comments_fetched,
                reviews_fetched,
                diff_fetched,
                commits_fetched,
            )) => {
                let duration = start.elapsed();
                debug!(
                    "Time elapsed retrieving inner data structures for [{}]/[{}] was: {:?}. Processing results...",
                    repo.name, pr.number, duration
                );

                trace!("PR body: {}", main_message);

                let comments = comments_fetched.items;
                trace!("Comments: {}", serde_json::to_string_pretty(&comments).unwrap());

                let reviews = reviews_fetched;
                trace!("Reviews: {}", serde_json::to_string_pretty(&reviews).unwrap());

                let commit_comments = commit_comments_fetched;
                trace!(
                    "Commit comments: {}",
                    serde_json::to_string_pretty(&commit_comments).unwrap()
                );

                let commits = commits_fetched;
                trace!("Commits: {}", serde_json::to_string_pretty(&commits).unwrap());

                let patch_set = diff_fetched;
                let modifications: u64 = patch_set
                    .files()
                    .iter()
                    .map(|file| {
                        u64::try_from(file.added()).unwrap()
                            + u64::try_from(file.removed()).unwrap()
                    })
                    .sum();
                trace!("Total modifications: {}", modifications);

                // having retrieved, parsed and traced all relevant elements, calculate time metrics and return result
                let result = PullRequestData::new(
                    &repo.name,
                    pr.number,
                    &pr.user.login,
                    &pr.title,
                    &main_message,
                    comments,
                    commit_comments,
                    commits,
                    reviews,
                    patch_set,
                    pr.created_at,
                    merged_at,
                    closed_at,
                );

                Ok(result)
            }
            Err(err) => {
                debug!("An unrecoverable error has occurred in one or more data-fetching steps for [{}]/[{}] and operation had to be aborted mid-process. Error = {:?}", repo.name, pr.number, err);
                Err(AnalyzeError::PullRequestDataRetrievalError {
                    repo_name: repo.name.to_string(),
                    pr_number: pr.number,
                    nested: nested!(err),
                })
            }
        };
    }

    /// The literal PR body; the first message, and arguably the comment that should have the most info of
    /// all (or at least a good summary of the changes).
    fn get_pr_message(pr: &PullRequest) -> String {
        let default = String::from("");
        pr.body
            .as_ref()
            .unwrap_or_else(|| {
                warn!("No PR body could be retrieved. Analysis for it will be empty.");
                &default
            })
            .clone()
    }

    /// The [`DateTime`] at which the [`PullRequest`] has been merged.
    fn get_merged_date(pr: &PullRequest) -> Result<DateTime<Utc>, AnalyzeError> {
        match pr.merged_at {
            Some(date) => Ok(date),
            None => Err(AnalyzeError::PullRequestIncompleteDataError {
                reason: "No merged date. Only properly merged PRs can be analyzed in full."
                    .to_string(),
                pr_number: pr.number,
            }),
        }
    }

    /// The [`DateTime`] at which the [`PullRequest`] has been closed.
    fn get_closed_date(pr: &PullRequest) -> Result<DateTime<Utc>, AnalyzeError> {
        match pr.closed_at {
            Some(date) => Ok(date),
            None => Err(AnalyzeError::PullRequestIncompleteDataError {
                reason: "No closed date. Only properly closed PRs can be analyzed in full."
                    .to_string(),
                pr_number: pr.number,
            }),
        }
    }

    /// 'comments' are the normal text snippets in a PR (they were submitted clicking on the 'Comment' button,
    /// instead of the 'Approve' or 'Request changes' buttons).
    #[prolice_trace_time]
    async fn get_pr_comments(
        github_connection: GitHubConnection, owner: String, repo_name: String, pr_number: u64,
    ) -> octocrab::Result<Page<Comment>> {
        trace!("Retrieving comments for [{}]/[{}]...", repo_name, pr_number);

        github_connection.issues(owner, repo_name).list_comments(pr_number).send().await
    }

    /// 'reviews' are those comments that were specially submitted as a review. Commit comments (comments
    /// on a portion of the unified diff) are also inside this category, but for some (weird) reason they
    /// are listed in a trimmed format as "event summaries" (for lack of a better description) in GitHub's
    /// response. Those are worthless that way because they don't have a body, so we must fetch them in
    /// some other way.
    #[prolice_trace_time]
    async fn get_pr_reviews(
        github_connection: GitHubConnection, owner: String, repo_name: String, pr_number: u64,
    ) -> Result<Vec<Review>, AnalyzeError> {
        trace!("Retrieving reviews for [{}]/[{}]...", repo_name, pr_number);

        /* === STORY TIME ===
         *
         * Ideally, instead of doing this whole fetch-and-parse process manually, we would be using the
         * function that the octocrab library already has available for fetching the reviews of a PR:
         *
         *      github_connection.pulls(owner, repo_name).list_reviews(pr_number).await
         *
         * Unfortunately, it has a tiny fatal flaw: it has 4 ReviewState's defined (Approved, Pending,
         * ChangesRequested & Commented) for GitHub's FIVE potential states (Approved, Pending, ChangesRequested,
         * Commented & DISMISSED).
         *
         * Since this is defined as an enumeration inside octocrab's Review struct, when the state is
         * 'DISMISSED' it causes the JSON parsing process to fail (because there is no defined value
         * for it). This not only causes an unrecoverable panic for the analyzing thread, but it also
         * completely ruins the PR for analysis.
         *
         * The rest of the library is pretty solid tbh, so until this annoying bug gets resolved, we
         * do this one manually; using our own struct (which was shamelessly copied from octocrab's
         * files, but with the fix).
         * */

        let url = format!(
            "{github_base_url}repos/{owner}/{repo}/pulls/{pr}/reviews",
            github_base_url = github_connection.base_url.as_str(),
            owner = owner,
            repo = repo_name,
            pr = pr_number
        );

        let builder = github_connection.request_builder(&url, reqwest::Method::GET);
        let response = github_connection
            .execute(builder)
            .await
            .map_err(|e| {
                trace!("Error = {:?}", e);
                AnalyzeError::GitHubAPIError {
                    msg: format!("Error fetching reviews for PR in [{}].", &url),
                    nested: nested!(e),
                }
            })
            .unwrap();

        if response.content_length().is_some() && response.content_length().unwrap() == 0 {
            warn!("No content received while fetching reviews for PR in [{}].", &url);
            return Ok(Vec::new());
        }

        let raw_response_text = response.text().await.map_err(|e| {
            trace!("Error = {:?}", e);
            AnalyzeError::GitHubAPIResponseBodyError {
                msg: format!("Error retrieving reviews' JSON for PR in [{}].", &url),
                nested: nested!(e),
            }
        })?;

        let parsed_json: Vec<Review> = serde_json::from_str(&raw_response_text).map_err(|e| {
            trace!("Error = {:?}", e);
            trace!("Raw response = {}", raw_response_text);
            AnalyzeError::JsonParseError {
                msg: format!("Error mapping reviews' JSON for PR in [{}].", url),
                nested: nested!(e),
            }
        })?;

        Ok(parsed_json)
    }

    /// 'commit comments' are comments on a portion of the unified diff.
    /// See more: https://stackoverflow.com/a/16200750
    #[prolice_trace_time]
    async fn get_pr_commit_comments(
        github_connection: GitHubConnection, pr_review_comments_url: Url,
    ) -> Result<Vec<CommitComment>, AnalyzeError> {
        trace!("Retrieving commit comments for PR in [{}]...", pr_review_comments_url);

        let url = pr_review_comments_url.as_str();
        let builder = github_connection.request_builder(url, reqwest::Method::GET);
        let response = github_connection
            .execute(builder)
            .await
            .map_err(|e| {
                trace!("Error = {:?}", e);
                AnalyzeError::GitHubAPIError {
                    msg: format!("Error fetching commit comments for PR in [{}].", url),
                    nested: nested!(e),
                }
            })
            .unwrap();

        if response.content_length().is_some() && response.content_length().unwrap() == 0 {
            warn!("No content received while fetching commit comments for PR in [{}].", url);
            return Ok(Vec::new());
        }

        let raw_response_text = response.text().await.map_err(|e| {
            trace!("Error = {:?}", e);
            AnalyzeError::GitHubAPIResponseBodyError {
                msg: format!("Error retrieving commit comments' JSON for PR in [{}].", url),
                nested: nested!(e),
            }
        })?;

        let parsed_json: Vec<CommitComment> =
            serde_json::from_str(&raw_response_text).map_err(|e| {
                trace!("Error = {:?}", e);
                trace!("Raw response = {}", raw_response_text);
                AnalyzeError::JsonParseError {
                    msg: format!("Error mapping commit comments' JSON for PR in [{}].", url),
                    nested: nested!(e),
                }
            })?;

        Ok(parsed_json)
    }

    /// 'commits' are snapshots of the codebase at a given time. The unified diff of all commits in a
    /// branch constitutes a [`PullRequest`]'s content.
    #[prolice_trace_time]
    async fn get_pr_commits(
        github_connection: GitHubConnection, pr_commits_url: Url,
    ) -> Result<Vec<CommitRoot>, AnalyzeError> {
        trace!("Retrieving commits for PR in [{}]...", pr_commits_url);

        let url = pr_commits_url.as_str();
        let builder = github_connection.request_builder(url, reqwest::Method::GET);
        let response = github_connection
            .execute(builder)
            .await
            .map_err(|e| {
                trace!("Error = {:?}", e);
                AnalyzeError::GitHubAPIError {
                    msg: format!("Error fetching commits for PR in [{}].", url),
                    nested: nested!(e),
                }
            })
            .unwrap();

        if response.content_length().is_some() && response.content_length().unwrap() == 0 {
            warn!("No content received while fetching commits for PR in [{}].", url);
            return Ok(Vec::new());
        }

        let raw_response_text = response.text().await.map_err(|e| {
            trace!("Error = {:?}", e);
            AnalyzeError::GitHubAPIResponseBodyError {
                msg: format!("Error retrieving commits' JSON for PR in [{}].", url),
                nested: nested!(e),
            }
        })?;

        let parsed_json: Vec<CommitRoot> =
            serde_json::from_str(&raw_response_text).map_err(|e| {
                trace!("Error = {:?}", e);
                trace!("Raw response = {}", raw_response_text);
                AnalyzeError::JsonParseError {
                    msg: format!("Error mapping commits' JSON for PR in [{}].", url),
                    nested: nested!(e),
                }
            })?;

        if parsed_json.is_empty() {
            return Err(AnalyzeError::NoCommitsFoundError);
        }

        Ok(parsed_json)
    }

    /// Returns a specific [`PullRequest`]'s diff.
    #[prolice_trace_time]
    async fn get_pr_diff(
        github_connection: GitHubConnection, owner: String, repo_name: String, pr_number: u64,
    ) -> Result<PatchSet, AnalyzeError> {
        trace!("Retrieving diff for [{}]/[{}]...", repo_name, pr_number);

        let diff =
            github_connection.pulls(owner, &repo_name).get_diff(pr_number).await.map_err(|e| {
                AnalyzeError::GitHubAPIError {
                    msg: format!(
                        "Could not retrieve diff for [{}/{}]. Aborting operation.",
                        repo_name, pr_number
                    ),
                    nested: nested!(e),
                }
            })?;

        let mut patch = PatchSet::new();
        patch.parse(diff).map_err(|e| AnalyzeError::DiffParseError {
            repo_name,
            pr_number,
            nested: nested!(e),
        })?;

        Ok(patch)
    }

    /// Instantiates a new [`Analyzer`] instance under the given `owner`, which can be either an individual
    /// or an organization.
    ///
    /// In the case of private organizations (or if you want to analyze private repositories belonging
    /// to an individual); the underlying `connection_pool` must have been instantiated with a
    /// [personal access token](https://docs.github.com/en/github/authenticating-to-github/creating-a-personal-access-token)
    /// that has read access for the intended targets.
    fn new(
        owner: &str, repository: Repository, github_personal_access_token: &str,
        connection_pool: &'static Pool<Octocrab, GitHubPoolError>,
    ) -> Self {
        Analyzer {
            owner: owner.to_string(),
            repository,
            github_personal_access_token: github_personal_access_token.to_string(),
            connection_pool,
        }
    }

    pub fn repository(&self) -> &Repository {
        &self.repository
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }
}
