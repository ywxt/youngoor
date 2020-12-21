pub mod bilibili;

use async_trait::async_trait;
use reqwest::Url;

#[async_trait]
pub trait VideoSource {
    fn pretty_name(&self) -> &'static str;
    async fn video_list(&self, url: &Url)
        -> Result<Vec<VideoInfo>, crate::error::VideoSourceError>;
    fn valid(&self, url: &Url) -> bool;

    fn set_token(&mut self, token: String);
    fn get_token(&self) -> Option<&str>;
}

#[derive(Debug)]
pub struct VideoInfo {
    pub pic: Url,
    pub title: String,
    pub video_type: VideoType,
    pub video: Vec<Url>,
    pub audio: Vec<Url>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum VideoType {
    Flv,
    MP4,
}

#[macro_export]
macro_rules! video_sources {
    [$($source:ty),*] => {{
        let mut sources = ::std::vec::Vec::<::std::boxed::Box::<dyn crate::source::VideoSource>>::new();
        $(sources.push(Box::new(<$source as ::std::default::Default>::default()));)*
        sources
    }};
}

#[cfg(test)]
mod test {
    use super::VideoSource;
    use crate::source::VideoInfo;
    use async_trait::async_trait;
    use reqwest::Url;

    #[test]
    fn video_sources_test() {
        #[derive(Default)]
        struct VideoSource1;
        #[async_trait]
        impl VideoSource for VideoSource1 {
            fn pretty_name(&self) -> &'static str {
                "source1"
            }

            async fn video_list(
                &self,
                _: &Url,
            ) -> Result<Vec<VideoInfo>, crate::error::VideoSourceError> {
                unimplemented!()
            }

            fn valid(&self, _: &Url) -> bool {
                unimplemented!()
            }

            fn set_token(&mut self, _token: String) {
                unimplemented!()
            }

            fn get_token(&self) -> Option<&str> {
                unimplemented!()
            }
        }
        #[derive(Default)]
        struct VideoSource2;
        #[async_trait]
        impl VideoSource for VideoSource2 {
            fn pretty_name(&self) -> &'static str {
                "source2"
            }

            async fn video_list(
                &self,
                _: &Url,
            ) -> Result<Vec<VideoInfo>, crate::error::VideoSourceError> {
                unimplemented!()
            }

            fn valid(&self, _: &Url) -> bool {
                unimplemented!()
            }

            fn set_token(&mut self, _token: String) {
                unimplemented!()
            }

            fn get_token(&self) -> Option<&str> {
                unimplemented!()
            }
        }
        let sources = video_sources![VideoSource1, VideoSource2];
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].pretty_name(), "source1");
        assert_eq!(sources[1].pretty_name(), "source2");
    }
}
