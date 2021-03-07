pub mod bilibili;

use crate::error::VideoSourceError;
use futures::{future::BoxFuture, stream::BoxStream};
use reqwest::Url;

pub type Result<T> = std::result::Result<T, VideoSourceError>;
pub type VideoInfoStream<'a> = BoxStream<'a, Result<VideoInfo>>;

pub trait VideoSource {
    fn pretty_name(&self) -> &'static str;
    fn video_list(
        &self,
        url: &Url,
        video_type: VideoType,
        dimension: i32,
    ) -> BoxFuture<'_, Result<VideoInfoStream<'_>>>;
    fn valid(&self, url: &Url) -> bool;

    fn set_token(&mut self, token: String);
    fn token(&self) -> Option<&str>;
}

#[derive(Debug)]
pub struct VideoInfo {
    pub pic: Option<Url>,
    pub title: String,
    pub video_type: VideoType,
    pub dimension: i32,
    pub video: Vec<Url>,
    pub audio: Vec<Url>,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
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
    use super::{Result, VideoInfoStream, VideoSource, VideoType};
    use reqwest::Url;
    use futures::future::BoxFuture;

    #[test]
    fn video_sources_test() {
        #[derive(Default)]
        struct VideoSource1;
        impl VideoSource for VideoSource1 {
            fn pretty_name(&self) -> &'static str {
                "source1"
            }

            fn video_list(
                &self,
                url: &Url,
                video_type: VideoType,
                dimension: i32,
            ) -> BoxFuture<'_, Result<VideoInfoStream<'_>>> {
                unimplemented!()
            }

            fn valid(&self, _: &Url) -> bool {
                unimplemented!()
            }

            fn set_token(&mut self, _token: String) {
                unimplemented!()
            }

            fn token(&self) -> Option<&str> {
                unimplemented!()
            }
        }
        #[derive(Default)]
        struct VideoSource2;
        impl VideoSource for VideoSource2 {
            fn pretty_name(&self) -> &'static str {
                "source2"
            }

            fn video_list(
                &self,
                url: &Url,
                video_type: VideoType,
                dimension: i32,
            ) -> BoxFuture<'_, Result<VideoInfoStream<'_>>> {
                unimplemented!()
            }

            fn valid(&self, _: &Url) -> bool {
                unimplemented!()
            }

            fn set_token(&mut self, _token: String) {
                unimplemented!()
            }

            fn token(&self) -> Option<&str> {
                unimplemented!()
            }
        }
        let sources = video_sources![VideoSource1, VideoSource2];
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].pretty_name(), "source1");
        assert_eq!(sources[1].pretty_name(), "source2");
    }
}
