//! Container for all relevant information for a particular [`PullRequest`](octocrab::models::pulls::PullRequest).

use chrono::{DateTime, Utc};
use itertools::Itertools;
use lazy_static::lazy_static;
use log::{debug, error, trace};
use octocrab::models::issues::Comment;
use regex::Regex;
use unidiff::Hunk;
use unidiff::PatchSet;

use crate::github::json::commit::CommitRoot;
use crate::github::json::commit_comment::CommitComment;
use crate::github::json::review::Review;
use crate::prolice_error::AnalyzeError;
use crate::scoring::scorable::Scorable;
use crate::scoring::score::{Score, ScoreType};

/// A wrapper for an already-analyzed [`PullRequest`](octocrab::models::pulls::PullRequest). It contains
/// all proper structures in order to retrieve useful metrics.
pub struct PullRequestData {
    repo_name: String,
    pr_number: u64,
    pr_author: String,
    pr_title: String,
    main_message: String,
    comments: Vec<Comment>,
    commit_comments: Vec<CommitComment>,
    commits: Vec<CommitRoot>,
    reviews: Vec<Review>,
    patch_set: PatchSet,
    created_at: DateTime<Utc>,
    merged_at: DateTime<Utc>,
    closed_at: DateTime<Utc>,
}

impl PullRequestData {
    pub fn new(
        repo_name: &str, pr_number: u64, pr_author: &str, pr_title: &str, main_message: &str,
        comments: Vec<Comment>, commit_comments: Vec<CommitComment>, commits: Vec<CommitRoot>,
        reviews: Vec<Review>, patch_set: PatchSet, created_at: DateTime<Utc>,
        merged_at: DateTime<Utc>, closed_at: DateTime<Utc>,
    ) -> Self {
        PullRequestData {
            repo_name: repo_name.to_string(),
            pr_number,
            pr_author: pr_author.to_string(),
            pr_title: pr_title.to_string(),
            main_message: main_message.to_string(),
            comments,
            commit_comments,
            commits,
            reviews,
            patch_set,
            created_at,
            merged_at,
            closed_at,
        }
    }

    pub fn repo_name(&self) -> &str {
        &self.repo_name
    }
    pub fn pr_number(&self) -> u64 {
        self.pr_number
    }
    pub fn pr_author(&self) -> &str {
        &self.pr_author
    }
    pub fn pr_title(&self) -> &str {
        &self.pr_title
    }
    pub fn main_message(&self) -> &str {
        &self.main_message
    }
    pub fn comments(&self) -> &Vec<Comment> {
        &self.comments
    }
    pub fn commit_comments(&self) -> &Vec<CommitComment> {
        &self.commit_comments
    }
    pub fn commits(&self) -> &Vec<CommitRoot> {
        &self.commits
    }
    pub fn reviews(&self) -> &Vec<Review> {
        &self.reviews
    }
    pub fn patch_set(&self) -> &PatchSet {
        &self.patch_set
    }
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub fn closed_at(&self) -> DateTime<Utc> {
        self.closed_at
    }
}

impl PullRequestData {
    /// Returns the amount of net added lines corresponding to test files.
    ///
    /// If result would be negative (because there were more deletions than additions), returned amount
    /// is effectively zero.
    pub fn get_amount_of_net_added_test_lines(&self) -> usize {
        self.patch_set
            .added_files()
            .iter()
            .chain(self.patch_set.modified_files().iter())
            .filter(|patched_file| PullRequestData::is_test_file(&patched_file.target_file))
            .flat_map(|patched_file| {
                trace!(
                    "[test-lines] Analyzing {} -> {} ...",
                    patched_file.source_file,
                    patched_file.target_file
                );
                patched_file.hunks().iter()
            })
            .map(|hunk| PullRequestData::count_net_added_lines_for_hunk(hunk))
            .sum()
    }

