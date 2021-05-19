use std::collections::HashMap;

use chrono::{Date, Utc};
use log::trace;
use num::integer;

use crate::github::utils::pull_request_data::{PullRequestData, PullRequestDataResult};
use crate::scoring::scorable::Scorable;
use crate::scoring::score::{Score, ScoreType};

pub type RepositoryData = Vec<PullRequestDataResult>;

impl Scorable for RepositoryData {
    fn get_score(&self) -> Score {
        // iterate over all individual PRs -
        let individual_prs_data: Vec<&PullRequestData> = self
            .into_iter()
            .filter(|prdr| prdr.is_ok())
            .map(|prdr| prdr.as_ref().unwrap())
            .collect();

        individual_prs_data.get_score()
    }
}

impl Scorable for Vec<&PullRequestData> {
    fn get_score(&self) -> Score {
        let total_amount_of_prs = self.iter().len() as u64;

        // calculate their individual scores -
        let scores: Vec<ScoreType> =
            self.iter().map(|prd| prd.get_score()).flat_map(|score| score.score()).collect();

        // subdivide their individual scores by type -
        let mut total_amount_of_participants: u64 = 0;
        let mut total_amount_of_reviewers: u64 = 0;
        let mut total_attachments: u64 = 0;
        let mut total_author_commentary_to_changes_ratio: f64 = 0.0;
        let mut total_pull_requests_discussion_size: usize = 0;
        let mut total_pull_request_lead_time: u64 = 0;
        let mut total_pull_request_size: usize = 0;
        let mut total_test_lines_added: usize = 0;
        let mut total_non_test_lines_added: usize = 0;
        let mut total_test_to_code_ratio: f64 = 0.0;
        let mut total_time_to_merge: u64 = 0;

        for score_type in scores.iter() {
            match score_type {
                ScoreType::AmountOfParticipants(aop) => {
                    total_amount_of_participants += aop;
                    trace!(
                        "Adding {} participants to count. Total count so far = {}",
                        aop,
                        total_amount_of_participants
                    )
                }
                ScoreType::AmountOfReviewers(aor) => {
                    total_amount_of_reviewers += aor;
                    trace!(
                        "Adding {} reviewers to count. Total count so far = {}",
                        aor,
                        total_amount_of_reviewers
                    )
                }
                ScoreType::Attachments(a) => {
                    total_attachments += a;
                    trace!(
                        "Adding {} attachments to count. Total count so far = {}",
                        a,
                        total_attachments
                    )
                }
                ScoreType::AuthorCommentaryToChangesRatio(actcr) => {
                    total_author_commentary_to_changes_ratio += actcr;
                    trace!(
                        "Adding {} author-comments-to-changes-ratio to count. Total count so far = {}",
                        actcr,
                        total_author_commentary_to_changes_ratio
                    )
                }
                ScoreType::PullRequestsDiscussionSize(prds) => {
                    total_pull_requests_discussion_size += prds;
                    trace!(
                        "Adding {} lines of discussion to count. Total count so far = {}",
                        prds,
                        total_pull_requests_discussion_size
                    )
                }
                ScoreType::PullRequestFlowRatio(_) => {
                    // PullRequestFlowRatio will be calculated below; there is nothing to sum here because it doesn't apply to individual PRs
                }
                ScoreType::PullRequestLeadTime(prlt) => {
                    total_pull_request_lead_time += prlt;
                    trace!(
                        "Adding {} days of lead-time to count. Total count so far = {}",
                        prlt,
                        total_pull_request_lead_time
                    )
                }
                ScoreType::PullRequestSize(prs) => {
                    total_pull_request_size += prs;
                    trace!(
                        "Adding {} lines of code to count. Total count so far = {}",
                        prs,
                        total_pull_request_size
                    )
                }
                ScoreType::TestToCodeRatio {
                    loc,
                    test_loc,
                    ratio,
                } => {
                    total_non_test_lines_added += loc;
                    total_test_lines_added += test_loc;
                    total_test_to_code_ratio += ratio;
                    trace!(
                        "Adding {}/{}/{} loc/test loc/test-to-code-ratio to count. Total count so far = {}/{}/{}",
                        loc, test_loc, ratio,
                        total_non_test_lines_added, total_test_lines_added, total_test_to_code_ratio
                    )
                }
                ScoreType::TimeToMerge(ttm) => {
                    total_time_to_merge += ttm;
                    trace!(
                        "Adding {} days of time-to-merge to count. Total count so far = {}",
                        ttm,
                        total_time_to_merge
                    )
                }
            }
        }

        // derive repository's global score by calculating the average of each type across all PRs -
        let mut scorables: Vec<ScoreType> = Vec::new();

        for score_type in ScoreType::get_iter() {
            match score_type {
                // having this iterator & match structure will guarantee that all possible ScoreType(s)
                // are present and accounted for at compilation time; which means a developer doesn't
                // have to worry about forgetting to include potential new ScoreType(s) into the scoring
                // process
                ScoreType::AmountOfParticipants(_) => {
                    scorables.push(ScoreType::AmountOfParticipants(integer::div_ceil(
                        total_amount_of_participants,
                        total_amount_of_prs,
                    )))
                }
                ScoreType::AmountOfReviewers(_) => scorables.push(ScoreType::AmountOfReviewers(
                    integer::div_ceil(total_amount_of_reviewers, total_amount_of_prs),
                )),
                ScoreType::Attachments(_) => scorables.push(ScoreType::Attachments(
                    integer::div_ceil(total_attachments, total_amount_of_prs),
                )),
                ScoreType::AuthorCommentaryToChangesRatio(_) => {
                    scorables.push(ScoreType::AuthorCommentaryToChangesRatio(
                        total_author_commentary_to_changes_ratio / (total_amount_of_prs as f64),
                    ))
                }
                ScoreType::PullRequestsDiscussionSize(_) => {
                    scorables.push(ScoreType::PullRequestsDiscussionSize(integer::div_ceil(
                        total_pull_requests_discussion_size,
                        total_amount_of_prs as usize,
                    )))
                }
                ScoreType::PullRequestFlowRatio(_) => scorables.push(
                    ScoreType::PullRequestFlowRatio(calculate_pull_request_flow_ratio(&self)),
                ),
                ScoreType::PullRequestLeadTime(_) => {
                    scorables.push(ScoreType::PullRequestLeadTime(integer::div_ceil(
                        total_pull_request_lead_time,
                        total_amount_of_prs,
                    )))
                }
                ScoreType::PullRequestSize(_) => scorables.push(ScoreType::PullRequestSize(
                    integer::div_ceil(total_pull_request_size, total_amount_of_prs as usize),
                )),
                ScoreType::TestToCodeRatio {
                    loc: _loc,
                    test_loc: _test_loc,
                    ratio: _ratio,
                } => scorables.push(ScoreType::TestToCodeRatio {
                    loc: total_test_lines_added / (total_amount_of_prs as usize),
                    test_loc: total_non_test_lines_added / (total_amount_of_prs as usize),
                    ratio: total_test_to_code_ratio / (total_amount_of_prs as f64),
                }),
                ScoreType::TimeToMerge(_) => scorables.push(ScoreType::TimeToMerge(
                    integer::div_ceil(total_time_to_merge, total_amount_of_prs),
                )),
            }
        }

        Score::new(scorables)
    }
}

