use super::Handler;
use crate::global::{self, judge_status, remote_judge_config, remote_judge_constant as constant};
use crate::judger::utils::get_text_of_element;

use super::provider::{Problem, Provider, SubmissionStatus};
use super::utils::{extract_integer, get_text_arr_of_children_element};

use anyhow::{anyhow, Ok};
use once_cell::sync::OnceCell;
use scraper::{Html, Selector};
use tokio::sync::Mutex;

async fn get_atcoder_handler() -> anyhow::Result<&'static Handler> {
    static HANDLERS: OnceCell<Vec<Handler>> = OnceCell::new();
    let handlers = HANDLERS.get_or_init(|| {
        let Some(config) = remote_judge_config(constant::names::ATCODER) else {
            return vec![];
        };
        config
            .accounts
            .iter()
            .map(|account| {
                Handler::new(
                    account.handler.clone(),
                    account.password.clone(),
                    constant::base_url::ATCODER,
                )
            })
            .collect()
    });
    if handlers.is_empty() {
        return Err(anyhow!("未注册 atcoder 账户"));
    }

    static IDX: OnceCell<Mutex<usize>> = OnceCell::new();
    let idx = IDX.get_or_init(|| Mutex::new(0));

    let mut c = idx.lock().await;
    let now = *c;
    *c += 1;
    if *c >= handlers.len() {
        *c = 0
    }
    Ok(&handlers[now])
}

pub fn status_map(status: &str) -> String {
    return global::judge_status_map::ATCODER_STATUS_MAP
        .get()
        .unwrap()
        .get(status)
        .unwrap_or(&judge_status::RN.into())
        .to_string();
}

pub struct Atcoder {
    h: &'static Handler,
}

impl Atcoder {
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            h: get_atcoder_handler().await?,
        })
    }

    pub fn is_login(resp: &reqwest::Response) -> bool {
        resp.headers()
            .get_all("set-cookie")
            .iter()
            .any(|f| f.to_str().unwrap_or_default().contains("SessionKey"))
    }

    pub fn get_csrf_token(text: &str) -> Option<String> {
        let Some(pos) = text.find("var csrfToken = ") else {
            return None
        };
        let l = (&text[pos..]).find('"').unwrap_or(100000000) + pos;
        if l >= text.len() {
            return None;
        }
        Some(
            (text[l + 1..])
                .chars()
                .take_while(|&c| c != '"')
                .map(|c| c)
                .collect(),
        )
    }

    pub async fn ensure_login(&self) -> anyhow::Result<String> {
        let resp = self.h.req.get("").await?;
        let logged = Atcoder::is_login(&resp);
        let text = resp.text().await?;
        let csrf_token = Atcoder::get_csrf_token(&text).unwrap_or_default();
        if logged {
            return Ok(csrf_token);
        }

        let data = serde_json::json!({
            "username": self.h.username,
            "password": self.h.password,
            "csrf_token": csrf_token,
        });
        let resp2 = self.h.req.post("login", &data).await?;
        if Atcoder::is_login(&resp2) {
            return Ok(csrf_token);
        }
        Err(anyhow!("无法登录"))
    }

    fn extract_submission_id_from_html(html: &str) -> anyhow::Result<String> {
        let document = Html::parse_document(html);
        let selector = Selector::parse(r#"tr td[data-id]"#).unwrap();

        if let Some(x) = document.select(&selector).next() {
            return Ok(x.value().attr("data-id").unwrap_or_default().into());
        }
        Err(anyhow!("提交失败"))
    }
}