    /// Returns the amount of net added lines *not* corresponding to test files (aka everything else).
    ///
    /// If result would be negative (because there were more deletions than additions), returned amount
    /// is effectively zero.
    pub fn get_amount_of_net_added_non_test_lines(&self) -> usize {
        self.patch_set
            .added_files()
            .iter()
            .chain(self.patch_set.modified_files().iter())
            .filter(|patched_file| !PullRequestData::is_test_file(&patched_file.target_file))
            .flat_map(|patched_file| {
                trace!(
                    "[non-test-lines] Analyzing {} -> {} ...",
                    patched_file.source_file,
                    patched_file.target_file
                );
                patched_file.hunks().iter()
            })
            .map(|hunk| PullRequestData::count_net_added_lines_for_hunk(hunk))
            .sum()
    }

    /// Returns the amount of modified lines, irrespective of whether they were additions or deletions.
    pub fn get_amount_of_changes(&self) -> usize {
        self.patch_set
            .files()
            .iter()
            .flat_map(|patched_file| {
                trace!(
                    "[changes] Analyzing {} -> {} ...",
                    patched_file.source_file,
                    patched_file.target_file
                );
                patched_file.hunks().iter()
            })
            .map(|hunk| hunk.added() + hunk.removed())
            .sum()
    }

    /// Returns all comments posted by the PR's author.
    /// <br/><br/>
    /// **Note:** The author may have posted a comment either with the aim to enrich the PR, or as an
    /// answer to a particular reviewer's inquiry. This method does not discriminate between one or the
    /// other; since no matter the intent behind it, it is still part of a PR's discussion, and it counts
    /// towards enriching (or polluting with noise) its overall quality.
    pub fn get_author_commentary(&self) -> Vec<&String> {
        trace!("PR body: {}", self.main_message);
        let author_main_pr_message: Vec<&String> = vec![&self.main_message];

        let author_comments: Vec<&String> = self
            .comments
            .iter()
            .filter(|comment| comment.user.login == self.pr_author)
            .filter_map(|comment| comment.body.as_ref())
            .collect();
        trace!("Comments from [{}]: {:?}", self.pr_author, author_comments);

        let author_commit_comments: Vec<&String> = self
            .commit_comments
            .iter()
            .filter(|commit_comment| commit_comment.user.login == self.pr_author)
            .map(|commit_comment| &commit_comment.body)
            .collect();
        trace!("Commit-comments from [{}]: {:?}", self.pr_author, author_commit_comments);

        let author_reviews: Vec<&String> = self
            .reviews
            .iter()
            .filter(|review| review.user.login == self.pr_author)
            .filter_map(|review| review.body.as_ref())
            .collect();
        trace!("Reviews from [{}]: {:?}", self.pr_author, author_reviews);

        // having filtered and traced everything, chain and return results
        author_main_pr_message
            .into_iter()
            .chain(author_comments.into_iter())
            .chain(author_commit_comments.into_iter())
            .chain(author_reviews.into_iter())
            .collect()
    }

    /// Returns all comments irrespective of their author(s).
    pub fn get_all_commentary(&self) -> Vec<&String> {
        trace!("PR body: {}", self.main_message);
        let main_pr_message: Vec<&String> = vec![&self.main_message];

        let comments: Vec<&String> =
            self.comments.iter().filter_map(|comment| comment.body.as_ref()).collect();
        trace!("Comments: {:?}", comments);

        let commit_comments: Vec<&String> =
            self.commit_comments.iter().map(|commit_comment| &commit_comment.body).collect();
        trace!("Commit-comments: {:?}", commit_comments);

        let reviews: Vec<&String> =
            self.reviews.iter().filter_map(|review| review.body.as_ref()).collect();
        trace!("Reviews: {:?}", reviews);

        // having processed and traced everything, chain and return results
        main_pr_message
            .into_iter()
            .chain(comments.into_iter())
            .chain(commit_comments.into_iter())
            .chain(reviews.into_iter())
            .collect()
    }

    /// Returns the amount of characters for all comments posted by the PR's author.
    pub fn get_amount_of_author_commentary(&self) -> usize {
        self.get_author_commentary().iter().map(|s| s.len()).sum()
    }

    /// Returns the amount of characters for all comments, irrespective of their author(s).
    pub fn get_amount_of_commentary(&self) -> usize {
        self.get_all_commentary().iter().map(|s| s.len()).sum()
    }

