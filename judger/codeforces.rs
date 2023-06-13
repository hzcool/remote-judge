use super::Handler;
use super::utils::request::PostConfig;
use crate::global::{self, remote_judge_config, remote_judge_constant as constant};
use crate::judger::utils::get_text_of_element;
use std::collections::HashMap;

use super::provider::{Problem, Provider, SubmissionStatus};
use super::utils::{
    extract_integer, get_text_arr_of_children_element, get_text_arr_of_html_str,
    get_text_of_html_str,
};

use anyhow::{anyhow, Ok};
use hyper::HeaderMap;
use once_cell::sync::OnceCell;
use rand::prelude::*;
use scraper::{ElementRef, Html, Selector};
use simple_log::info;
use tokio::sync::{Mutex, RwLock};

async fn get_codeforces_handler() -> anyhow::Result<&'static Handler> {
    static HANDLERS: OnceCell<Vec<Handler>> = OnceCell::new();
    let handlers = HANDLERS.get_or_init(|| {
        let Some(config) = remote_judge_config(constant::names::CODEFORCES) else {
            return vec![];
        };
        config
            .accounts
            .iter()
            .map(|account| {
                Handler::new(
                    account.handler.clone(),
                    account.password.clone(),
                    constant::base_url::CODEFORCES,
                )
            })
            .collect()
    });
    if handlers.is_empty() {
        return Err(anyhow!("未注册 Codeforces 账户"));
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
    for (&k, &v) in global::judge_status_map::CODEFORCES_STATUS_MAP.get().unwrap() {
        if status.contains(k) {
            return v.to_string();
        }
    }
    global::judge_status::RE.into()
} 

fn records_map() -> &'static RwLock<HashMap<String, (String, String)>> {
    static MAP: OnceCell<RwLock<HashMap<String, (String, String)>>> = OnceCell::new();
    MAP.get_or_init(|| RwLock::new(HashMap::new()))
}

async fn record_set(submission_id: String, contest_problem: (String, String)) {
    records_map().write().await.insert(submission_id.clone(), contest_problem);
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
        let _ = records_map().write().await.remove(submission_id.as_str());
    });
}


pub struct Codeforces {
    h: &'static Handler,
    for_gym: bool,
    csrf_token: tokio::sync::RwLock<String>,
}



impl Codeforces {
    pub async fn new(for_gym: bool) -> anyhow::Result<Self> {
        Ok(Self {
            h: get_codeforces_handler().await?,
            for_gym,
            csrf_token: tokio::sync::RwLock::new(String::new()),
        })
    }

