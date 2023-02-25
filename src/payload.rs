use octocrab::models::{
    events::payload::Commit, issues::Comment, issues::Issue, pulls::PullRequest, Repository, User,
};
use serde::Deserialize;

use reqwest::Url;
use std::error::Error;

#[derive(Deserialize, Debug)]
pub struct PushPayload {
    //    pub sender: User,
    //    pub repository: Repository,
    //pub pusher: ..
    #[serde(rename = "ref")]
    pub _ref: String,
    pub before: String,
    pub after: String,
    pub created: bool,
    pub deleted: bool,
    pub forced: bool,
    pub base_ref: String,
    pub compare: Url,
    pub commits: Vec<Commit>,
    pub head_commit: Commit,
}

#[derive(Deserialize, Debug)]
pub struct IssueCommentPayload {
    /// The action (created/edited/deleted) that triggered the webhook.
    pub action: Action,
    /// The account that triggered the action that triggered the webhook.
    pub sender: User,
    /// The issue the comment was placed on.
    pub issue: Issue,
    /// The comment involved in the action.
    pub comment: Comment,
    /// The repository the issue belongs to.
    pub repository: Repository,
}

#[derive(Deserialize, Debug)]
pub struct IssuePayload {
    /// The action (created/edited/deleted/opened/closed) that triggered the webhook.
    pub action: Action,
    /// The account that triggered the action that triggered the webhook.
    pub sender: User,
    /// The issue the comment was placed on.
    pub issue: Issue,
    /// The repository the issue belongs to.
    pub repository: Repository,
}

#[derive(Deserialize, Debug)]
pub struct PullRequestPayload {
    /// The action (created/edited/deleted/opened/closed) that triggered the webhook.
    pub action: Action,
    /// The account that triggered the action that triggered the webhook.
    pub sender: User,
    /// The pull_request this payload is about.
    pub pull_request: PullRequest,
    /// The repository the pull request belongs to.
    pub repository: Repository,
}
/// Action represents the action the Github webhook is send for.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// The something was created.
    Created,
    /// The something has been edited.
    Edited,
    /// The something has been deleted.
    Deleted,
    /// The something has been opened.
    Opened,
    /// The something has been opened.
    Closed,
    /// The something has been reopened.
    Reopened,
    /// The something has been synchronized.
    Synchronized,
}

pub enum Payload {
    Ping,
    Issue(IssuePayload),
    IssueComment(IssueCommentPayload),
    PullRequest(PullRequestPayload),
    Push(PushPayload),
}

#[derive(thiserror::Error, Debug)]
pub enum PayloadError {
    #[error("unhandled event type `{0}`")]
    UnhandledEventType(String),
}

impl Payload {
    pub fn from(p: &str, event_type: &str) -> Result<Payload, Box<dyn Error>> {
        match event_type {
            "ping" => Ok(Self::Ping),
            "issue_comment" => Ok(Self::IssueComment(serde_json::from_str(p)?)),
            "issues" => Ok(Self::Issue(serde_json::from_str(p)?)),
            "pull_request" => Ok(Self::PullRequest(serde_json::from_str(p)?)),
            "push" => Ok(Self::Push(serde_json::from_str(p)?)),
            _ => Err(Box::new(PayloadError::UnhandledEventType(
                event_type.into(),
            ))),
        }
    }
}
