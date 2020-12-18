use thiserror::Error;

#[derive(Error, Debug)]
pub enum VideoSourceError {
    #[error("错误: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("需要登录")]
    NeedLogin,
    #[error("请求错误: {0}")]
    RequestError(String),
    #[error("找不到资源: {0}")]
    NoSuchResource(String),
}
