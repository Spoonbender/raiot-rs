use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct D2CMsg {
    pub content: Option<serde_json::Value>,
    pub headers: Option<HashMap<String, String>>,
}
