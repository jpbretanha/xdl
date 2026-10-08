mod download;
mod link;
mod select;
mod source;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Result, bail};
use clap::Parser;

use source::{FxTwitter, Media, Syndication, Tweet, VideoSource};

const DEFAULT_MAX_SIZE: &str = "100MB";

/// Downloads videos from X (formerly Twitter) posts at the best quality
/// that fits within a size limit.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Post link, e.g. https://x.com/user/status/123/video/1
    url: String,

    /// Output directory (default: ~/Downloads)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Maximum file size, e.g. 50MB, 1.5GB, 800KB
    #[arg(short = 's', long, default_value = DEFAULT_MAX_SIZE, value_parser = parse_size)]
    max_size: u64,

    /// Ignore the size limit and download the highest resolution available
    #[arg(short, long)]
    best: bool,
}

enum Failure {
    Invalid,
    Other(anyhow::Error),
}

impl From<anyhow::Error> for Failure {
    fn from(e: anyhow::Error) -> Self {
        Failure::Other(e)
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Invalid) => {
            eprintln!("Invalid link: {}", cli.url.trim());
            ExitCode::from(1)
        }
        Err(Failure::Other(e)) => {
            eprintln!("Error: {e:#}");
            ExitCode::from(3)
        }
    }
}

fn run(cli: &Cli) -> Result<(), Failure> {
    let link = link::parse(&cli.url).ok_or(Failure::Invalid)?;

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .user_agent(concat!("xdl/", env!("CARGO_PKG_VERSION")))
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_response(Some(Duration::from_secs(20)))
        .build()
        .into();

    let sources: [Box<dyn VideoSource>; 2] = [
        Box::new(Syndication { agent: agent.clone() }),
        Box::new(FxTwitter { agent: agent.clone() }),
    ];
    let tweet = fetch_tweet(&sources, &link)?.ok_or(Failure::Invalid)?;

    let videos: Vec<(usize, &source::Video)> = match link.media_index {
        Some(n) => match tweet.media.get(n - 1) {
            Some(Media::Video(v)) => vec![(n, v)],
            _ => return Err(Failure::Invalid),
        },
        None => tweet
            .media
            .iter()
            .enumerate()
            .filter_map(|(i, m)| match m {
                Media::Video(v) => Some((i + 1, v)),
                Media::Other => None,
            })
            .collect(),
    };
    if videos.is_empty() {
        return Err(Failure::Invalid);
    }

    let dir = cli
        .output
        .clone()
        .or_else(dirs::download_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&dir).map_err(anyhow::Error::from)?;

    let max_size = (!cli.best).then_some(cli.max_size);
    let multiple = videos.len() > 1;

    for (n, video) in videos {
        let variants = select::mp4_variants(video);
        let Some((variant, fits)) =
            select::choose(&variants, video.duration_ms, max_size, |v| download::remote_size(&agent, &v.url))
        else {
            return Err(Failure::Invalid);
        };
        if !fits {
            eprintln!(
                "Warning: no version fits in {}; downloading the smallest available.",
                format_size(cli.max_size)
            );
        }

        let name = if multiple {
            format!("{}_{}_{}.mp4", tweet.user, link.id, n)
        } else {
            format!("{}_{}.mp4", tweet.user, link.id)
        };
        let dest = dir.join(name);
        if dest.exists() {
            println!("Already exists: {}", dest.display());
            continue;
        }

        println!("Downloading {} …", resolution(&variant.url).unwrap_or("video"));
        let bytes = download::download(&agent, &variant.url, &dest)?;
        println!("Saved to {} ({})", dest.display(), format_size(bytes));
    }
    Ok(())
}

/// Tries each source in order; the first one that finds the tweet wins.
fn fetch_tweet(sources: &[Box<dyn VideoSource>], link: &link::TweetLink) -> Result<Option<Tweet>> {
    let mut last_err = None;
    let mut not_found = false;
    for s in sources {
        match s.fetch(link) {
            Ok(Some(t)) => return Ok(Some(t)),
            Ok(None) => not_found = true,
            Err(e) => last_err = Some(e.context(format!("source {}", s.name()))),
        }
    }
    match last_err {
        Some(e) if !not_found => bail!("could not reach X: {e:#}"),
        _ => Ok(None),
    }
}

/// Extracts "854x480" from the video URL, if present.
fn resolution(url: &str) -> Option<&str> {
    url.split('/').find(|p| {
        p.split_once('x')
            .is_some_and(|(w, h)| !w.is_empty() && !h.is_empty() && (w.to_owned() + h).chars().all(|c| c.is_ascii_digit()))
    })
}

fn parse_size(s: &str) -> Result<u64, String> {
    let s = s.trim().to_ascii_uppercase();
    let split = s.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let num: f64 = num.parse().map_err(|_| format!("invalid size: {s}"))?;
    let mult = match unit.trim() {
        "" | "B" => 1u64,
        "K" | "KB" => 1 << 10,
        "M" | "MB" => 1 << 20,
        "G" | "GB" => 1 << 30,
        _ => return Err(format!("invalid unit in {s} (use KB, MB or GB)")),
    };
    Ok((num * mult as f64) as u64)
}

fn format_size(bytes: u64) -> String {
    let mb = bytes as f64 / (1 << 20) as f64;
    if mb >= 1.0 { format!("{mb:.1} MB") } else { format!("{:.0} KB", bytes as f64 / 1024.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(parse_size("100MB"), Ok(100 << 20));
        assert_eq!(parse_size("1.5gb"), Ok((1.5 * (1u64 << 30) as f64) as u64));
        assert_eq!(parse_size("800K"), Ok(800 << 10));
        assert_eq!(parse_size("1234"), Ok(1234));
        assert!(parse_size("abc").is_err());
        assert!(parse_size("10TB").is_err());
    }

    #[test]
    fn resolution_from_url() {
        let u = "https://video.twimg.com/amplify_video/1/vid/avc1/854x480/G3Ua.mp4";
        assert_eq!(resolution(u), Some("854x480"));
        assert_eq!(resolution("https://x/abc.mp4"), None);
    }
}
