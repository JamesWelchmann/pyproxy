#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum CodeExecResult {
    Exeception(Vec<u8>),
    Return(Vec<u8>),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CodeExec {
    pub future_id: String,
    pub code_exec_result: CodeExecResult,
}
