use thiserror::Error;
use reqwest::Url;

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
    #[error("无效的链接: {0}")]
    InvalidUrl(Url),
}