   pub fn get_x_csrf_token(text: &str) -> Option<String> {
        static RE: OnceCell<regex::Regex> = OnceCell::new();
        let re = RE.get_or_init(|| {
            regex::Regex::new(r#"name="X-Csrf-Token"\s+content="([[:alnum:]]+)""#).unwrap()
        });
        if let Some(cap) = re.captures_iter(text).next() {
            // println!("token {}", &cap[1]);
            return Some(cap[1].into())
        }
        return None;
   }

    pub fn is_login(text: &str) -> anyhow::Result<(bool, String)> { // csrf_token
        let Some(csrf_token) = Codeforces::get_x_csrf_token(text) else {
            info!("codeforces 页面访问未包含 X-Csrf-Token, 可能访问已经被拦截 !!!");
            return Err(anyhow!("codeforces 页面访问未包含 X-Csrf-Token, 可能访问已经被拦截 !!!"));
        };
        let pos: usize = text.find("Enter").unwrap_or(10000000);
        if (pos + 120 < text.len() && (&text[pos..pos + 120]).find("Register").is_some()) 
            || (text.len() < 1000 && text.contains("Redirecting..."))
        {
            return Ok((false, csrf_token))
        }
        Ok((true, csrf_token))
    }

    pub async fn ensure_login(&self) -> anyhow::Result<String> {
        let resp = self.h.req.get("edu/courses").await?;
        let text = resp.text().await?;
        let(logged,csrf_token) = Codeforces::is_login(text.as_str())?;
        if logged {
            return Ok(csrf_token);
        }
        let data = serde_json::json!({
            "handleOrEmail": self.h.username,
            "password": self.h.password,
            "action": "enter",
            "csrf_token": csrf_token
        });
        let resp = self.h.req.post("enter", &data).await?;
        let text = resp.text().await?;
        let(logged, csrf_token) = Codeforces::is_login(text.as_str())?;
        if logged {Ok(csrf_token)} else {Err(anyhow!("登录失败"))}
    }

    pub fn html_to_markdown(html: &str) -> String {
        let s = html2md::parse_html(html).replace("$$$", "$");
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
        ret
    }


    pub fn extract_submission_id_from_html(
        html: &str,
        index: Option<&str>,
    ) -> anyhow::Result<String> {
        let document = Html::parse_document(html);
        let selector = Selector::parse(r#"tr[data-submission-id]"#).unwrap();
        for ele in document.select(&selector) {
            let td_s = Selector::parse(r#"td[data-problemid] a"#).unwrap();
            if let Some(td) = ele.select(&td_s).next() {
                if index.is_none()
                    || td
                        .value()
                        .attr("href")
                        .unwrap_or("")
                        .ends_with(index.unwrap())
                {
                    if let Some(sid) = ele.value().attr("data-submission-id") {
                        return Ok(sid.into());
                    }
                }
            }
        }
        Err(anyhow!("无法找到提交信息"))
    }

    pub async fn get_contest_info(
        &self,
        contest_id: &str,
        for_gym: bool,
    ) -> anyhow::Result<(Vec<Problem>, String, String)> {
        // 题目列表， 比赛题目, 比赛状态
        let resp = self
            .h
            .req
            .get(&format!(
                "{}/{}",
                if for_gym { "gym" } else { "contest" },
                contest_id
            ))
            .await?;
        if !resp.url().as_str().ends_with(contest_id) {
            return Err(anyhow!("No Such Contest"));
        }
        let html = &resp.text().await?;
        let document = Html::parse_document(html);
        let mut selector = Selector::parse(r#"table[class="problems"] tr"#).unwrap();
        let v: Vec<_> = document
            .select(&selector)
            .skip(1)
            .map(|tr| get_text_arr_of_children_element(tr))
            .collect();

        let problems: Vec<_> = v
            .into_iter()
            .map(|x| {
                let y: Vec<String> = x.into_iter().filter(|s| !s.is_empty()).collect();
                let mut p = Problem::default();
                if y.len() >= 2 {
                    p.problem_id = y[0].clone();
                    let s = &y[1];
                    if let Some(pos) = s.find("standard") {
                        p.title = s[..pos].to_string();
                        let mut it = s[pos..].split(",");
                        if let Some(time_str) = it.next() {
                            p.time_limit = (extract_integer::<f32>(time_str) * 1000.0) as u32;
                        }
                        if let Some(mem_str) = it.next() {
                            p.memory_limit = extract_integer::<u32>(mem_str);
                        }
                    }
                }
                p
            })
            .collect();

        //获取比赛标题
        selector = Selector::parse(r#"div[id="sidebar"] table tr"#).unwrap();
        let mut it = document.select(&selector);
        let contest_title = match it.next() {
            None => "".to_string(),
            Some(ele) => get_text_of_element(ele),
        };
        let contest_status = match it.next() {
            None => "".to_string(),
            Some(ele) => get_text_of_element(ele),
        };

        // println!("{:?} \n {} {}", problems, contest_title, contest_status);
        Ok((problems, contest_title, contest_status))
    }
}

impl Provider for Codeforces {
    async fn get_problem(&self, problem_id: &str) -> anyhow::Result<Problem> {
        let pos = problem_id.find(|c: char| c.is_alphabetic()).unwrap_or(0);
        let contest_id = &problem_id[..pos];
        let problem_index = &problem_id[pos..];

        let resp = self
            .h
            .req
            .get(&format!(
                "{}/{contest_id}/problem/{problem_index}",
                if self.for_gym { "gym" } else { "contest" },    
            ))
            .await?;
        // println!("{:?}", resp);

        let url = resp.url().clone();
        if !url.as_str().contains(contest_id) {
            return Err(anyhow!("No such problem"));
        }
        let text = resp.text().await?;

        if self.for_gym && url.as_str().ends_with("attachments") {
            let mut p = Problem::default();
            p.problem_id = problem_id.into();
            if let Result::Ok((ps, _, _)) = self.get_contest_info(contest_id, self.for_gym).await {
                for item in ps.into_iter() {
                    if item.problem_id == problem_index {
                        p.title = item.title;
                        p.time_limit = item.time_limit;
                        p.memory_limit = item.memory_limit;
                    }
                }
            };

            let document = Html::parse_document(text.as_str());
            let selector = Selector::parse(r#"div[class="datatable"] table td a"#).unwrap();
            return match document.select(&selector).next() {
                None => Err(anyhow!("获取 gym 题目失败")),
                Some(e) => {
                    p.description = format!(
                        "题目文件: [problemset.pdf](https://codeforces.com{})",
                        e.value().attr("href").unwrap_or("")
                    );

                    Ok(p)
                }
            };
        }

        let document = Html::parse_document(text.as_str());
        let selector = Selector::parse(r#"div[class="problem-statement"]"#).unwrap();

        let Some(statement) = document.select(&selector).next() else {
            return Err(anyhow!("查找题目失败"));
        };

        let mut problem = Problem::default();
        problem.problem_id = problem_id.into();
        let mut items = statement.children();
        if let Some(header) = items.next() {
            if let Some(header_ele) = ElementRef::wrap(header) {
                let v = get_text_arr_of_children_element(header_ele);
                if v.len() >= 3 {
                    problem.title = v[0].clone();
                    problem.time_limit = (extract_integer::<f32>(&v[1]) * 1000.0) as u32;
                    problem.memory_limit = extract_integer::<u32>(&v[2]);
                }
            }
        }

        let resolve = |select_str: &str| {
            let s = Selector::parse(select_str).unwrap();
            if let Some(e) = statement.select(&s).next() {
                let c = e.html();
                return Codeforces::html_to_markdown(&c[c.find("<p>").unwrap_or(0)..]);
            }
            return "".to_string();
        };

        let resolve_examples = |select_str: &str| {
            let s = Selector::parse(select_str).unwrap();
            statement
                .select(&s)
                .map(|ele| get_text_arr_of_html_str(&ele.html()).join("\n"))
                .collect()
        };

        if let Some(node) = items.next() {
            if let Some(ele) = ElementRef::wrap(node) {
                let c = ele.html();
                problem.description =
                    Codeforces::html_to_markdown(&c[c.find("<p>").unwrap_or(0)..]);
            }
        }
        problem.input_format = resolve(r#"div[class="input-specification"]"#);
        problem.output_format = resolve(r#"div[class="output-specification"]"#);
        problem.limit_and_hint = resolve(r#"div[class="note"]"#);
        problem.examples_input = resolve_examples(r#"div[class="input"] pre"#);
        problem.examples_output = resolve_examples(r#"div[class="output"] pre"#);

        Ok(problem)
    }

    async fn submit_code(
        &self,
        problem_id: &str,
        source: &str,
        lang: &str,
    ) -> anyhow::Result<(String, serde_json::Value)> {
        let csrf_token = self.ensure_login().await?;
        *self.csrf_token.write().await = csrf_token.clone();
        let pos = problem_id.find(|c: char| c.is_alphabetic()).unwrap_or(0);
        let rand_num: u64 = rand::thread_rng().gen::<u64>();
        let contest_id = &problem_id[..pos];
        let peoblem_idx = &problem_id[pos..];

        let data = serde_json::json!({
            "action": "submitSolutionFormSubmitted",
            "tabSize": 4,
            "source": format!("{}//{}", source, rand_num),
            "sourceFile": "",
            "contestId": contest_id,
            "submittedProblemIndex": peoblem_idx,
            "programTypeId": lang,
            "csrf_token": csrf_token
        });

        self.h.accquire(contest_id).await;
        let future = || async {
            let resp = self
                .h
                .req
                .post(
                    &format!(
                        "{}/{}/submit",
                        if self.for_gym { "gym" } else { "contest" },
                        &problem_id[..pos]
                    ),
                    &data,
                )
                .await?;

            if !resp.status().is_success() {
                return Err(anyhow!("提交代码失败"));
            }
            let reps2 = self.h.req.get(&format!("contest/{}/my", contest_id)).await?;
            let html = reps2.text().await?;
            Codeforces::extract_submission_id_from_html(&html, None)
        };
        
        self.h.release(contest_id).await;
        let res = future().await?;
        record_set(res.clone(), (contest_id.into(), peoblem_idx.into())).await;
        let value = serde_json::json!({"submissionId": res,  "account": self.h.username});
        Ok((res, value))
    }

    async fn poll(&self, submission_id: &str) -> anyhow::Result<SubmissionStatus> {
        let data = serde_json::json!({
            "submissionId": submission_id,
            "csrf_token": self.csrf_token.read().await.clone()
        });
        let header_map = {
            let mp = records_map().read().await;
            let x = mp.get(submission_id);
            let mut header_map = HeaderMap::new();
            header_map.insert("referer", format!("{}contest/{}/my", constant::base_url::CODEFORCES, x.unwrap().0 ).parse().unwrap());
            header_map
        };

        let config = PostConfig::new(header_map, None);
        
        let resp = self.h.req.post_with_config("data/submitSource", &data, config).await?;

        let text = resp.text().await?;
        let result = serde_json::from_str::<HashMap<String, String>>(&text).unwrap_or_default();

        if result.is_empty() {
            return Err(anyhow!("获取失败"));
        }

        let verdict = get_text_of_html_str(result.get("verdict").unwrap_or(&"".into()));
        if verdict.as_str() == "" {
            return Err(anyhow!("获取失败"));
        }

        let judge_status = status_map(&verdict);
        let mut status = SubmissionStatus::default();
        status.submission_id = submission_id.into();
        status.is_over = !(["Running", "Pending", "queue"]
            .iter()
            .any(|&s| verdict.contains(s)));
        status.status = judge_status;
        status.info = verdict;
        if status.is_over {
            if status.status == global::judge_status::CE {
                status.compile = Some(serde_json::json!({
                    "message": result.get("checkerStdoutAndStderr#1")
                }))

            } else {
                if status.status == global::judge_status::AC {
                    status.score = 100
                }

                if !self.for_gym {
                    let count: usize = result
                        .get("testCount")
                        .unwrap_or(&"0".into())
                        .parse()
                        .unwrap_or(0);
                    let mut time = 0u32;
                    let mut memory = 0u32;

                    let mut cases = vec![];
                    for i in 1..count + 1 {
                        let _memory: u32 = result
                            .get(&format!("memoryConsumed#{}", i))
                            .unwrap_or(&"0".into())
                            .parse::<u32>()
                            .unwrap_or(0) / 1024;
                        let _time: u32 = result
                            .get(&format!("timeConsumed#{}", i))
                            .unwrap_or(&"0".into())
                            .parse()
                            .unwrap_or(0);

                        time = std::cmp::max(time, _time);
                        memory = std::cmp::max(memory, _memory);
                        cases.push(serde_json::json!({
                            "status": 2, 
                            "result": {
                                "type": if i < count { global::judge_status::AC.to_string() } else {status.status.clone()},
                                "scoringRate": if i < count {1} else { status.score / 100 },
                                "memory": _memory,
                                "time": _time,
                                "input": {
                                    "name": "---",
                                    "content": result.get(&format!("input#{}", i)),
                                },
                                "output": {
                                    "name": "---",
                                    "content": result.get(&format!("answer#{}", i)),
                                },
                                "userOutput": result.get(&format!("output#{}", i)),
                                "spjMessage": result.get(&format!("checkerStdoutAndStderr#{}", i))
                            }   
                        }));
                    }


                    status.time = time;
                    status.memory = memory;
                    status.judge = Some(serde_json::json!({
                        "subtasks": [{
                            "score": status.score,
                            "cases": cases
                        }]
                    }));
                }
            }
        }
        Ok(status)
    }
}

// mod tests {

//     use super::*;

//     #[tokio::test]
//     async fn test_ensure_login() {
//         crate::request::create_global_request();
//         crate::definition::init_config();

//         let h = Codeforces::new(false).await.unwrap();
//         assert!(h.ensure_login().await.is_ok());
//     }

//     #[tokio::test]
//     async fn test_get_problem() {
//         crate::request::create_global_request();
//         crate::definition::init_config();
//         let h = Codeforces::new(false).await.unwrap();

//         let x = h.get_problem("1812I").await;

//         assert!(x.is_ok());
//         println!("{:?}", x.unwrap());
//     }

//     #[tokio::test]
//     async fn test_get_gym_problem() {
//         crate::request::create_global_request();
//         crate::definition::init_config();
//         let h = Codeforces::new(true).await.unwrap();

//         let x = h.get_problem("104279A").await;

//         assert!(x.is_ok());
//         println!("{:?}", x.unwrap());
//     }

//     #[tokio::test]
//     async fn test_submit_code() {
//         crate::request::create_global_request();
//         crate::definition::init_config();

//         let h = Codeforces::new(true).await.unwrap();
//         println!("{} {}", h.h.username, h.h.password);
//         let source = include_str!("code.txt");
//         let res = h.submit_code("104279B", source, "54").await;
//         // let res = h.submit_code("11D", source, "54").await;

//         assert!(res.is_ok());
//         let sid = res.unwrap();
//         println!("{}", sid);
//         loop {
//             tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
//             let s = h.poll(&sid).await.unwrap();
//             println!("{:?}", s);
//             if s.is_over {
//                 break;
//             }
//         }
//     }
// }
