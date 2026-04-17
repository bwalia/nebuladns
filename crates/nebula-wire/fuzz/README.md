# nebula-wire fuzz targets

Run a target locally (requires `cargo install cargo-fuzz` and the nightly toolchain):

```bash
cargo +nightly fuzz run header_decode --fuzz-dir crates/nebula-wire/fuzz -- -max_total_time=60
cargo +nightly fuzz run question_decode --fuzz-dir crates/nebula-wire/fuzz -- -max_total_time=60
```

CI runs each target for 60 seconds on every PR, and continuously via ClusterFuzzLite /
OSS-Fuzz (post-M0).