impl Provider for Atcoder {
    async fn get_problem(&self, problem_id: &str) -> anyhow::Result<Problem> {
        let pos = problem_id.find('_').unwrap_or(0);
        let contest_id = &problem_id[..pos];
        // let problem_idx = &problem_id[pos + 1..];
        let resp = self
            .h
            .req
            .get(&format!("contests/{}/tasks/{}", contest_id, problem_id))
            .await?;
        let text = resp.text().await?;
        if text.contains("404 Page Not Found") {
            return Err(anyhow!("No such problem"));
        }

        let mut problem = Problem::default();
        let document = Html::parse_document(&text);
        let selector = Selector::parse(r#"meta[property="twitter:title"]"#).unwrap();
        if let Some(e) = document.select(&selector).next() {
            problem.title = e.value().attr("content").unwrap_or_default().to_string()
        }

        let pos = text.find("Time Limit: ").unwrap_or(100000);
        if pos < text.len() {
            problem.time_limit = (extract_integer::<f32>(&text[pos..pos + 20]) * 1000.0) as u32;
            problem.memory_limit = extract_integer::<u32>(&text[pos + 20..pos + 40]);
            problem.description = format!(
                "[题目链接]({}contests/{}/tasks/{})",
                self.h.req.base_url, contest_id, problem_id
            );
        }
        Ok(problem)
    }
    async fn submit_code(
        &self,
        problem_id: &str,
        source: &str,
        lang_id: &str,
    ) -> anyhow::Result<(String, serde_json::Value)> {
        let csrf_token = self.ensure_login().await?;
        let pos = problem_id.find('_').unwrap_or(0);
        let contest_id = &problem_id[..pos];
        // let problem_idx = &problem_id[pos + 1..];

        // https://atcoder.jp/contests/abc064/submit

        let data = serde_json::json!({
            "data.TaskScreenName": problem_id,
            "data.LanguageId": lang_id,
            "sourceCode": source,
            "csrf_token":csrf_token
        });

        self.h.accquire(problem_id).await;

        let future = || async {
            let resp = self
                .h
                .req
                .post(&format!("contests/{}/submit", contest_id), &data)
                .await?;

            if !resp.url().as_str().ends_with("me") {
                return Err(anyhow!("提交代码失败"));
            }
            let html = resp.text().await?;
            let sid = Atcoder::extract_submission_id_from_html(&html)?;
            Ok(format!("{}_{}", contest_id, sid))
        };
        self.h.release(problem_id).await;
        let res = future().await?;
        let pos = res.find('_').unwrap();
        let value =
            serde_json::json!({"submissionId": &res[pos + 1..],  "account": self.h.username});
        Ok((res, value))
    }

    async fn poll(&self, submission_id: &str) -> anyhow::Result<SubmissionStatus> {
        let pos = submission_id.find('_').unwrap_or(0);
        let contest_id = &submission_id[..pos];
        let sid = &submission_id[pos + 1..];

        let resp = self
            .h
            .req
            .get(&format!("contests/{}/submissions/{}", contest_id, sid))
            .await?;

        let text = resp.text().await?;
        let document = Html::parse_document(&text);
        let selector = Selector::parse(r#"td[id="judge-status"] span"#).unwrap();

        if let Some(ele) = document.select(&selector).next() {
            let mut s = SubmissionStatus::default();
            s.info = get_text_of_element(ele);
            s.status = status_map(&s.info);
            s.submission_id = sid.to_string();

            if s.status != judge_status::WT && s.status != judge_status::RN {
                s.is_over = true;
                if s.status == judge_status::CE {
                    let pre_selector = Selector::parse(r#"pre"#).unwrap();
                    if let Some(e) = document.select(&pre_selector).last() {
                        s.compile = Some(serde_json::json!({ "message": get_text_of_element(e) }))
                    }
                } else {
                    if s.status == judge_status::AC {
                        s.score = 100
                    }

                    let l = text.find("Exec Time").unwrap_or(1000000);
                    if l < text.len() {
                        let r = (&text[l..]).find("ms").unwrap_or(0) + l;
                        s.time = extract_integer(&text[r - 15..r]);

                        let l = text.find("Memory").unwrap_or(1000000);
                        let r = (&text[l..]).find("KB").unwrap_or(0) + l;
                        s.memory = extract_integer(&text[r - 15..r]);
                    }

                    let selector = Selector::parse(r#"table[class]"#).unwrap();
                    let tr_select = Selector::parse(r#"tr"#).unwrap();

                    if let Some(ele) = document.select(&selector).last() {
                        let mut cases = vec![];

                        ele.select(&tr_select).skip(1).for_each(|tr| {
                            let tds: Vec<String> = get_text_arr_of_children_element(tr)
                                .into_iter()
                                .filter(|s| !s.is_empty())
                                .collect();

                            if tds.len() >= 4 {
                                let _status = status_map(&tds[1]);
                                cases.push(serde_json::json!({
                                    "result": {
                                        "type" : _status,
                                        "scoringRate": if _status == judge_status::AC {1} else {0},
                                        "time": extract_integer::<u32>(&tds[2]),
                                        "memory": extract_integer::<u32>(&tds[3]),
                                        "input": {
                                            "name": tds[0],
                                            "content": ""
                                        }
                                    },
                                    "status": 2
                                }))
                            }
                        });

                        s.judge = Some(serde_json::json!({
                            "subtasks": [{
                                "score": s.score,
                                "cases": cases
                            }]
                        }));
                    }
                }
            }

            return Ok(s);
        }
        Err(anyhow!("poll status 失败"))
    }
}

// #[cfg(test)]
// mod tests {

//     use super::*;

//     #[tokio::test]
//     async fn test_get_problem() {
//         crate::global::init_config();
//         let h = Atcoder::new().await.unwrap();
//         let p = h.get_problem("abc297_a").await.unwrap();
//         println!("{:?}", p);
//     }

//     #[tokio::test]
//     async fn test_ensure_login() {
//         crate::global::init_config();
//         let h = Atcoder::new().await.unwrap();
//         let csrf_token = h.ensure_login().await.unwrap();
//         println!("{}", csrf_token);
//     }

//     #[tokio::test]
//     async fn test_submit_code() {
//         crate::global::init_config();
//         let h = Atcoder::new().await.unwrap();
//         let problem_id = "abc064_c";
//         let source = include_str!("../code.txt");
//         let lang = "4003";
//         let sid = h.submit_code(problem_id, source, lang).await;
//         println!("{:?}", sid);
//     }

//     #[tokio::test]
//     async fn test_poll() {
//         crate::global::init_config();
//         let h = Atcoder::new().await.unwrap();
//         let x = h.poll("arc159_40605169").await.unwrap();
//         println!("{:?}", x);
//     }
// }
