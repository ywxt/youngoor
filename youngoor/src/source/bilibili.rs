use super::VideoInfo;
use super::VideoSource;
use crate::error::VideoSourceError;
use async_trait::async_trait;
use reqwest::header::COOKIE;
use reqwest::{RequestBuilder, StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::export::fmt::Display;
use serde::export::Formatter;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::HashMap;

type Result<T> = std::result::Result<T, VideoSourceError>;

const REQUEST_VIDEO_INFO_URL: &str = "https://api.bilibili.com/x/player/pagelist";
const REQUEST_VIDEO_URL: &str = "https://api.bilibili.com/x/player/playurl";
const REQUEST_SSID_BY_MDID_URL: &str = "https://api.bilibili.com/pgc/review/user";
const REQUEST_BANGUMI_INFO_URL: &str = "https://api.bilibili.com/pgc/view/web/season";

#[derive(Debug)]
pub struct BilibiliSource {
    client: reqwest::Client,
    pub cookie: Option<String>,
}

#[async_trait]
impl VideoSource for BilibiliSource {
    fn pretty_name(&self) -> &'static str {
        "bilibili"
    }

    async fn video_list(&self, _url: &Url) -> Result<Vec<VideoInfo>> {
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

    fn set_token(&mut self, token: String) {
        self.cookie = Some(token);
    }

    fn get_token(&self) -> Option<&str> {
        self.cookie.as_deref()
    }
}

impl BilibiliSource {
    async fn request_video_info(&self, bvid: &str) -> Result<Vec<PInfo>> {
        let url = Self::parse_url(REQUEST_VIDEO_INFO_URL)?;
        self.bilibili_http_get_not_null(&url, [("bvid", bvid)].iter(), self.cookie.is_some())
            .await
    }
    /// 请求剧集ssid
    async fn request_bangumi_ssid(&self, media_id: i32) -> Result<i32> {
        let query_param = [("media_id", media_id.to_string())];
        let url = Self::parse_url(REQUEST_SSID_BY_MDID_URL)?;
        let result: BangumiInfo = self
            .bilibili_http_get_not_null(&url, query_param.iter(), self.cookie.is_some())
            .await?;
        Ok(result.media.season_id)
    }
    async fn request_bangumi_info(&self, ssid: i32) -> Result<Vec<Episode>> {
        let url = Self::parse_url(REQUEST_BANGUMI_INFO_URL)?;
        let query_param = [("season_id", ssid.to_string())];
        let result: EpisodesInfo = self
            .bilibili_http_get_not_null(&url, query_param.iter(), self.cookie.is_some())
            .await?;
        Ok(result.episodes)
    }
    /// 返回`Result<(视频, 音频)>`
    async fn request_video_url(
        &self,
        bvid: &str,
        cid: i32,
        vide_type: VideoTypeCode,
        dimension: DimensionCode,
    ) -> Result<(Vec<Url>, Vec<Url>)> {
        let query_params: HashMap<_, _> = VideoUrlRequest {
            bvid: bvid.to_string(),
            cid,
            fnver: 0,
            fnval: vide_type,
            qn: dimension,
            fourk: 1,
        }
        .into();
        let url = Self::parse_url(REQUEST_VIDEO_URL)?;
        let result: VideoUrlInfo = self
            .bilibili_http_get_not_null(&url, query_params.iter(), dimension.need_login())
            .await?;
        if let Some(flv) = result.durl {
            let video_url: Result<_> = flv
                .into_iter()
                .map(|durl| Self::parse_url(&durl.url))
                .collect();
            return Ok((video_url?, vec![]));
        }
        if let Some(dash) = result.dash {
            let video_url = dash
                .video
                .into_iter()
                .filter_map(|video| {
                    if video.id == (dimension as i32) {
                        Some(video.base_url)
                    } else {
                        None
                    }
                })
                .next()
                .ok_or_else(|| VideoSourceError::NoSuchResource(format!("bvid={}", bvid)))?;
            let audio_url = dash
                .audio
                .into_iter()
                .next()
                .ok_or_else(|| VideoSourceError::NoSuchResource(format!("bvid={}", bvid)))?
                .base_url;
            return Ok((
                vec![Self::parse_url(&video_url)?],
                vec![Self::parse_url(&audio_url)?],
            ));
        }
        Err(VideoSourceError::NoSuchResource(format!("bvid={}", bvid)))
    }

    async fn bilibili_http_get_not_null<T, I, K, V>(
        &self,
        url: &Url,
        params: I,
        with_cookie: bool,
    ) -> Result<T>
    where
        T: DeserializeOwned,
        I: Iterator,
        I::Item: Borrow<(K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let response = self.bilibili_http_get(url, params, with_cookie).await?;
        Self::wrap_response_not_null(response).await
    }
    async fn bilibili_http_post_not_null<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        url: &Url,
        body: &B,
        with_cookie: bool,
    ) -> Result<T> {
        let response = self.bilibili_http_post(url, body, with_cookie).await?;
        Self::wrap_response_not_null(response).await
    }

    async fn bilibili_http_get<I, K, V>(
        &self,
        url: &Url,
        params: I,
        with_cookie: bool,
    ) -> Result<reqwest::Response>
    where
        I: Iterator,
        I::Item: Borrow<(K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut url = url.clone();
        url.query_pairs_mut().extend_pairs(params);
        let mut request = self.client.get(url.clone());
        request = self.wrap_cookie(request, with_cookie)?;
        Self::http_request(request).await
    }
    async fn bilibili_http_post<B: Serialize + ?Sized>(
        &self,
        url: &Url,
        body: &B,
        with_cookie: bool,
    ) -> Result<reqwest::Response> {
        let mut request = self.client.post(url.clone()).json(body);
        request = self.wrap_cookie(request, with_cookie)?;
        Self::http_request(request).await
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
    fn wrap_cookie(&self, request: RequestBuilder, with_cookie: bool) -> Result<RequestBuilder> {
        if with_cookie {
            if let Some(cookie) = &self.cookie {
                Ok(request.header(COOKIE, cookie))
            } else {
                Err(VideoSourceError::NeedLogin)
            }
        } else {
            Ok(request)
        }
    }
    async fn wrap_response_null<T: DeserializeOwned>(
        response: reqwest::Response,
    ) -> Result<Option<T>> {
        let url = response.url().to_string();
        let result: Response<T> = response.json().await?;
        // assert!(result.data.is_some());
        match result.code {
            0 => Ok(result.data),
            -400 => Err(VideoSourceError::RequestError(result.message)),
            -404 => Err(VideoSourceError::NoSuchResource(url)),
            _ => Err(VideoSourceError::RequestError(result.message)),
        }
    }
    async fn wrap_response_not_null<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
        let url = response.url().clone().to_string();
        let result = Self::wrap_response_null(response).await?;
        //assert!(result.is_some());
        result.ok_or(VideoSourceError::NoSuchResource(url))
    }
    fn parse_url(url: &str) -> Result<Url> {
        Url::parse(url).map_err(|_| VideoSourceError::RequestError(format!("无效的地址: {}", url)))
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
    // ttl: i32,
    #[serde(alias = "data", alias = "result")]
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

/// 获取下载地址时的分辨率
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum DimensionCode {
    P240 = 6,
    P360 = 16,
    P480 = 32,
    P720 = 64,
    P720F60 = 74,
    P1080 = 80,
    P1080P = 112,
    P1080F60 = 116,
    P4K = 120,
}

impl DimensionCode {
    pub fn need_login(&self) -> bool {
        !matches!(
            self,
            DimensionCode::P240 | DimensionCode::P360 | DimensionCode::P480
        )
    }
}

impl Display for DimensionCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DimensionCode::P240 => f.write_str("240P 极速"),
            DimensionCode::P360 => f.write_str("360P 流畅"),
            DimensionCode::P480 => f.write_str("480P 清晰"),
            DimensionCode::P720 => f.write_str("720P 高清（登录）"),
            DimensionCode::P720F60 => f.write_str("720P60 高清（大会员）"),
            DimensionCode::P1080 => f.write_str("1080P 高清（登录）"),
            DimensionCode::P1080P => f.write_str("1080P+ 高清（大会员）"),
            DimensionCode::P1080F60 => f.write_str("1080P60 高清（大会员）"),
            DimensionCode::P4K => f.write_str("4K 超清（大会员）"),
        }
    }
}

