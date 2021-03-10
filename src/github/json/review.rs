use octocrab::models::pulls::Links;
use octocrab::models::User;
use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
/// Custom wrapper for a GitHub's review.
pub struct Review {
    pub id: u64,
    pub node_id: String,
    pub html_url: Url,
    pub user: User,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<ReviewState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_request_url: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "_links")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Links>,
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize)]
#[serde(rename_all(serialize = "SCREAMING_SNAKE_CASE"))]
#[non_exhaustive]
pub enum ReviewState {
    Approved,
    Pending,
    ChangesRequested,
    Commented,
    Dismissed,
}

// This is rather annoying, but Github uses both SCREAMING_SNAKE_CASE and snake_case
// for the review state, it's uppercase when coming from an API request, but
// lowercase when coming from a webhook payload, so we need to deserialize both,
// but still use uppercase for serialization
impl<'de> Deserialize<'de> for ReviewState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = ReviewState;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(match value {
                    "APPROVED" | "approved" => ReviewState::Approved,
                    "PENDING" | "pending" => ReviewState::Pending,
                    "CHANGES_REQUESTED" | "changes_requested" => ReviewState::ChangesRequested,
                    "COMMENTED" | "commented" => ReviewState::Commented,
                    "DISMISSED" | "dismissed" => ReviewState::Dismissed,
                    unknown => return Err(E::custom(format!("unknown variant `{}`, expected one of `approved`, `pending`, `changes_requested`, `commented`", unknown))),
                })
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}
