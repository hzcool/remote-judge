use super::{Problem, SubmissionStatus};
use async_trait::async_trait;

#[async_trait]
pub trait Provider: Send {
    async fn get_problem(&self, __problem_id: &str) -> anyhow::Result<Problem>;

    async fn submit_code(
        &self,
        __problem_id: &str,
        __source: &str,
        __lang: &str,
    ) -> anyhow::Result<String>;

    async fn poll(&self, submission_id: &str) -> anyhow::Result<SubmissionStatus>;

    fn get_handler_info(&self) -> serde_json::Value;
}