/// 获取下载地址时的格式
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum VideoTypeCode {
    Flv1 = 0,
    Mp4 = 1,
    Flv2 = 2,
    Dash = 16,
}

impl Display for VideoTypeCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoTypeCode::Flv1 => f.write_str("FLV"),
            VideoTypeCode::Mp4 => f.write_str("MP4"),
            VideoTypeCode::Flv2 => f.write_str("FLV"),
            VideoTypeCode::Dash => f.write_str("高清MP4"),
        }
    }
}

/// 视频下载请求
#[derive(Debug)]
struct VideoUrlRequest {
    pub bvid: String,
    pub cid: i32,
    /// 分辨率
    pub qn: DimensionCode,
    /// 格式
    pub fnval: VideoTypeCode,
    /// 固定为0
    pub fnver: i32,
    /// 是否允许4K
    pub fourk: i32,
}

impl From<VideoUrlRequest> for HashMap<&'static str, String> {
    fn from(data: VideoUrlRequest) -> Self {
        let mut map = HashMap::with_capacity(6);
        map.insert("bvid", data.bvid);
        map.insert("cid", data.cid.to_string());
        map.insert("qn", (data.qn as u8).to_string());
        map.insert("fnval", (data.fnval as u8).to_string());
        map.insert("fnver", data.fnver.to_string());
        map.insert("fourk", data.fourk.to_string());
        map
    }
}

