use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResponseStatus {
    Ok,
    Error,
    NoJob,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum Response {
    Put {
        status: ResponseStatus,
        id: usize,
    },
    Get {
        status: ResponseStatus,
        id: usize,
        queue: String,
        pri: usize,
        job: Value,
    },
    Delete {
        status: ResponseStatus,
    },
    Abort {
        status: ResponseStatus,
    },
    Err {
        status: ResponseStatus,
    },
}
