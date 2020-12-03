use async_trait::async_trait;
use reqwest::Url;

#[async_trait]
pub trait VideoSource {
    async fn video_list(url: &Url) -> Vec<VideoInfo>;
    fn host() -> &'static str;
}

#[derive(Debug)]
pub struct VideoInfo {
    pub pic: Url,
    pub title: String,
    pub video_type: VideoType,
    pub videos: Vec<Url>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum VideoType {
    Flv,
    MP4,
}
