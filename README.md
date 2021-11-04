Simple vcp server restarter for n et cu p servers based upon reachability of a specified URL

# building

- install [rust](https://www.rust-lang.org/tools/install)
- copy `example.credentials.toml` to `.credentials.toml` and set your credentials
- run `cargo build --release`
- binary in `target/release/vcp_monitoring`

# running
- run binary in `target/release/vcp_monitoring`
- or use `cargo run --release`
