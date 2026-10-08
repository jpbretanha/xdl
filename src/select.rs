//! Variant selection: highest quality that fits the size limit.

use crate::source::{Variant, Video};

/// MP4 variants sorted from highest to lowest quality.
pub fn mp4_variants(video: &Video) -> Vec<&Variant> {
    let mut v: Vec<&Variant> = video
        .variants
        .iter()
        .filter(|v| v.content_type == "video/mp4")
        .collect();
    v.sort_by_key(|v| std::cmp::Reverse(v.bitrate.unwrap_or(0)));
    v
}

/// Estimated size in bytes (X's bitrate is a ceiling, so this overestimates).
pub fn estimate_size(variant: &Variant, duration_ms: Option<u64>) -> Option<u64> {
    Some(variant.bitrate? * duration_ms? / 8 / 1000)
}

/// Walks the variants (best → worst) and returns the first one whose size
/// fits in `max_size`. `size_of` returns the actual size (e.g. via HEAD) or
/// `None` to fall back to the estimate. If none fits, returns the smallest.
/// The second value tells whether the limit was respected.
pub fn choose<'a>(
    variants: &[&'a Variant],
    duration_ms: Option<u64>,
    max_size: Option<u64>,
    mut size_of: impl FnMut(&Variant) -> Option<u64>,
) -> Option<(&'a Variant, bool)> {
    let Some(max) = max_size else {
        return variants.first().map(|v| (*v, true));
    };
    for v in variants {
        match size_of(v).or_else(|| estimate_size(v, duration_ms)) {
            Some(size) if size <= max => return Some((v, true)),
            None => return Some((v, true)),
            _ => {}
        }
    }
    variants.last().map(|v| (*v, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{Media, Syndication};

    const SYN: &str = include_str!("../tests/fixtures/syndication_example.json");
    const MB: u64 = 1024 * 1024;

    fn example() -> Video {
        let t = Syndication::parse(SYN).unwrap().unwrap();
        match t.media.into_iter().next().unwrap() {
            Media::Video(v) => v,
            Media::Other => panic!(),
        }
    }

    // Actual sizes (Content-Length) of a 15 s video with these variants.
    fn real_size(v: &Variant) -> Option<u64> {
        if v.url.contains("854x480") {
            Some(1_948_290)
        } else if v.url.contains("640x360") {
            Some(1_154_214)
        } else {
            Some(417_715)
        }
    }

    #[test]
    fn skips_hls_and_sorts() {
        let video = example();
        let v = mp4_variants(&video);
        assert_eq!(v.len(), 3);
        assert!(v[0].url.contains("854x480"));
        assert!(v[2].url.contains("480x270"));
    }

    #[test]
    fn picks_480p_with_default_limit() {
        let video = example();
        let (v, ok) = choose(&mp4_variants(&video), video.duration_ms, Some(100 * MB), real_size).unwrap();
        assert!(v.url.contains("854x480") && ok);
    }

    #[test]
    fn falls_back_to_360p_with_1_5mb_limit() {
        let video = example();
        let (v, ok) = choose(&mp4_variants(&video), video.duration_ms, Some(MB * 3 / 2), real_size).unwrap();
        assert!(v.url.contains("640x360") && ok);
    }

    #[test]
    fn falls_back_to_270p_with_1mb_limit() {
        let video = example();
        let (v, ok) = choose(&mp4_variants(&video), video.duration_ms, Some(MB), real_size).unwrap();
        assert!(v.url.contains("480x270") && ok);
    }

    #[test]
    fn uses_estimate_without_content_length() {
        // 480p estimate ≈ 4.17 MB → doesn't fit in 3 MB, falls back to 360p (~1.6 MB).
        let video = example();
        let (v, _) = choose(&mp4_variants(&video), video.duration_ms, Some(3 * MB), |_| None).unwrap();
        assert!(v.url.contains("640x360"));
    }

    #[test]
    fn none_fits_uses_smallest() {
        let video = example();
        let (v, ok) = choose(&mp4_variants(&video), video.duration_ms, Some(1), real_size).unwrap();
        assert!(v.url.contains("480x270") && !ok);
    }

    #[test]
    fn best_ignores_limit() {
        let video = example();
        let (v, ok) = choose(&mp4_variants(&video), video.duration_ms, None, |_| unreachable!()).unwrap();
        assert!(v.url.contains("854x480") && ok);
    }
}
