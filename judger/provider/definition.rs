use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Problem {
    pub problem_id: String,
    pub title: String,
    pub time_limit: u32,   // MS
    pub memory_limit: u32, // MB
    pub description: String,
    pub input_format: String,
    pub output_format: String,
    pub limit_and_hint: String,
    pub examples_input: Vec<String>,
    pub examples_output: Vec<String>,
    pub others: HashMap<String, String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct SubmissionStatus {
    pub submission_id: String,
    pub status: String,
    pub info: String,
    pub is_over: bool,
    pub score: u16,
    pub time: u32,   //MS
    pub memory: u32, //KB
    pub compile: Option<serde_json::Value>,
    pub judge: Option<serde_json::Value>,
}
