//! Utilities for any and all `type`s that want to be able to establish a managed pool connection against
//! GitHub.

use async_trait::async_trait;
use deadpool::managed::Object;
use log::error;
use octocrab::Octocrab;

use crate::github::client::pool::{GitHubConnectionPool, GitHubPoolError};

pub type GitHubConnection = Object<Octocrab, GitHubPoolError>;

/// Trait for any and all `type`s that want to be able to establish a managed pool connection against
/// GitHub.
#[async_trait]
pub trait GitHubConnector {
    /// Getter for the GitHub connection pool;
    fn get_connection_pool(&self) -> &GitHubConnectionPool;

    /// Retrieves a GitHub client configured with a particular pre-loaded personal token from the connection pool.
    async fn get_github_client(&self) -> GitHubConnection {
        self.get_connection_pool().get().await.unwrap_or_else(|e| {
            error!("Could not retrieve a GitHub managed connection despite the pool being initialized (ran out of connections and hit a timeout?). Aborting operation.");
            panic!(e)
        })
    }
}
