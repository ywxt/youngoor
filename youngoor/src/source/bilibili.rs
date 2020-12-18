use super::VideoInfo;
use super::VideoSource;
use crate::error::VideoSourceError;
use async_trait::async_trait;
use reqwest::header::COOKIE;
use reqwest::{RequestBuilder, StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

type Result<T> = std::result::Result<T, VideoSourceError>;

const REQUEST_CIDS_URL: &str = "http://api.bilibili.com/x/player/pagelist";

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

    async fn video_list(&self, url: &Url) -> Result<Vec<VideoInfo>> {
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

impl BilibiliSource {
    async fn request_cids(&self, bvid: &str) -> Result<Vec<PInfo>> {
        let url = Url::parse(REQUEST_CIDS_URL).map_err(|_| {
            VideoSourceError::RequestError(format!("无效的地址: {}", REQUEST_CIDS_URL))
        })?;
        self.bilibili_http_get(&url, &[("bvid", bvid)], false)
            .await
            .map(|op| op.unwrap_or_default())
    }
    async fn bilibili_http_get<T>(
        &self,
        url: &Url,
        params: &[(impl AsRef<str>, impl AsRef<str>)],
        with_cookie: bool,
    ) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let mut url = url.clone();
        url.query_pairs_mut().extend_pairs(params);
        let mut request = self.client.get(url.clone());
        request = self.wrap_cookie(request, with_cookie);
        let result = Self::http_request(request).await?;
        Self::wrap_response(result).await
    }
    async fn bilibili_http_post<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        url: &Url,
        body: &B,
        with_cookie: bool,
    ) -> Result<Option<T>> {
        let mut request = self.client.post(url.clone()).json(body);
        request = self.wrap_cookie(request, with_cookie);
        let result = Self::http_request(request).await?;
        Self::wrap_response(result).await
    }
    async fn http_request(request: RequestBuilder) -> Result<reqwest::Response> {
        let response = request.send().await?;
        if response.status() != StatusCode::OK {
            let reason = response
                .status()
                .canonical_reason()
                .unwrap_or("请求错误")
                .to_string();
            return Err(VideoSourceError::RequestError(reason));
        }
        Ok(response)
    }
    fn wrap_cookie(&self, mut request: RequestBuilder, with_cookie: bool) -> RequestBuilder {
        if with_cookie {
            if let Some(cookie) = &self.cookie {
                request = request.header(COOKIE, cookie);
            }
        }
        request
    }
    async fn wrap_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<Option<T>> {
        let url = response.url().to_string();
        let result: Response<T> = response.json().await?;
        match result.code {
            0 => Ok(result.data),
            -400 => Err(VideoSourceError::RequestError(result.message)),
            -404 => Err(VideoSourceError::NoSuchResource(url)),
            _ => Err(VideoSourceError::RequestError(result.message)),
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
    /// 视频标题
    pub part: String,
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

#[cfg(test)]
mod test {
    use super::{BilibiliSource, PInfo, Result, REQUEST_CIDS_URL};
    use crate::error::VideoSourceError;
    use reqwest::Url;

    #[tokio::test]
    async fn bilibili_http_get_test() {
        let bilibili = BilibiliSource::default();
        let url = Url::parse(REQUEST_CIDS_URL).unwrap();
        let result: Vec<PInfo> = bilibili
            .bilibili_http_get(&url, &[("bvid", "BV1ex411J7GE")], false)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(result.len(), 0);
        assert_eq!(result[0].cid, 66445301);
        assert_eq!(result[0].part, "00. 宣传短片");
        assert_eq!(result[0].page, 1);
        assert_eq!(result[1].cid, 35039663);
        assert_eq!(result[1].part, "01. 火柴人与动画师");

        let result: Result<Option<Vec<PInfo>>> = bilibili
            .bilibili_http_get(&url, &[("bvid", "BV1ex411J7G1")], false)
            .await;
        assert!(result.is_err());
        assert!(matches!(result, Err(VideoSourceError::NoSuchResource(_))));

        let result: Result<Option<Vec<PInfo>>> = bilibili
            .bilibili_http_get(&url, &[("bvid", "BVxxxxxx")], false)
            .await;
        assert!(result.is_err());
        assert!(matches!(result, Err(VideoSourceError::RequestError(_))));
    }

    #[tokio::test]
    async fn request_cids_test() {
        let bilibili = BilibiliSource::default();
        let result = bilibili.request_cids("BV1ex411J7G1").await;
        assert!(result.is_err());
        assert!(matches!(result, Err(VideoSourceError::NoSuchResource(_))));

        let result = bilibili.request_cids("BVxxxxxx").await;
        assert!(result.is_err());
        assert!(matches!(result, Err(VideoSourceError::RequestError(_))));

        let result = bilibili.request_cids("BV1ex411J7GE").await.unwrap();
        assert_ne!(result.len(), 0);
        assert_eq!(result[0].cid, 66445301);
        assert_eq!(result[0].part, "00. 宣传短片");
        assert_eq!(result[0].page, 1);
        assert_eq!(result[1].cid, 35039663);
        assert_eq!(result[1].part, "01. 火柴人与动画师");
    }
}
