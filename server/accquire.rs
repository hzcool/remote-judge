// 获取各大 oj 通过的题目

use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug)]
struct Problem {
    contestId: i32,
    index: String,
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct SubmissionStatus {
    id: i64,
    problem: Problem,
    verdict: String,
}

pub async fn get_codeforces_solved_problems(handle: &str) -> anyhow::Result<Vec<String>> {
    let url = format!(
        "https://codeforces.com/api/user.status?handle={}&from=1&count=1000000",
        handle
    );
    let resp = reqwest::get(&url).await?.text().await?;
    let mut data: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(resp.as_bytes())?;
    let x = data.remove("result").unwrap_or(json!([]));
    let ss: Vec<SubmissionStatus> = serde_json::from_value(x).unwrap();
    let mut _res: Vec<(i32, String)> = ss
        .into_iter()
        .filter(|s| s.verdict.as_str() == "OK")
        .map(|s| (s.problem.contestId, s.problem.index))
        .collect();
    _res.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Less));
    let mut res = vec![];
    let mut pre = "".to_string();
    for (id, idx) in _res.into_iter() {
        let pid = format!("{}{}", id, idx);
        if !pid.eq(&pre) {
            pre = pid.clone();
            res.push(pid)
        }
    }
    Ok(res)
}

#[derive(Serialize, Deserialize, Debug)]
struct LojProblem {
    id: i64,
}

#[derive(Serialize, Deserialize, Debug)]
struct LojSubmission {
    id: i64,
    problem: LojProblem,
}
pub async fn get_loj_solved_problems(handle: &str) -> anyhow::Result<Vec<String>> {
    let url = "https://api.loj.ac/api/submission/querySubmission";
    let mut res = vec![];
    let mut max_id = 1000000000i64;
    const MAX_SIZE: usize = 10; // 只能是 10
    loop {
        let data = serde_json::json!({
            "locale": "zh_CN",
            "status": "Accepted",
            "submitter": handle,
            "takeCount": MAX_SIZE,
            "maxId": max_id,
        });
        let resp = reqwest::Client::new()
            .post(url)
            .json(&data)
            .send()
            .await?
            .text()
            .await?;
        let mut data: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(resp.as_bytes())?;
        let x = data.remove("submissions").unwrap_or(json!([]));
        let ss: Vec<LojSubmission> = serde_json::from_value(x).unwrap();
        ss.iter().for_each(|s| res.push(s.problem.id));
        if ss.len() < MAX_SIZE {
            break;
        }
        max_id = ss.last().unwrap().id - 1;
    }
    res.sort();
    let mut res2 = vec![];
    let mut pre = -1i64;
    for id in res.into_iter() {
        if id != pre {
            pre = id;
            res2.push(id.to_string());
        }
    }
    Ok(res2)
}

mod tests {

    use super::*;
    #[tokio::test]
    async fn test_get_loj_solved() {
        let x = get_loj_solved_problems("cz_xuyixuan").await.unwrap();
        println!("{:?}", x);
    }
}
