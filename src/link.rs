//! Validação de links do X/Twitter.

const HOSTS: &[&str] = &["x.com", "twitter.com", "mobile.twitter.com", "mobile.x.com"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TweetLink {
    pub user: String,
    pub id: u64,
    /// Índice (base 1) vindo de `/video/{n}` ou `/photo/{n}`.
    pub media_index: Option<usize>,
}

/// Retorna `None` se o texto não for um link de tweet válido.
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
        let link = parse(url).unwrap_or_else(|| panic!("deveria aceitar: {url}"));
        assert_eq!(link.user, "usuario");
        assert_eq!(link.id, ID);
        assert_eq!(link.media_index, idx, "{url}");
    }

    #[test]
    fn aceita_link_de_exemplo() {
        ok("https://x.com/usuario/status/1234567890123456789/video/1", Some(1));
    }

    #[test]
    fn aceita_variacoes() {
        ok("https://x.com/usuario/status/1234567890123456789", None);
        ok("https://x.com/usuario/status/1234567890123456789/", None);
        ok("https://x.com/usuario/status/1234567890123456789?s=20", None);
        ok("https://x.com/usuario/status/1234567890123456789/video/1?s=46&t=abc", Some(1));
        ok("https://twitter.com/usuario/status/1234567890123456789", None);
        ok("https://www.twitter.com/usuario/status/1234567890123456789", None);
        ok("https://mobile.twitter.com/usuario/status/1234567890123456789", None);
        ok("http://WWW.X.COM/usuario/status/1234567890123456789/photo/2", Some(2));
        ok("x.com/usuario/status/1234567890123456789", None);
        ok("  https://x.com/usuario/status/1234567890123456789  ", None);
    }

    #[test]
    fn rejeita_invalidos() {
        for url in [
            "",
            "não é um link",
            "https://youtube.com/usuario/status/1234567890123456789",
            "https://notx.com/usuario/status/1234567890123456789",
            "https://x.com/usuario",
            "https://x.com/usuario/status/",
            "https://x.com/usuario/status/abc",
            "https://x.com/usuario/status/1234567890123456789/video/0",
            "https://x.com/usuario/status/1234567890123456789/video/x",
            "https://x.com/usuario/status/1234567890123456789/likes",
            "https://x.com/status/1234567890123456789",
            "https://x.com/usuario/status/99999999999999999999999",
        ] {
            assert_eq!(parse(url), None, "deveria rejeitar: {url}");
        }
    }
}
