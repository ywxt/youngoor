use super::{Result, VideoInfo, VideoInfoStream, VideoSource, VideoType};
use crate::error::VideoSourceError;

use reqwest::{header::COOKIE, RequestBuilder, StatusCode, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::{HashMap, VecDeque};
use std::fmt::{Display, Formatter};

const REQUEST_VIDEO_INFO_URL: &str = "https://api.bilibili.com/x/player/pagelist";
const REQUEST_VIDEO_URL: &str = "https://api.bilibili.com/x/player/playurl";
const REQUEST_SSID_BY_MDID_URL: &str = "https://api.bilibili.com/pgc/review/user";
const REQUEST_BANGUMI_INFO_URL: &str = "https://api.bilibili.com/pgc/view/web/season";

#[derive(Clone, Debug, Default)]
struct BilibiliClient {
    client: reqwest::Client,
    cookie: Option<String>,
}

#[derive(Debug, Default)]
pub struct BilibiliSource(BilibiliClient);

#[derive(Debug, Eq, PartialEq)]
enum UrlType {
    /// BV ID
    Video(String),
    /// media id
    Bangumi(i32),
}

impl VideoSource for BilibiliSource {
    fn pretty_name(&self) -> &'static str {
        "bilibili"
    }

    fn video_list(
        &self,
        url: &Url,
        video_type: VideoType,
        dimension: i32,
    ) -> Result<VideoInfoStream<'_>> {
        use async_stream::try_stream;

        match Self::url_type(&url) {
            Some(UrlType::Bangumi(media_id)) => Ok(Box::pin(try_stream! {
              let ssid = self.0.request_bangumi_ssid(media_id).await?;
              let episodes = self.0.request_bangumi_info(ssid).await?;
              let play_list: VecDeque<BilibiliSourceItem> = episodes
              .into_iter()
              .map(|episode| {
                  Ok::<_,VideoSourceError>(BilibiliSourceItem {
                      bvid: episode.bvid.clone(),
                      cid: episode.cid,
                      pic: Some(Url::parse(&episode.cover).map_err(|_| {
                          VideoSourceError::InvalidApiData(format!(
                              "视频地址错误: bvid={},cid={}",
                              episode.bvid, episode.cid
                          ))
                      })?),
                      title: format!("{} {}",episode.title, episode.long_title),
                      video_type,
                  })
              })
              .collect()?;
              for item in play_list {
                  let urls =  self.0.request_video_url(&item.bvid,item.cid,video_type.into(),dimension.into()).await?;
                  yield VideoInfo {
                      title: item.title,
                      pic: item.pic,
                      video: urls.0,
                      audio: urls.1,
                  }
              }
            })),
            Some(UrlType::Video(bvid)) => Ok(Box::pin(try_stream! {
              let videos = self.0.request_video_info(&bvid).await?;
              let play_list: VecDeque<BilibiliSourceItem> = videos
                .into_iter()
                .map(|p_info| BilibiliSourceItem {
                     bvid: bvid.clone(),
                     cid: p_info.cid,
                     pic: None,
                     title: p_info.part,
                     video_type,
                })
                .collect();
              for item in play_list {
                 let urls =  self.0.request_video_url(&item.bvid,item.cid,video_type.into(),dimension.into()).await?;
                 yield VideoInfo {
                     title: item.title,
                     pic: item.pic,
                     video: urls.0,
                     audio: urls.1,
                 }
             }
            })),
            None => Err(VideoSourceError::InvalidUrl(url.to_owned())),
        }
    }
    fn valid(&self, url: &Url) -> bool {
        Self::url_type(url).is_some()
    }

    fn set_token(&mut self, token: String) {
        self.0.set_token(token)
    }

    fn token(&self) -> Option<&str> {
        self.0.token()
    }

    fn dimension(&self) -> Vec<(i32, String)> {
        vec![
            (
                DimensionCode::P240.into(),
                format!("{}", DimensionCode::P240),
            ),
            (
                DimensionCode::P480.into(),
                format!("{}", DimensionCode::P480),
            ),
            (
                DimensionCode::P720.into(),
                format!("{}", DimensionCode::P720),
            ),
            (
                DimensionCode::P720F60.into(),
                format!("{}", DimensionCode::P720F60),
            ),
            (
                DimensionCode::P1080.into(),
                format!("{}", DimensionCode::P1080),
            ),
            (
                DimensionCode::P1080P.into(),
                format!("{}", DimensionCode::P1080P),
            ),
            (
                DimensionCode::P1080F60.into(),
                format!("{}", DimensionCode::P1080F60),
            ),
            (DimensionCode::P4K.into(), format!("{}", DimensionCode::P4K)),
        ]
    }
}

