use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(tag = "request")]
#[serde(rename_all = "kebab-case")]
pub enum Request {
    Put {
        queue: String,
        pri: usize,
        job: Value,
    },
    Get {
        queues: Vec<String>,
        wait: Option<bool>,
    },
    Delete {
        id: usize,
    },
    Abort {
        id: usize,
    },
}
