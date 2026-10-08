//! Download with a progress bar, writing to `.part` and renaming when done.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};

/// Actual size from a HEAD request's `Content-Length`; `None` if unavailable.
pub fn remote_size(agent: &ureq::Agent, url: &str) -> Option<u64> {
    let resp = agent.head(url).call().ok()?;
    resp.headers().get("content-length")?.to_str().ok()?.parse().ok()
}

pub fn download(agent: &ureq::Agent, url: &str, dest: &Path) -> Result<u64> {
    let mut resp = agent.get(url).call().context("failed to start download")?;
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());

    let pb = match total {
        Some(n) => ProgressBar::new(n).with_style(
            ProgressStyle::with_template("{bar:40.cyan/blue} {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏ "),
        ),
        None => ProgressBar::new_spinner(),
    };

    let part = dest.with_extension("mp4.part");
    let result = (|| -> Result<u64> {
        let mut out = BufWriter::new(File::create(&part).context("could not create file")?);
        let mut reader = pb.wrap_read(resp.body_mut().as_reader());
        let n = io::copy(&mut reader, &mut out)?;
        out.flush()?;
        Ok(n)
    })();
    pb.finish_and_clear();

    match result {
        Ok(n) => {
            fs::rename(&part, dest)?;
            Ok(n)
        }
        Err(e) => {
            let _ = fs::remove_file(&part);
            Err(e)
        }
    }
}
