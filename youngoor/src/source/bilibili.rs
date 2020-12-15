use super::VideoInfo;
use super::VideoSource;
use crate::error::VideoSourceError;
use async_trait::async_trait;
use reqwest::Url;
use serde::Deserialize;

#[derive(Debug)]
pub struct BilibiliSource {
    client: reqwest::Client,
    cookie: Option<String>,
}

#[async_trait]
impl VideoSource for BilibiliSource {
    fn pretty_name(&self) -> &'static str {
        "bilibili"
    }

    async fn video_list(&self, url: &Url) -> Result<Vec<VideoInfo>, VideoSourceError> {
        unimplemented!()
    }

    fn valid(&self, url: &Url) -> bool {
        if let Some(host) = url.host_str() {
            let is_host = host.eq_ignore_ascii_case("www.bilibili.com")
                || host.eq_ignore_ascii_case("bilibili.com");
            let is_path = {
                let path = url.path().to_ascii_lowercase();
                path.starts_with("/video/") // 视频
                    || path.starts_with("/bangumi/") // 剧集
            };
            is_host && is_path
        } else {
            false
        }
    }
}

impl Default for BilibiliSource {
    fn default() -> Self {
        BilibiliSource {
            client: reqwest::Client::new(),
            cookie: None,
        }
    }
}

/// Bilibili响应格式
#[derive(Debug, Deserialize)]
struct Response<T> {
    pub code: i32,
    pub message: String,
    ttl: i32,
    pub data: Option<T>,
}

/// Bilibili分P
#[derive(Debug, Deserialize)]
struct PInfo {
    pub cid: i32,
    /// 当前P
    pub page: i32,
    /// 视频来源
    pub from: String,
    /// 时间
    pub duration: i32,
    /// 站外ID
    pub vid: String,
    /// 外链
    pub weblink: String,
    /// 分辨率
    pub dimension: Dimension,
}

/// 视频分辨率
#[derive(Debug, Deserialize)]
struct Dimension {
    pub width: i32,
    pub height: i32,
    /// - 0 :正常
    /// - 1 :宽高对换
    pub rotate: u8,
}
