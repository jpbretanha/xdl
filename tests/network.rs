//! Testes que acessam a rede. Informe um post real com vídeo:
//!
//!     XDL_TEST_URL=https://x.com/<usuario>/status/<id> cargo test -- --ignored

use std::process::Command;

fn xdl(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_xdl")).args(args).output().unwrap()
}

fn test_url() -> String {
    std::env::var("XDL_TEST_URL").expect("defina XDL_TEST_URL com o link de um post com vídeo")
}

#[test]
#[ignore]
fn baixa_video_real() {
    let dir = std::env::temp_dir().join(format!("xdl-test-{}", std::process::id()));
    let out = xdl(&[&test_url(), "-o", dir.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect();
    assert!(!files.is_empty(), "nenhum arquivo baixado");
    for f in &files {
        let bytes = std::fs::read(f).unwrap();
        assert_eq!(&bytes[4..8], b"ftyp", "{} não parece um MP4", f.display());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore]
fn links_invalidos() {
    let url = test_url();
    let base = url.split("/video/").next().unwrap().split('?').next().unwrap();
    for url in [
        format!("{base}/video/99"),
        "https://x.com/foo/status/1".to_string(),
        "https://youtube.com/watch?v=abc".to_string(),
    ] {
        let out = xdl(&[&url]);
        assert_eq!(out.status.code(), Some(1), "{url}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("Link inválido"), "{url}");
    }
}
