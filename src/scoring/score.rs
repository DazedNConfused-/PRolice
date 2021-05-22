use std::fmt::{Display, Formatter};

use log::error;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};

/// Enumeration of important qualities from either a [`PullRequest`](octocrab::models::pulls::PullRequest)
/// or a [`Repository`](octocrab::models::Repository) that are worth analyzing and measuring.
#[derive(Display, Serialize, Deserialize, EnumIter, Debug, PartialEq)]
pub enum ScoreType {
    AmountOfParticipants(u64),
    AmountOfReviewers(u64),
    Attachments(u64),
    AuthorCommentaryToChangesRatio(f64),
    PullRequestsDiscussionSize(usize),
    PullRequestFlowRatio(f64),
    PullRequestLeadTime(u64),
    PullRequestSize(usize),
    TestToCodeRatio {
        loc: usize,
        test_loc: usize,
        ratio: f64,
    },
    TimeToMerge(u64),
}

impl ScoreType {
    /// Returns an iterator over all the possible elements of [`ScoreType`].
    pub fn get_iter() -> ScoreTypeIter {
        ScoreType::iter()
    }

    /// Returns a verbose explanation of what a particular [`ScoreType`] represents.
    // Some of these come from personal experience, others from this excellent article on PR metrics:
    // https://sourcelevel.io/blog/5-metrics-engineering-managers-can-extract-from-pull-requests
    pub fn get_legend(&self) -> &'static str {
        match &self {
            ScoreType::AmountOfParticipants(_) =>
                "The amount of non-authoring people participating in a PR's discussion. Bigger participation \
                may enrich discussion and produce higher quality code.",
            ScoreType::AmountOfReviewers(_) =>
                "The amount of non-authoring people that have taken a stand on a PR's outcome, either by \
                 approving or requesting for changes. This measures the amount of participants that effectively \
                 decide on a PR's fate.",
            ScoreType::Attachments(_) =>
                "Attachments can be anything ranging from added screenshots to embedded PDF files. Particularly \
                useful for those PRs that have a visual component associated to it.",
            ScoreType::AuthorCommentaryToChangesRatio(_) =>
                "Good code should be self-explanatory; but a good PR may also include extra commentary \
                on what it aims to achieve, how it does it and/or why it does it the chosen way. \n\n\

                A slim commentary may make for an ambiguous PR, shifting the burden of understanding \
                onto the reviewer and consuming extra time from it. On the other hand, too many comments \
                may pollute a PR with unneeded noise, to the same effect.",
            ScoreType::PullRequestsDiscussionSize(_) =>
                "Similar to Author Commentary to Changes Ratio, it measures the total amount of comments \
                in a PR, but irrespective of who they come from. On the contrary to social media posts, \
                too much engagement in pull requests leads to inefficiency. Measuring the number of comments \
                and reactions for each pull request gives an idea of how the team collaborates. Collaboration \
                is great, and its endorsement is something to be desired. However, after a certain level, \
                discussions slow down development. \n\n\

                Discussions that get too big may be indicative of something wrong: maybe the team is not \
                aligned, or maybe the software requirements are not precise enough. In any case, misalignment \
                in discussions are not collaboration; they are a waste of time. In the opposite scenario, \
                having almost zero engagement means code review is not part of the team's habits. \n\n\

                In summary, this metric must reach an 'ideal number' based on the team's size and distribution. \
                It can't be too much, and it can't be too little either.",
            ScoreType::PullRequestFlowRatio(_) =>
                "The Pull Request Flow Ratio is the sum of the opened pull requests in a day divided by \
                the sum of closed pull requests in that same day. This metric shows whether the team \
                works in a healthy proportion. Merging pull requests and deploying to production is a \
                good thing, for it adds value to the final user. However, when the team closes more pull \
                requests than opens, soon the pull request queue starves, which means there may be a \
                hiatus in the delivery. Ideally, it is best to make sure the team merges pull requests \
                in a ratio as close as they open; the closer to 1:1, the better.",
            ScoreType::PullRequestLeadTime(_) =>
                "The lead-time metric gives an idea of how many times (usually in days) pull requests \
                take to be merged or closed. To find this number, the date and time for each pull request \
                when opened and then merged is needed. The formula is easy: a simple average for the \
                difference of dates. Calculating this metric across all repositories in an organization \
                can give a team a clearer idea of their dynamics.",
            ScoreType::PullRequestSize(_) =>
                "A large amount of changes per PR imposes a strain on the reviewer, who sees its attention \
                to detail diminished the bigger a changelog gets. Ironically, developers tend to merge \
                longer pull requests faster than shorter ones, for it is more difficult to perform thorough \
                reviews when there are too many things going on. Regardless of how thorough the reviews \
                are, big PRs lead to the Time To Merge going up, and the quality going down.",
            ScoreType::TestToCodeRatio{
                loc: _loc,
                test_loc: _test_loc,
                ratio: _ratio,
            }  =>
                "As a rule of thumb, at least half of a PR should be comprised of tests whenever possible.",
            ScoreType::TimeToMerge(_) =>
                "In general, pull requests are open with some work in progress, which means that measuring \
                Pull Request Lead Time does not tell the whole story. Time to Merge is how much time \
                it takes for the first commit of a branch to reach the target branch. In practice, the \
                math is simple: it is the timestamp of the oldest commit of a branch minus the timestamp \
                of the merge commit. \n\n\
                
                The Time to Merge is usually useful while compared against the Pull Request Lead Time.\
                Take the following example:\n\n\

                * Pull Request Lead Time = 3 days \n\
                * Time To Merge = 15 days \n\n\
                
                In the above scenario, a pull request took an average time of 3 days to be merged (which \
                is pretty good); but the Time to Merge was 15 days. Which means that the developers worked \
                an average of 12 days (15 – 3) before opening a pull request. \n\n\
    
                NOTE: \n\
                This metric is rendered somewhat obsolete if developers work on WIP branches before squashing \
                all the changes into a single commit that is later used as base for the PR (this would \
                make the Time To Merge effectively equal to the Pull Request Lead Time). However, the metric \
                still remains incredibly useful for merge PRs (for example, merge develop into master): \
                said PRs would have a very short Pull Request Lead Time (they don't get thorough re-reviews), \
                but measuring against the first commit's date (Time to Merge) will tell how long it takes \
                for features to get accumulated into a milestone worthy enough of merging into one of \
                the 'big' branches.",
        }
    }

    /// Returns a verbose explanation of all possible [`ScoreType`]s.
    pub fn get_legends() -> String {
        let mut result = String::new();

        for score_type in ScoreType::get_iter() {
            let score_type_name: String = score_type.to_string();

            result.push('\n');
            result.push_str(&"-".repeat(score_type_name.len()));
            result.push('\n');
            result.push_str(&score_type_name);
            result.push('\n');
            result.push_str(&"-".repeat(score_type_name.len()));
            result.push_str("\n\n");
            result.push_str(score_type.get_legend());
            result.push('\n');
        }

        result
    }
}

/// A collection of [`ScoreType`]s, the "end-product" of an analysis.
#[derive(Debug, Serialize, Deserialize)]
pub struct Score {
    pr_number: Option<u64>,
    score: Vec<ScoreType>,
}

impl Score {
    pub fn new(pr_number: Option<u64>, score: Vec<ScoreType>) -> Self {
        Score {
            pr_number,
            score,
        }
    }

    pub fn score(self) -> Vec<ScoreType> {
        self.score
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self).unwrap_or_else(|e| {
            error!("Could not construct JSON for Score [{:#?}].", &self);
            panic!(e);
        })
    }
}

impl Display for Score {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", &self.to_json())
    }
}
