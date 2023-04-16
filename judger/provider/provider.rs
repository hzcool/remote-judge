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
    ) -> anyhow::Result<(String, serde_json::Value)>; // 返回 submission id 和 返回服务方其他信息

    async fn poll(&self, submission_id: &str) -> anyhow::Result<SubmissionStatus>;
}
