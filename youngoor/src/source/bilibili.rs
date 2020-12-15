use super::VideoInfo;
use super::VideoSource;
use crate::error::VideoSourceError;
use async_trait::async_trait;
use reqwest::Url;
use serde::Deserialize;
use std::option::Option::Some;

#[derive(Debug)]
pub struct BilibiliSource(reqwest::Client);

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
        BilibiliSource(reqwest::Client::new())
    }
}

#[derive(Debug, Deserialize)]
struct Response<T> {
    pub code: i32,
    pub message: String,
    ttl: i32,
    pub data: T,
}
