//! Tests that hit the network. Provide a real post with a video:
//!
//!     XDL_TEST_URL=https://x.com/<user>/status/<id> cargo test -- --ignored

use std::process::Command;

fn xdl(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_xdl")).args(args).output().unwrap()
}

fn test_url() -> String {
    std::env::var("XDL_TEST_URL").expect("set XDL_TEST_URL to the link of a post with a video")
}

#[test]
#[ignore]
fn downloads_real_video() {
    let dir = std::env::temp_dir().join(format!("xdl-test-{}", std::process::id()));
    let out = xdl(&[&test_url(), "-o", dir.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect();
    assert!(!files.is_empty(), "no file downloaded");
    for f in &files {
        let bytes = std::fs::read(f).unwrap();
        assert_eq!(&bytes[4..8], b"ftyp", "{} does not look like an MP4", f.display());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore]
fn invalid_links() {
    let url = test_url();
    let base = url.split("/video/").next().unwrap().split('?').next().unwrap();
    for url in [
        format!("{base}/video/99"),
        "https://x.com/foo/status/1".to_string(),
        "https://youtube.com/watch?v=abc".to_string(),
    ] {
        let out = xdl(&[&url]);
        assert_eq!(out.status.code(), Some(1), "{url}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("Invalid link"), "{url}");
    }
}
