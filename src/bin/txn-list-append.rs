use serde::Deserialize;

#[tokio::main]
async fn main() {}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    Txn { msg_id: usize, txn: Vec<Txn> },
}
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Txn {
    Read(R, serde_json::Value, Option<Vec<serde_json::Value>>),
    Append(Append, serde_json::Value, serde_json::Value),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum R {
    R,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Append {
    Append,
}
