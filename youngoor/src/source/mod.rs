use async_trait::async_trait;
use reqwest::Url;

#[async_trait]
pub trait VideoSource {
    fn pretty_name() -> String;
    async fn video_list(url: &Url) -> Vec<VideoInfo>;
    fn valid(url: &Url) -> bool;
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