#[derive(Debug, Deserialize)]
struct VideoUrlInfo {
    from: String,
    result: String,
    /// 分辨率
    pub quality: i32,
    /// 视频格式
    pub format: String,
    /// 视频长度
    #[serde(rename(deserialize = "timelength"))]
    pub time_length: i32,
    /// 视频支持的全部格式
    pub accept_format: String,
    /// 视频支持的分辨率列表
    pub accept_description: Vec<String>,
    /// 视频支持的分辨率代码列表
    pub accept_quality: Vec<i32>,
    video_codecid: i32,
    seek_param: String,
    seek_type: String,
    /// 视频分段
    pub durl: Option<Vec<Durl>>,
    /// dash音视频流信息
    pub dash: Option<Dash>,
}

/// MP4,FLV格式返回
#[derive(Debug, Deserialize)]
struct Durl {
    /// 序号
    pub order: i32,
    /// 时间
    pub length: i32,
    /// 字节大小
    pub size: i32,
    ahead: String,
    vhead: String,
    /// 地址，存在转义
    pub url: String,
    /// 备用地址，存在转义
    pub backup_url: Vec<String>,
}

/// Dash 格式返回
#[derive(Debug, Deserialize)]
struct Dash {
    duration: i32,
    min_buffer_time: f32,
    pub video: Vec<DashItem>,
    pub audio: Vec<DashItem>,
}

#[derive(Debug, Deserialize)]
struct DashItem {
    /// 音视频清晰度
    pub id: i32,
    /// 下载地址
    pub base_url: String,
    /// 备用地址
    pub backup_url: Vec<String>,
    /// 所需带宽
    #[serde(rename(deserialize = "bandwidth"))]
    band_width: i32,
    /// 媒体类型
    mime_type: String,
    /// 编码/音频类型
    codecs: String,
    /// 视频宽度
    width: i32,
    /// 视频高度
    height: i32,
    /// 视频帧率
    frame_rate: String,
    sar: String,
    start_with_sap: i32,
    segment_base: SegmentBase,
    codecid: i32,
}

#[derive(Deserialize, Debug)]
struct SegmentBase {
    initialization: String,
    index_range: String,
}

#[derive(Debug, Deserialize)]
struct BangumiInfo {
    pub media: MediaInfo,
}

/// 剧集基本信息（mdID方式）
#[derive(Debug, Deserialize)]
struct MediaInfo {
    pub cover: String,
    pub media_id: i32,
    pub season_id: i32,
    pub title: String,
}

/// 具体分集信息
#[derive(Debug, Deserialize)]
struct EpisodesInfo {
    /// 分集
    pub episodes: Vec<Episode>,
    /// 简介
    pub evaluate: String,

    pub media_id: i32,
    pub season_id: i32,
    pub title: String,
}

/// 分集
#[derive(Debug, Deserialize)]
struct Episode {
    pub bvid: String,
    pub cid: i32,
    /// 封面
    pub cover: String,
    /// 单集epid
    pub id: i32,
    /// 单集完整标题
    pub long_title: String,
    /// 单集标题
    pub title: String,
}

#[cfg(test)]
mod test {
    use super::{BilibiliSource, REQUEST_VIDEO_INFO_URL};
    use crate::error::VideoSourceError;
    use crate::source::bilibili::{DimensionCode, VideoTypeCode};
    use crate::source::VideoSource;
    use reqwest::{StatusCode, Url};

