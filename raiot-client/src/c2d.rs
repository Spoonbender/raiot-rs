use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct C2DMsg {
    pub body: Option<String>,
    pub props: Option<HashMap<String, String>>,
}

pub type C2DResult = Result<(), ()>;
pub type C2DHandler = fn(C2DMsg) -> C2DResult;
