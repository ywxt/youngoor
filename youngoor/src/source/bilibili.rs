use super::VideoSource;
use crate::error::VideoSourceError;
use crate::source::VideoInfo;
use async_trait::async_trait;
use reqwest::Url;

#[derive(Debug)]
pub struct BilibiliSource {}

#[async_trait]
impl VideoSource for BilibiliSource {
    fn pretty_name(&self) -> String {
        unimplemented!()
    }

    async fn video_list(&self, url: &Url) -> Result<Vec<VideoInfo>, VideoSourceError> {
        unimplemented!()
    }

    fn valid(&self, url: &Url) -> bool {
        unimplemented!()
    }
}
