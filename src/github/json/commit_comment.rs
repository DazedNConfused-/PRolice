use serde::Deserialize;
use serde::Serialize;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Custom wrapper for a GitHub's commit's comment (that is, a comment on a portion of the unified diff
/// from a particular [`PullRequest`]).
pub struct CommitComment {
    pub url: String,
    #[serde(rename = "pull_request_review_id")]
    pub pull_request_review_id: i64,
    pub id: i64,
    #[serde(rename = "node_id")]
    pub node_id: String,
    #[serde(rename = "diff_hunk")]
    pub diff_hunk: String,
    pub path: String,
    pub position: ::serde_json::Value,
    #[serde(rename = "original_position")]
    pub original_position: i64,
    #[serde(rename = "commit_id")]
    pub commit_id: String,
    #[serde(rename = "original_commit_id")]
    pub original_commit_id: String,
    pub user: User,
    pub body: String,
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "updated_at")]
    pub updated_at: String,
    #[serde(rename = "html_url")]
    pub html_url: String,
    #[serde(rename = "pull_request_url")]
    pub pull_request_url: String,
    #[serde(rename = "author_association")]
    pub author_association: String,
    #[serde(rename = "_links")]
    pub links: Links,
    #[serde(rename = "start_line")]
    pub start_line: ::serde_json::Value,
    #[serde(rename = "original_start_line")]
    pub original_start_line: ::serde_json::Value,
    #[serde(rename = "start_side")]
    pub start_side: ::serde_json::Value,
    pub line: ::serde_json::Value,
    #[serde(rename = "original_line")]
    pub original_line: i64,
    pub side: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub login: String,
    pub id: i64,
    #[serde(rename = "node_id")]
    pub node_id: String,
    #[serde(rename = "avatar_url")]
    pub avatar_url: String,
    #[serde(rename = "gravatar_id")]
    pub gravatar_id: String,
    pub url: String,
    #[serde(rename = "html_url")]
    pub html_url: String,
    #[serde(rename = "followers_url")]
    pub followers_url: String,
    #[serde(rename = "following_url")]
    pub following_url: String,
    #[serde(rename = "gists_url")]
    pub gists_url: String,
    #[serde(rename = "starred_url")]
    pub starred_url: String,
    #[serde(rename = "subscriptions_url")]
    pub subscriptions_url: String,
    #[serde(rename = "organizations_url")]
    pub organizations_url: String,
    #[serde(rename = "repos_url")]
    pub repos_url: String,
    #[serde(rename = "events_url")]
    pub events_url: String,
    #[serde(rename = "received_events_url")]
    pub received_events_url: String,
    #[serde(rename = "type")]
    pub type_field: String,
    #[serde(rename = "site_admin")]
    pub site_admin: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Links {
    #[serde(rename = "self")]
    pub self_field: SelfField,
    pub html: Html,
    #[serde(rename = "pull_request")]
    pub pull_request: PullRequest,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfField {
    pub href: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Html {
    pub href: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub href: String,
}
