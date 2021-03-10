//! A connection pool manager for GitHub.
//!
//! In addition to being the caretaker of the available pool of connections (both creating new and recycling
//! old ones); it stores the `Personal Access Token` to access GitHub's REST API.
//! <br/><br/>
//!
//! ### Usage example:
//!
//! ```rust
//! use crate::github::client::pool::{GitHubConnectionPool, GitHubConnectionPoolManager, GitHubPoolError};
//!
//! let github_token = "MY_AWESOME_PERSONAL_ACCESS_TOKEN";
//! let connection_pool_size = 16;
//!
//! // initialize GitHub's connection pool -
//! GitHubConnectionPool::new(
//!     GitHubConnectionPoolManager::new(github_token),
//!     connection_pool_size
//! );
//! ```
//!
//! See more: [https://docs.github.com/en/github/authenticating-to-github/creating-a-personal-access-token](https://docs.github.com/en/github/authenticating-to-github/creating-a-personal-access-token)

use async_trait::async_trait;
use log::trace;
use octocrab::Octocrab;

#[derive(Debug)]
pub enum GitHubPoolError {}

pub struct GitHubConnectionPoolManager {
    github_personal_token_param: String,
}
impl GitHubConnectionPoolManager {
    /// Instantiates a new [`GitHubConnectionPoolManager`].
    pub fn new(github_personal_token_param: &str) -> Self {
        GitHubConnectionPoolManager {
            github_personal_token_param: github_personal_token_param.to_string(),
        }
    }

    /// Retrieves a GitHub client configured with a particular pre-loaded personal token.
    fn get_github_client(&self) -> Octocrab {
        Octocrab::builder()
            .personal_token(self.github_personal_token_param.clone())
            .build()
            .expect("Could not build GitHub client. Aborting operation.")
    }
}

pub type GitHubConnectionPool = deadpool::managed::Pool<Octocrab, GitHubPoolError>;

#[async_trait]
/// Managed pool's async implementation for [`deadpool`]'s generic trait. Here is where the black magic happens.
/// See more: [https://docs.rs/crate/deadpool/0.7.0](https://docs.rs/crate/deadpool/0.7.0)
impl deadpool::managed::Manager<Octocrab, GitHubPoolError> for GitHubConnectionPoolManager {
    async fn create(&self) -> Result<Octocrab, GitHubPoolError> {
        trace!("Retrieving new connection from the pool...");
        Ok(self.get_github_client())
    }

    async fn recycle(
        &self, _old: &mut Octocrab,
    ) -> deadpool::managed::RecycleResult<GitHubPoolError> {
        trace!("Recycling connection back into the pool...");
        Ok(())
    }
}
