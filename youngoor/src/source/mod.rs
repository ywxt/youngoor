use async_trait::async_trait;
use reqwest::Url;

#[async_trait]
pub trait VideoSource {
    fn pretty_name(&self) -> String;
    async fn video_list(&self, url: &Url)
        -> Result<Vec<VideoInfo>, crate::error::VideoSourceError>;
    fn valid(&self, url: &Url) -> bool;
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
            fn pretty_name(&self) -> String {
                "source1".to_string()
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
        }
        #[derive(Default)]
        struct VideoSource2;
        #[async_trait]
        impl VideoSource for VideoSource2 {
            fn pretty_name(&self) -> String {
                "source2".to_string()
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
        }
        let sources = video_sources![VideoSource1, VideoSource2];
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].pretty_name(), "source1");
        assert_eq!(sources[1].pretty_name(), "source2");
    }
}
