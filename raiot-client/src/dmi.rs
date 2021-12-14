#[derive(Debug, Clone)]
pub struct DMIRequest {
    pub method_name: String,
    pub body: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct DMIResult {
    pub status: i32,
    pub payload: Option<serde_json::Value>,
}

pub type DMIHandler = fn(DMIRequest) -> DMIResult;