impl BilibiliClient {
    fn set_token(&mut self, token: String) {
        self.cookie = Some(token);
    }

    fn token(&self) -> Option<&str> {
        self.cookie.as_deref()
    }
    async fn request_video_info(&self, bvid: &str) -> Result<Vec<PInfo>> {
        let url = BilibiliSource::parse_url(REQUEST_VIDEO_INFO_URL)?;
        self.bilibili_http_get_not_null(&url, [("bvid", bvid)].iter(), self.cookie.is_some())
            .await
    }
    /// 请求剧集ssid
    async fn request_bangumi_ssid(&self, media_id: i32) -> Result<i32> {
        let query_param = [("media_id", media_id.to_string())];
        let url = BilibiliSource::parse_url(REQUEST_SSID_BY_MDID_URL)?;
        let result: BangumiInfo = self
            .bilibili_http_get_not_null(&url, query_param.iter(), self.cookie.is_some())
            .await?;
        Ok(result.media.season_id)
    }
    async fn request_bangumi_info(&self, ssid: i32) -> Result<Vec<Episode>> {
        let url = BilibiliSource::parse_url(REQUEST_BANGUMI_INFO_URL)?;
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
        let url = BilibiliSource::parse_url(REQUEST_VIDEO_URL)?;
        let result: VideoUrlInfo = self
            .bilibili_http_get_not_null(&url, query_params.iter(), dimension.need_login())
            .await?;
        if let Some(flv) = result.durl {
            let video_url: Result<_> = flv
                .into_iter()
                .map(|durl| BilibiliSource::parse_url(&durl.url))
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
                vec![BilibiliSource::parse_url(&video_url)?],
                vec![BilibiliSource::parse_url(&audio_url)?],
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
}
impl BilibiliSource {
    pub fn new() -> Self {
        Self::default()
    }

    fn parse_url(url: &str) -> Result<Url> {
        Url::parse(url).map_err(|_| VideoSourceError::RequestError(format!("无效的地址: {}", url)))
    }

    fn url_type(url: &Url) -> Option<UrlType> {
        let host = url.host_str()?;
        let is_host = host.eq_ignore_ascii_case("www.bilibili.com")
            || host.eq_ignore_ascii_case("bilibili.com");
        if !is_host {
            return None;
        }
        let mut path = url.path_segments()?;
        match path.next() {
            Some("video") => {
                let bvid = path.next()?;
                if bvid.starts_with("BV") {
                    Some(UrlType::Video(bvid.to_string()))
                } else {
                    None
                }
            }

            Some("bangumi") => match path.next() {
                Some("media") => {
                    let media_id = path.next()?;
                    if media_id.starts_with("md") {
                        let id: std::result::Result<i32, _> =
                            media_id.strip_prefix("md").unwrap_or("no id").parse();
                        id.ok().map(UrlType::Bangumi)
                    } else {
                        None
                    }
                }
                _ => None,
            },
            _ => None,
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

impl From<i32> for DimensionCode {
    fn from(dimension: i32) -> Self {
        match dimension {
            6 => Self::P240,
            16 => Self::P360,
            32 => Self::P480,
            64 => Self::P720,
            74 => Self::P720F60,
            80 => Self::P1080,
            112 => Self::P1080P,
            116 => Self::P1080F60,
            120 => Self::P4K,
            _ => Self::P720,
        }
    }
}

impl From<DimensionCode> for i32 {
    fn from(dimension: DimensionCode) -> Self {
        dimension as Self
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

impl From<VideoTypeCode> for VideoType {
    fn from(video_type: VideoTypeCode) -> Self {
        match video_type {
            VideoTypeCode::Flv1 => Self::Flv,
            VideoTypeCode::Mp4 => Self::MP4,
            VideoTypeCode::Flv2 => Self::Flv,
            VideoTypeCode::Dash => Self::MP4,
        }
    }
}
impl From<VideoType> for VideoTypeCode {
    fn from(video_type: VideoType) -> Self {
        match video_type {
            VideoType::Flv => Self::Flv2,
            VideoType::MP4 => Self::Dash,
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
#[derive(Debug, Clone)]
pub struct BilibiliSourceItem {
    pub bvid: String,
    pub cid: i32,
    pub pic: Option<Url>,
    pub title: String,
    pub video_type: VideoType,
}

#[cfg(test)]
mod test {
    use super::{
        super::{VideoSource, VideoType},
        BilibiliClient, BilibiliSource, DimensionCode, UrlType, VideoTypeCode,
        REQUEST_VIDEO_INFO_URL,
    };
    use crate::error::VideoSourceError;
    use futures::StreamExt;
    use reqwest::{StatusCode, Url};
    use std::convert::TryInto;

    #[tokio::test]
    async fn bilibili_http_get_test() {
        let bilibili = BilibiliClient::default();
        let url = Url::parse(REQUEST_VIDEO_INFO_URL).unwrap();
        let result = bilibili
            .bilibili_http_get(&url, [("bvid", "BV1ex411J7GE")].iter(), false)
            .await
            .unwrap();
        assert_eq!(result.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn request_cids_test() {
        let bilibili = BilibiliClient::default();
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
        let mut bilibili = BilibiliClient::default();
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
        let bilibili = BilibiliClient::default();
        //bilibili.set_token(std::env::var("BILIBILI_COOKIE").unwrap());
        assert_eq!(bilibili.request_bangumi_ssid(5978).await.unwrap(), 5978);
        assert_eq!(
            bilibili.request_bangumi_ssid(28229053).await.unwrap(),
            33624,
        );
    }

    #[tokio::test]
    async fn request_bangumi_info_test() {
        let bilibili = BilibiliClient::default();
        let result = bilibili.request_bangumi_info(33624).await.unwrap();
        assert_eq!(result.len(), 36);
        assert_eq!(result[0].cid, 200063835);
        assert_eq!(result[0].long_title, "林黛玉别父进京都");

        let result = bilibili.request_bangumi_info(5978).await.unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].cid, 15915981);
        assert_eq!(result[0].long_title, "漩涡博人");
    }

    #[test]
    fn url_type_test() {
        assert_eq!(
            BilibiliSource::url_type(
                &"https://www.bilibili.com/video/BVXXXXXX"
                    .try_into()
                    .unwrap()
            ),
            Some(UrlType::Video("BVXXXXXX".to_string()))
        );
        assert_eq!(
            BilibiliSource::url_type(
                &"https://www.bilibili.com/bangumi/media/md28229053"
                    .parse()
                    .unwrap()
            ),
            Some(UrlType::Bangumi(28229053))
        );
        assert_eq!(
            BilibiliSource::url_type(
                &"https://www.bilibili.com/bangumi/play/ep327884"
                    .parse()
                    .unwrap()
            ),
            None
        );
    }
    #[tokio::test]
    async fn bilibili_source_video_type_test() {
        let source = BilibiliSource::default();
        let mut videos_info = source
            .video_list(
                &Url::parse("https://www.bilibili.com/bangumi/media/md28229053").unwrap(),
                VideoType::MP4,
                32,
            )
            .unwrap();
        while let Some(video) = videos_info.next().await {
            let video = video.unwrap();
            println!("{:?}", video);
        }
    }
}
