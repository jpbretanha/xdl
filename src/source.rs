//! Fontes de metadados de tweets. Cada fonte implementa [`VideoSource`];
//! novas fontes podem ser adicionadas sem alterar o resto do programa.

use anyhow::Result;
use serde::Deserialize;

use crate::link::TweetLink;

#[derive(Debug, Clone, PartialEq)]
pub struct Tweet {
    pub user: String,
    /// Todas as mídias do tweet, na ordem em que aparecem (`/video/{n}` indexa aqui).
    pub media: Vec<Media>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Media {
    Video(Video),
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Video {
    pub duration_ms: Option<u64>,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Variant {
    pub url: String,
    #[serde(default)]
    pub bitrate: Option<u64>,
    pub content_type: String,
}

pub trait VideoSource {
    fn name(&self) -> &'static str;
    /// `Ok(None)` significa que o tweet não existe ou não está acessível.
    fn fetch(&self, link: &TweetLink) -> Result<Option<Tweet>>;
}

/// Faz GET e devolve o corpo em texto, ou `None` em 404.
fn get_text(agent: &ureq::Agent, url: &str) -> Result<Option<String>> {
    match agent.get(url).call() {
        Ok(mut resp) => Ok(Some(resp.body_mut().read_to_string()?)),
        Err(ureq::Error::StatusCode(404)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------
// Syndication (cdn.syndication.twimg.com) — usado pelos embeds oficiais do X.

pub struct Syndication {
    pub agent: ureq::Agent,
}

#[derive(Deserialize)]
struct SynTweet {
    #[serde(rename = "__typename", default)]
    typename: Option<String>,
    user: Option<SynUser>,
    #[serde(rename = "mediaDetails", default)]
    media_details: Vec<SynMedia>,
}

#[derive(Deserialize)]
struct SynUser {
    screen_name: String,
}

#[derive(Deserialize)]
struct SynMedia {
    #[serde(rename = "type")]
    kind: String,
    video_info: Option<SynVideoInfo>,
}

#[derive(Deserialize)]
struct SynVideoInfo {
    duration_millis: Option<u64>,
    #[serde(default)]
    variants: Vec<Variant>,
}

impl Syndication {
    pub fn parse(body: &str) -> Result<Option<Tweet>> {
        let t: SynTweet = serde_json::from_str(body)?;
        if t.typename.as_deref() == Some("TweetTombstone") {
            return Ok(None);
        }
        let Some(user) = t.user else { return Ok(None) };
        let media = t
            .media_details
            .into_iter()
            .map(|m| match (m.kind.as_str(), m.video_info) {
                ("video" | "animated_gif", Some(info)) => Media::Video(Video {
                    duration_ms: info.duration_millis,
                    variants: info.variants,
                }),
                _ => Media::Other,
            })
            .collect();
        Ok(Some(Tweet { user: user.screen_name, media }))
    }
}

/// Token exigido pelo endpoint: `((id / 1e15) * π).toString(36)` sem zeros e sem ponto.
pub fn syndication_token(id: u64) -> String {
    let n = (id as f64 / 1e15) * std::f64::consts::PI;
    let digit = |d: u32| char::from_digit(d, 36).unwrap();

    // 8 dígitos fracionários arredondados, como o JS faz para números dessa magnitude.
    const FRAC_DIGITS: u32 = 8;
    let scale = 36u64.pow(FRAC_DIGITS);
    let mut int = n.trunc() as u64;
    let mut frac = (n.fract() * scale as f64).round() as u64;
    if frac >= scale {
        int += 1;
        frac -= scale;
    }

    let to_base36 = |mut v: u64, min_len: usize| {
        let mut d = Vec::new();
        while v > 0 || d.len() < min_len {
            d.push(digit((v % 36) as u32));
            v /= 36;
        }
        d.into_iter().rev().collect::<String>()
    };
    let out = to_base36(int, 1) + &to_base36(frac, FRAC_DIGITS as usize);
    out.replace('0', "")
}

impl VideoSource for Syndication {
    fn name(&self) -> &'static str {
        "syndication"
    }

    fn fetch(&self, link: &TweetLink) -> Result<Option<Tweet>> {
        let url = format!(
            "https://cdn.syndication.twimg.com/tweet-result?id={}&token={}&lang=en",
            link.id,
            syndication_token(link.id)
        );
        match get_text(&self.agent, &url)? {
            Some(body) => Self::parse(&body),
            None => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// FxTwitter (api.fxtwitter.com) — serviço de terceiros, usado como fallback.

pub struct FxTwitter {
    pub agent: ureq::Agent,
}

#[derive(Deserialize)]
struct FxResponse {
    code: u16,
    tweet: Option<FxTweet>,
}

#[derive(Deserialize)]
struct FxTweet {
    author: SynUser,
    media: Option<FxMediaSet>,
}

#[derive(Deserialize)]
struct FxMediaSet {
    #[serde(default)]
    all: Vec<FxMedia>,
}

#[derive(Deserialize)]
struct FxMedia {
    #[serde(rename = "type")]
    kind: String,
    duration: Option<f64>,
    #[serde(default)]
    variants: Vec<Variant>,
}

impl FxTwitter {
    pub fn parse(body: &str) -> Result<Option<Tweet>> {
        let r: FxResponse = serde_json::from_str(body)?;
        let Some(t) = r.tweet.filter(|_| r.code == 200) else { return Ok(None) };
        let media = t
            .media
            .map(|m| m.all)
            .unwrap_or_default()
            .into_iter()
            .map(|m| match m.kind.as_str() {
                "video" | "gif" => Media::Video(Video {
                    duration_ms: m.duration.map(|d| (d * 1000.0) as u64),
                    variants: m.variants,
                }),
                _ => Media::Other,
            })
            .collect();
        Ok(Some(Tweet { user: t.author.screen_name, media }))
    }
}

impl VideoSource for FxTwitter {
    fn name(&self) -> &'static str {
        "fxtwitter"
    }

    fn fetch(&self, link: &TweetLink) -> Result<Option<Tweet>> {
        let url = format!("https://api.fxtwitter.com/{}/status/{}", link.user, link.id);
        match get_text(&self.agent, &url)? {
            Some(body) => Self::parse(&body),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYN: &str = include_str!("../tests/fixtures/syndication_example.json");
    const FX: &str = include_str!("../tests/fixtures/fxtwitter_example.json");

    fn assert_example(t: Tweet) {
        assert_eq!(t.user, "usuario");
        assert_eq!(t.media.len(), 1);
        let Media::Video(v) = &t.media[0] else { panic!("esperava vídeo") };
        assert_eq!(v.duration_ms, Some(15325));
        let mp4: Vec<_> = v.variants.iter().filter(|v| v.content_type == "video/mp4").collect();
        assert_eq!(mp4.len(), 3);
        assert!(mp4.iter().any(|v| v.url.contains("854x480") && v.bitrate == Some(2176000)));
    }

    #[test]
    fn token_do_exemplo() {
        assert_eq!(syndication_token(1234567890123456789), "2zqic77uqyk");
    }

    #[test]
    fn parse_syndication() {
        assert_example(Syndication::parse(SYN).unwrap().unwrap());
    }

    #[test]
    fn parse_fxtwitter() {
        assert_example(FxTwitter::parse(FX).unwrap().unwrap());
    }

    #[test]
    fn syndication_tombstone_e_sem_midia() {
        assert_eq!(Syndication::parse(r#"{"__typename":"TweetTombstone"}"#).unwrap(), None);
        let t = Syndication::parse(r#"{"__typename":"Tweet","user":{"screen_name":"jack"}}"#)
            .unwrap()
            .unwrap();
        assert!(t.media.is_empty());
    }

    #[test]
    fn fxtwitter_nao_encontrado() {
        assert_eq!(FxTwitter::parse(r#"{"code":404,"message":"NOT_FOUND"}"#).unwrap(), None);
    }
}
