// ml4w-zh — 自定义错误类型

use std::io;

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum Ml4wError {
    #[error("网络请求失败: {0}")]
    Network(String),
    #[error("文件操作失败: {0}")]
    File(#[from] io::Error),
    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),
    #[error("编码错误: {0}")]
    Encoding(String),
    #[error("校验失败: {0}")]
    Validation(String),
}
