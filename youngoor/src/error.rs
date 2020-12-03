use thiserror::Error;

#[derive(Error, Debug)]
pub enum VideoSourceError {
    #[error("网络错误: {0}")]
    NetworkError(#[from] std::io::Error),
}
