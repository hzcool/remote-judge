use super::provider::{Problem, Provider, SubmissionStatus};
use super::utils::{extract_integer, get_text_arr_of_children_element, get_text_of_element};
use super::Handler;

use crate::global::{self, judge_status, remote_judge_config, remote_judge_constant as constant};
use anyhow::{anyhow, Ok};
use async_trait::async_trait;
use once_cell::sync::OnceCell;
use scraper::{ElementRef, Html, Selector};
use tokio::sync::Mutex;

async fn get_hdu_handler() -> anyhow::Result<&'static Handler> {
    static HANDLERS: OnceCell<Vec<Handler>> = OnceCell::new();
    let handlers = HANDLERS.get_or_init(|| {
        let Some(config) = remote_judge_config(constant::names::HDU) else {
            return vec![];
        };
        config
            .accounts
            .iter()
            .map(|account| {
                Handler::new(
                    account.handler.clone(),
                    account.password.clone(),
                    constant::base_url::HDU,
                )
            })
            .collect()
    });
    if handlers.is_empty() {
        return Err(anyhow!("未注册 hdu 账户"));
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

fn status_map(status: &str) -> String {
    for (&k, &v) in global::judge_status_map::HDU_STATUS_MAP.get().unwrap() {
        if status.contains(k) {
            return v.to_string();
        }
    }
    global::judge_status::RE.into()
}

pub struct Hdu {
    h: &'static Handler,
}

impl Hdu {
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            h: get_hdu_handler().await?,
        })
    }

    pub fn html_to_markdown(html: &str) -> String {
        let s = html2md::parse_html(html);
        let mut ret = String::new();
        let mut chars = s.chars();
        while let Some(mut c) = chars.next() {
            loop {
                if c != '\\' {
                    ret.push(c);
                    break;
                } else {
                    let cc = chars.next().unwrap_or_default();
                    if cc.is_alphabetic() || ['{', '}', '[', ']'].contains(&cc) {
                        ret.push(c)
                    }
                    c = cc
                }
            }
        }
        let re = regex::Regex::new(r"\[\]\((?P<p>.*)\)").unwrap();
        re.replace_all(&ret, "[](http://acm.hdu.edu.cn/$p)").into()
    }

    pub async fn is_login(&self) -> anyhow::Result<()> {
        let resp = self.h.req.get("index.php").await?;
        let text = resp.text().await?;
        if !text.contains(&self.h.username) {
            return Err(anyhow!("未登录"));
        }
        Ok(())
    }

    pub async fn ensure_login(&self) -> anyhow::Result<()> {
        if self.is_login().await.is_ok() {
            return Ok(());
        }

        let data = serde_json::json!({
            "username": self.h.username,
            "userpass": self.h.password,
            "login": "Sign In"
        });

        self.h
            .req
            .post("userloginex.php?action=login", &data)
            .await?;
        self.is_login().await
    }

    pub async fn extract_submission_status_from_html(
        html: &str,
    ) -> anyhow::Result<SubmissionStatus> {
        let document = Html::parse_document(html);
        let selector = Selector::parse(r#"div[id="fixed_table"] table tbody tr"#).unwrap();
        let mut iter = document.select(&selector);
        iter.next();
        if let Some(ele) = iter.next() {
            let v = get_text_arr_of_children_element(ele);
            if v.len() < 8 {
                return Err(anyhow!("获取 submission status 失败"));
            }

            let status = status_map(&v[2]);
            let res = SubmissionStatus {
                submission_id: v[0].clone(),
                status: status.clone(),
                info: status.clone(),
                is_over: !(["Running", "Que", "Pending", "Compiling"]
                    .iter()
                    .any(|&s| v[2].contains(s))),
                score: if status == judge_status::AC { 100 } else { 0 },
                time: extract_integer(&v[4]),
                memory: extract_integer(&v[5]),
                compile: None,
                judge: None,
            };

            return Ok(res);
        }

        Err(anyhow!("获取 submission status 失败"))
    }

    pub async fn get_compile_info(&self, submission_id: &str) -> anyhow::Result<String> {
        let resp = self
            .h
            .req
            .get(&format!("viewerror.php?rid={}", submission_id))
            .await?;
        let html = &resp.text().await?;
        let document = Html::parse_document(html);
        let selector = Selector::parse(r#"pre"#).unwrap();
        if let Some(x) = document.select(&selector).next() {
            return Ok(get_text_of_element(x));
        }
        Ok("".into())
    }
}

#[async_trait]
impl Provider for Hdu {
    async fn get_problem(&self, problem_id: &str) -> anyhow::Result<Problem> {
        let resp = self
            .h
            .req
            .get(&format!("showproblem.php?pid={}", problem_id))
            .await?;

        let text = &resp.text().await?;
        if text.contains("Show Problem - System Message") {
            return Err(anyhow!("No such problem"));
        }

        let document = Html::parse_document(&text);
        let selector = Selector::parse(r#"div[class="panel_content"]"#).unwrap();

        let mut problem = Problem::default();
        problem.problem_id = problem_id.into();
        document.select(&selector).for_each(|x| {
            let mut nodes = x.prev_siblings();
            while let Some(p) = nodes.next() {
                if let Some(t) = ElementRef::wrap(p) {
                    // let text = get_text_of_html_str(&t.inner_html());
                    let text = get_text_of_element(t);
                    if text.contains("Problem Description") {
                        problem.description = Hdu::html_to_markdown(&x.html());
                    } else if text.contains("Sample Input") {
                        problem.examples_input.push(get_text_of_element(x))
                    } else if text.contains("Sample Output") {
                        problem.examples_output.push(get_text_of_element(x))
                    } else if text.contains("Input") {
                        problem.input_format = Hdu::html_to_markdown(&x.html())
                    } else if text.contains("Output") {
                        problem.output_format = Hdu::html_to_markdown(&x.html())
                    } else if text.contains("Hint") {
                        problem.limit_and_hint = Hdu::html_to_markdown(&x.html())
                    }
                    return;
                }
            }
        });

        let s2 = Selector::parse("h1[style]").unwrap();
        if let Some(ele) = document.select(&s2).next() {
            problem.title = get_text_of_element(ele);
            // get_text_of_html_str(&ele.html());

            if let Some(node) = ele.next_sibling() {
                if let Some(e) = ElementRef::wrap(node) {
                    let re = regex::Regex::new(r"Time Limit: (\d+).*Memory Limit: (\d+)/").unwrap();
                    let text = get_text_of_element(e);
                    // &get_text_of_html_str(&e.html());
                    let mut iter = re.captures_iter(text.as_str());
                    if let Some(cap) = iter.next() {
                        problem.time_limit = (cap[1].parse()).unwrap_or_default();
                        problem.memory_limit = (cap[2].parse::<u32>()).unwrap_or_default() / 1024;
                    }
                }
            }
        }

        Ok(problem)
    }

    async fn submit_code(
        &self,
        problem_id: &str,
        source: &str,
        lang: &str,
    ) -> anyhow::Result<(String, serde_json::Value)> {
        self.ensure_login().await?;

        let data = serde_json::json!({
            "check": 0,
            "problemid": problem_id,
            "language": lang,
            "_usercode": base64_url::encode(urlencoding::encode(source).as_bytes())
        });

        self.h.accquire(problem_id).await;
        let furure = || async {
            let resp = self.h.req.post("submit.php?action=submit", &data).await?;

            let text = &resp.text().await?;
            if !text.contains("Realtime Status") {
                return Err(anyhow!("提交失败"));
            }

            let resp = self
                .h
                .req
                .get(&format!(
                    "status.php?user={}&pid={}",
                    self.h.username, problem_id
                ))
                .await?;
            let text = resp.text().await?;
            let res = Hdu::extract_submission_status_from_html(&text).await?;

            Ok(res.submission_id)
        };
        self.h.release(problem_id).await;
        let res = furure().await?;

        let value = serde_json::json!({"submissionId": res,  "account": self.h.username});
        Ok((res, value))
    }

    async fn poll(&self, submission_id: &str) -> anyhow::Result<SubmissionStatus> {
        let resp = self
            .h
            .req
            .get(&format!("status.php?first={}", submission_id))
            .await?;

        let html = &resp.text().await?;
        if !html.contains("Realtime Status") {
            return Err(anyhow!("获取 Status 失败"));
        }

        let mut s = Hdu::extract_submission_status_from_html(html.as_str()).await?;

        if s.status.as_str() == judge_status::CE {
            s.compile = Some(serde_json::json!({
                "message": self.get_compile_info(submission_id).await.unwrap_or_default()
            }))
        }
        Ok(s)
    }
}

// #[cfg(test)]
// mod tests {

//     use super::*;

//     #[tokio::test]
//     async fn test_hdu_get_problem() {
//         crate::global::init_config();

//         let h = Hdu::new().await.unwrap();

//         let res = h.get_problem("1006").await.unwrap();
//         println!("{:?}", res);
//     }

//     #[tokio::test]
//     async fn test_hdu_submit_code() {
//         crate::global::init_config();
//         let problem_id = "1000";
//         let source = r"
//         #include <iostream>
//         using namespace std;
//         int main() {
//             int a, b;
//             while (cin >> a >> b) {
//                 cout << a + b << endl;
//             }
//             return 0;
//         }";

//         let h = Hdu::new().await.unwrap();
//         let sid = h.submit_code(problem_id, source, "0").await.unwrap();

//         loop {
//             let t = h.poll(&sid).await.unwrap();
//             println!("{:?}", t);
//             if t.is_over {
//                 break;
//             }
//             tokio::time::sleep(Hdu::interval()).await;
//         }
//     }

//     #[tokio::test]
//     async fn test_multi_task() {
//         crate::global::init_config();

//         for i in 0..20 {
//             tokio::spawn(async move {
//                 let problem_id = "1000";
//                 let source = r"
//                     #include <iostream>
//                     using namespace std;
//                     int main() {
//                         int a, b;
//                         while (cin >> a >> b) {
//                             cout << a + b << endl;
//                         }
//                         return 0;
//                     }";
//                 let h = Hdu::new().await.unwrap();
//                 let sid = h.submit_code(problem_id, source, "0").await;
//                 if sid.is_err() {
//                     println!("submit error : {} {}", i, sid.err().unwrap());
//                 } else {
//                     println!("submit success {} {}", i, sid.unwrap());
//                 }
//             });
//         }
//         tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
//     }
// }