    /// Determines whether this [`PullRequestData`] corresponds to a merge PR or not.
    /// Merge PRs are those that are basically used to update branches between environments (ie: merging
    /// the 'develop' branch into the 'master' branch).
    /// <br/><br/>
    /// **Note:** This implementation is quite 'naive' and depends on proper naming conventions (aka
    /// the PR's title must start with 'Merge'... - ie: "Merge develop into QA").
    pub fn is_merge_pr(&self) -> bool {
        self.pr_title.to_ascii_lowercase().starts_with("merge")
    }

    /// Returns all the non-author participants of the [`PullRequest`](octocrab::models::pulls::PullRequest).
    pub fn get_non_authoring_participants(&self) -> Vec<&String> {
        self.comments
            .iter()
            .map(|comment| &comment.user.login)
            .chain(self.reviews.iter().map(|review| &review.user.login))
            .chain(self.commit_comments.iter().map(|commit_comments| &commit_comments.user.login))
            .unique()
            .filter(|user| user != &&self.pr_author)
            .collect()
    }

    /// Returns all the non-author reviewers of the [`PullRequest`](octocrab::models::pulls::PullRequest).
    /// <br/><br/>
    /// This can be considered a smaller subset of the [`PullRequestData::get_non_authoring_participants()`]
    /// universe.
    pub fn get_non_authoring_reviewers(&self) -> Vec<&String> {
        self.reviews
            .iter()
            .map(|comment| &comment.user.login)
            .unique()
            .filter(|user| user != &&self.pr_author)
            .collect()
    }

    /// Returns all attachments posted by the PR's author.
    pub fn get_attachments_markdown(&self) -> Vec<String> {
        lazy_static! {
            static ref ATTACHMENT_REGEX: Regex = Regex::new("!?\\[.*\\]\\(.*?\\)").unwrap();
            // the regex will be compiled when it is used for the first time. On subsequent uses, it
            // will reuse the previous compilation.
            // https://docs.rs/regex/1.4.5/regex/#example-avoid-compiling-the-same-regex-in-a-loop
        }

        self.get_author_commentary()
            .into_iter()
            .flat_map(|s| ATTACHMENT_REGEX.find_iter(s))
            .map(|m| String::from(m.as_str()))
            .collect()
    }

    /// Returns the [`PullRequest`](octocrab::models::pulls::PullRequest)'s first commit's [`DateTime`].
    pub fn get_first_commit_date(&self) -> DateTime<Utc> {
        self.commits
            .get(0)
            .unwrap_or_else(|| {
                error!(
                    "Could not retrieve first commit for PR [{}]/[{}]. Aborting operation.",
                    self.repo_name, self.pr_number
                );
                panic!() // this is a fatal error that involves delving into the codebase (because a PR should be guaranteed at least a single commit).
            })
            .commit
            .author
            .date
    }

    /// Determines if a [`PatchedFile`](unidiff::PatchedFile)'s affected file corresponds to a test suite
    /// or not.
    /// <br/><br/>
    /// **Note:** This implementation is quite 'naive' and depends on proper naming conventions (aka
    /// the file must have the 'test' keyword somewhere in its name).
    ///
    /// **May trigger false positives if
    /// the file contains the word within another unrelated word - ie: 'contest'**.
    fn is_test_file(name: &str) -> bool {
        name.to_ascii_lowercase().contains("test")
    }

    /// Returns the count for the *net* amount of added lines in a [`Hunk`].
    /// If result would be negative, returned amount is zero.
    fn count_net_added_lines_for_hunk(hunk: &Hunk) -> usize {
        let net_added = hunk.added() as isize - hunk.removed() as isize;
        trace!("Added: {}; removed: {}; net-added: {}", hunk.added(), hunk.removed(), net_added);

        if net_added > 0 {
            net_added as usize
        } else {
            0
        }
    }
}

