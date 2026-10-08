# xdl

Downloads videos from X (formerly Twitter) posts at the highest resolution that fits within a size limit (default: 100 MB).

## Installation (macOS)

Requires [Rust](https://rustup.rs) (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).

```sh
cargo install --git https://github.com/jpbretanha/xdl
# or, from a local clone:
cargo install --path .
```

The binary is installed to `~/.cargo/bin/xdl`.

## Usage

```sh
xdl https://x.com/user/status/1234567890123456789/video/1
xdl <link> -o ~/Videos        # output directory (default: ~/Downloads)
xdl <link> --max-size 20MB    # size limit (KB, MB, GB)
xdl <link> --best             # ignore the size limit
```

- `/video/N` downloads only the N-th media item; without the suffix, all videos in the post are downloaded.
- Invalid links, missing/private posts and posts without video print `Link inválido` (exit code 1). Other failures (network, etc.) exit with code 3.

## How it works

1. Validates the link and extracts the post ID (`src/link.rs`).
2. Queries X's public syndication endpoint, falling back to the FxTwitter API if it fails (`src/source.rs`).
3. Sorts the MP4 variants by bitrate and picks the best one whose actual size (`Content-Length`) fits the limit (`src/select.rs`).
4. Downloads to `file.mp4.part` and renames it once complete (`src/download.rs`).

## Tests

```sh
cargo test                                               # offline, uses fixtures in tests/fixtures/
XDL_TEST_URL=<link to a post with video> cargo test -- --ignored   # hits the network
```