    #[tokio::test]
    async fn bilibili_http_get_test() {
        let bilibili = BilibiliSource::default();
        let url = Url::parse(REQUEST_VIDEO_INFO_URL).unwrap();
        let result = bilibili
            .bilibili_http_get(&url, [("bvid", "BV1ex411J7GE")].iter(), false)
            .await
            .unwrap();
        assert_eq!(result.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn request_cids_test() {
        let bilibili = BilibiliSource::default();
        let result = bilibili.request_video_info("BV1ex411J7G1").await;
        assert!(result.is_err());
        assert!(matches!(result, Err(VideoSourceError::NoSuchResource(_))));

        let result = bilibili.request_video_info("BVxxxxxx").await;
        assert!(result.is_err());
        assert!(matches!(result, Err(VideoSourceError::RequestError(_))));

        let result = bilibili.request_video_info("BV1ex411J7GE").await.unwrap();
        assert_ne!(result.len(), 0);
        assert_eq!(result[0].cid, 66445301);
        assert_eq!(result[0].part, "00. 宣传短片");
        assert_eq!(result[0].page, 1);
        assert_eq!(result[1].cid, 35039663);
        assert_eq!(result[1].part, "01. 火柴人与动画师");
    }

    #[tokio::test]
    async fn request_video_url_test() {
        let mut bilibili = BilibiliSource::default();
        let (video, audio) = bilibili
            .request_video_url(
                "BV1y7411Q7Eq",
                171776208,
                VideoTypeCode::Flv1,
                DimensionCode::P480,
            )
            .await
            .unwrap();
        assert!(audio.is_empty());
        assert_eq!(video.len(), 1);
        assert!(video[0].host_str().unwrap().ends_with("bilivideo.com"));

        assert!(matches!(
            bilibili
                .request_video_url(
                    "BV1y7411Q7Eq",
                    171776208,
                    VideoTypeCode::Flv1,
                    DimensionCode::P1080,
                )
                .await,
            Err(VideoSourceError::NeedLogin)
        ));
        bilibili.set_token(std::env::var("BILIBILI_COOKIE").unwrap());
        let (video, audio) = bilibili
            .request_video_url(
                "BV1y7411Q7Eq",
                171776208,
                VideoTypeCode::Flv1,
                DimensionCode::P1080,
            )
            .await
            .unwrap();
        assert!(audio.is_empty());
        assert_eq!(video.len(), 1);
        assert!(video[0].host_str().unwrap().ends_with("bilivideo.com"));

        // 无大会员时 返回可用的最高画质
        let (video, audio) = bilibili
            .request_video_url(
                "BV1y7411Q7Eq",
                171776208,
                VideoTypeCode::Flv1,
                DimensionCode::P4K,
            )
            .await
            .unwrap();
        assert!(audio.is_empty());
        assert_eq!(video.len(), 1);
        let video = video[0].to_string();
        assert!(video.contains("bilivideo.com"));

        let (video, audio) = bilibili
            .request_video_url(
                "BV1y7411Q7Eq",
                171776208,
                VideoTypeCode::Dash,
                DimensionCode::P1080,
            )
            .await
            .unwrap();
        assert_eq!(audio.len(), 1);
        assert!(audio[0].to_string().contains("bilivideo.com"));
        assert_eq!(video.len(), 1);
        let video = video[0].to_string();
        assert!(
            video.contains("bilivideo.com")
                && (video.contains("30080.m4s") || video.contains("30077.m4s"))
        );
    }

    #[tokio::test]
    async fn request_bangumi_ssid_test() {
        let bilibili = BilibiliSource::default();
        //bilibili.set_token(std::env::var("BILIBILI_COOKIE").unwrap());
        assert_eq!(bilibili.request_bangumi_ssid(5978).await.unwrap(), 5978);
        assert_eq!(
            bilibili.request_bangumi_ssid(28229053).await.unwrap(),
            33624,
        );
    }

    #[tokio::test]
    async fn request_bangumi_info_test() {
        let bilibili = BilibiliSource::default();
        let result = bilibili.request_bangumi_info(33624).await.unwrap();
        assert_eq!(result.len(), 36);
        assert_eq!(result[0].cid, 200063835);
        assert_eq!(result[0].long_title, "林黛玉别父进京都");

        let result = bilibili.request_bangumi_info(5978).await.unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].cid, 15915981);
        assert_eq!(result[0].long_title, "漩涡博人");
    }
}