impl Scorable for PullRequestData {
    fn get_score(&self) -> Score {
        let all_comments = self.get_amount_of_commentary();
        let author_comments = self.get_amount_of_author_commentary();
        let changes_added = self.get_amount_of_changes();
        let commentary_to_changes_ratio =
            f64::trunc((author_comments as f64 / changes_added as f64) * 100.0) / 100.0; // 2 decimals

        debug!(
            "overall changes: {}, all comments: {}, author comments: {}; commentary-to-changes-ratio: {}",
            changes_added, all_comments, author_comments, commentary_to_changes_ratio
        );

        let net_test_lines_added = self.get_amount_of_net_added_test_lines();
        let net_non_test_lines_added = self.get_amount_of_net_added_non_test_lines();
        let test_to_code_ratio: f64 = if net_non_test_lines_added == 0 {
            0.0 // if net non-test lines is zero, avoid divide-by-zero (doesn't crash, but produces NaN) and return hard 0
        } else {
            (f64::trunc((net_test_lines_added as f64 / net_non_test_lines_added as f64) * 100.0)
                / 100.0)
                .abs() // 2 decimals
        };

        debug!(
            "net-test-lines added: {}; net-non-test-lines added: {}; test-to-code-ratio: {}",
            net_test_lines_added, net_non_test_lines_added, test_to_code_ratio
        );

        let non_authoring_participants = self.get_non_authoring_participants();
        debug!("non-authoring participants: {:?}", non_authoring_participants);

        let non_authoring_reviewers = self.get_non_authoring_reviewers();
        debug!("non-authoring reviewers: {:?}", non_authoring_reviewers);

        let attachments = self.get_attachments_markdown();
        debug!("author attachments: {:?}", attachments);

        let pull_request_lead_time = (self.closed_at - self.created_at).num_days() as u64;
        debug!(
            "created at: {}, closed at: {}, pull request lead time: {}",
            self.created_at, self.closed_at, pull_request_lead_time
        );

        let first_commit_at = self.get_first_commit_date();
        let time_to_merge = (self.merged_at - first_commit_at).num_days() as u64;
        debug!(
            "first commit at: {}, merged at: {}, time to merge: {}",
            first_commit_at, self.merged_at, time_to_merge
        );

        // having processed a PR's attributes, prepare individual scoring of important attributes
        let mut scorables: Vec<ScoreType> = Vec::new();

        for score_type in ScoreType::get_iter() {
            match score_type {
                // having this iterator & match structure will guarantee that all possible ScoreType(s)
                // are present and accounted for at compilation time; which means a developer doesn't
                // have to worry about forgetting to include potential new ScoreType(s) into the scoring
                // process
                ScoreType::AmountOfParticipants(_) => scorables
                    .push(ScoreType::AmountOfParticipants(non_authoring_participants.len() as u64)),
                ScoreType::AmountOfReviewers(_) => scorables
                    .push(ScoreType::AmountOfReviewers(non_authoring_reviewers.len() as u64)),
                ScoreType::Attachments(_) => {
                    scorables.push(ScoreType::Attachments(attachments.len() as u64))
                }
                ScoreType::AuthorCommentaryToChangesRatio(_) => scorables
                    .push(ScoreType::AuthorCommentaryToChangesRatio(commentary_to_changes_ratio)),
                ScoreType::PullRequestsDiscussionSize(_) => {
                    scorables.push(ScoreType::PullRequestsDiscussionSize(all_comments))
                }
                ScoreType::PullRequestFlowRatio(_) => {
                    trace!(
                        "PullRequestFlowRatio metric not applicable to individual Pull Request(s); only to Repository(ies)."
                    )
                }
                ScoreType::PullRequestLeadTime(_) => {
                    scorables.push(ScoreType::PullRequestLeadTime(pull_request_lead_time))
                }
                ScoreType::PullRequestSize(_) => {
                    scorables.push(ScoreType::PullRequestSize(changes_added))
                }
                ScoreType::TestToCodeRatio(_) => {
                    scorables.push(ScoreType::TestToCodeRatio(test_to_code_ratio))
                }
                ScoreType::TimeToMerge(_) => scorables.push(ScoreType::TimeToMerge(time_to_merge)),
            }
        }

        Score::new(scorables)
    }
}

pub type PullRequestDataResult = Result<PullRequestData, AnalyzeError>;