/// Calculates the PullRequestFlowRatio over the provided array of [`PullRequestData`]s.
fn calculate_pull_request_flow_ratio(prs: &Vec<&PullRequestData>) -> f64 {
    // generate map with all PRs that were created in the same day -
    let created_at_map: HashMap<Date<Utc>, u64> =
        prs.iter().fold(HashMap::new(), |mut acc, prd| {
            *acc.entry(prd.created_at().date()).or_insert(0) += 1;
            acc
        });
    trace!("pull-request-flow-ratio's created_at_map: {:?}", created_at_map);

    // generate map with all PRs that were closed in the same day -
    let closed_at_map: HashMap<Date<Utc>, u64> = prs.iter().fold(HashMap::new(), |mut acc, prd| {
        *acc.entry(prd.closed_at().date()).or_insert(0) += 1;
        acc
    });
    trace!("pull-request-flow-ratio's closed_at_map: {:?}", closed_at_map);

    // generate map calculating the PullRequestFlowRatio of those entries that match between the two previous maps -
    let pull_request_flow_ratio_map: HashMap<&Date<Utc>, f64> =
        created_at_map.iter().fold(HashMap::new(), |mut acc, created_at_entry| {
            let closed_at_entry = closed_at_map.get(created_at_entry.0);
            if let Some(amount_of_closures_in_day) = closed_at_entry {
                acc.insert(
                    created_at_entry.0,
                    (*created_at_entry.1 as f64) / (*amount_of_closures_in_day as f64),
                );
            }
            acc
        });
    trace!("pull-request-flow-ratio's result map: {:?}", pull_request_flow_ratio_map);

    // return average result -
    pull_request_flow_ratio_map.iter().map(|entry| entry.1).sum::<f64>()
        / (pull_request_flow_ratio_map.len() as f64)
}
