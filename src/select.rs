//! Escolha da variante: maior qualidade que caiba no limite de tamanho.

use crate::source::{Variant, Video};

/// Variantes MP4 ordenadas da maior para a menor qualidade.
pub fn mp4_variants(video: &Video) -> Vec<&Variant> {
    let mut v: Vec<&Variant> = video
        .variants
        .iter()
        .filter(|v| v.content_type == "video/mp4")
        .collect();
    v.sort_by_key(|v| std::cmp::Reverse(v.bitrate.unwrap_or(0)));
    v
}

/// Tamanho estimado em bytes (o bitrate do X é um teto, então superestima).
pub fn estimate_size(variant: &Variant, duration_ms: Option<u64>) -> Option<u64> {
    Some(variant.bitrate? * duration_ms? / 8 / 1000)
}

/// Percorre as variantes (melhor → pior) e devolve a primeira cujo tamanho
/// caiba em `max_size`. `size_of` devolve o tamanho real (ex.: via HEAD) ou
/// `None` para cair na estimativa. Se nenhuma couber, devolve a menor.
/// O segundo valor indica se o limite foi respeitado.
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

    // Tamanhos reais (Content-Length) de um vídeo de 15 s com essas variantes.
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
    fn ignora_hls_e_ordena() {
        let video = example();
        let v = mp4_variants(&video);
        assert_eq!(v.len(), 3);
        assert!(v[0].url.contains("854x480"));
        assert!(v[2].url.contains("480x270"));
    }

    #[test]
    fn escolhe_480p_com_limite_padrao() {
        let video = example();
        let (v, ok) = choose(&mp4_variants(&video), video.duration_ms, Some(100 * MB), real_size).unwrap();
        assert!(v.url.contains("854x480") && ok);
    }

    #[test]
    fn desce_para_360p_com_limite_de_1_5mb() {
        let video = example();
        let (v, ok) = choose(&mp4_variants(&video), video.duration_ms, Some(MB * 3 / 2), real_size).unwrap();
        assert!(v.url.contains("640x360") && ok);
    }

    #[test]
    fn desce_para_270p_com_limite_de_1mb() {
        let video = example();
        let (v, ok) = choose(&mp4_variants(&video), video.duration_ms, Some(MB), real_size).unwrap();
        assert!(v.url.contains("480x270") && ok);
    }

    #[test]
    fn usa_estimativa_sem_content_length() {
        // Estimativa da 480p ≈ 4,17 MB → não cabe em 3 MB, cai para 360p (~1,6 MB).
        let video = example();
        let (v, _) = choose(&mp4_variants(&video), video.duration_ms, Some(3 * MB), |_| None).unwrap();
        assert!(v.url.contains("640x360"));
    }

    #[test]
    fn nenhuma_cabe_usa_a_menor() {
        let video = example();
        let (v, ok) = choose(&mp4_variants(&video), video.duration_ms, Some(1), real_size).unwrap();
        assert!(v.url.contains("480x270") && !ok);
    }

    #[test]
    fn best_ignora_limite() {
        let video = example();
        let (v, ok) = choose(&mp4_variants(&video), video.duration_ms, None, |_| unreachable!()).unwrap();
        assert!(v.url.contains("854x480") && ok);
    }
}
