//! X/Twitter link validation.

const HOSTS: &[&str] = &["x.com", "twitter.com", "mobile.twitter.com", "mobile.x.com"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TweetLink {
    pub user: String,
    pub id: u64,
    /// 1-based index from `/video/{n}` or `/photo/{n}`.
    pub media_index: Option<usize>,
}

/// Returns `None` if the input is not a valid tweet link.
pub fn parse(input: &str) -> Option<TweetLink> {
    let s = input.trim();
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    let s = s.split(['?', '#']).next()?;

    let (host, path) = s.split_once('/')?;
    let host = host.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if !HOSTS.contains(&host) {
        return None;
    }

    let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    let (user, id, rest) = match parts.as_slice() {
        [user, "status" | "statuses", id, rest @ ..] => (*user, *id, rest),
        _ => return None,
    };

    let valid_user = !user.is_empty() && user.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid_user || id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let id = id.parse().ok()?;

    let media_index = match rest {
        [] => None,
        ["video" | "photo", n] => match n.parse::<usize>() {
            Ok(n) if n >= 1 => Some(n),
            _ => return None,
        },
        _ => return None,
    };

    Some(TweetLink { user: user.to_string(), id, media_index })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: u64 = 1234567890123456789;

    fn ok(url: &str, idx: Option<usize>) {
        let link = parse(url).unwrap_or_else(|| panic!("should accept: {url}"));
        assert_eq!(link.user, "user");
        assert_eq!(link.id, ID);
        assert_eq!(link.media_index, idx, "{url}");
    }

    #[test]
    fn accepts_example_link() {
        ok("https://x.com/user/status/1234567890123456789/video/1", Some(1));
    }

    #[test]
    fn accepts_variations() {
        ok("https://x.com/user/status/1234567890123456789", None);
        ok("https://x.com/user/status/1234567890123456789/", None);
        ok("https://x.com/user/status/1234567890123456789?s=20", None);
        ok("https://x.com/user/status/1234567890123456789/video/1?s=46&t=abc", Some(1));
        ok("https://twitter.com/user/status/1234567890123456789", None);
        ok("https://www.twitter.com/user/status/1234567890123456789", None);
        ok("https://mobile.twitter.com/user/status/1234567890123456789", None);
        ok("http://WWW.X.COM/user/status/1234567890123456789/photo/2", Some(2));
        ok("x.com/user/status/1234567890123456789", None);
        ok("  https://x.com/user/status/1234567890123456789  ", None);
    }

    #[test]
    fn rejects_invalid() {
        for url in [
            "",
            "not a link",
            "https://youtube.com/user/status/1234567890123456789",
            "https://notx.com/user/status/1234567890123456789",
            "https://x.com/user",
            "https://x.com/user/status/",
            "https://x.com/user/status/abc",
            "https://x.com/user/status/1234567890123456789/video/0",
            "https://x.com/user/status/1234567890123456789/video/x",
            "https://x.com/user/status/1234567890123456789/likes",
            "https://x.com/status/1234567890123456789",
            "https://x.com/user/status/99999999999999999999999",
        ] {
            assert_eq!(parse(url), None, "should reject: {url}");
        }
    }
}
