# SECSGML Rust [NOT READY FOR USE]

A rust parser for SEC SGML with python bindings.

Rust installation [TODO]

Python Installation
```
pip install secsgmlrs
```

To debug
maturin build --release
pip install --user --force-reinstall target/wheels/secsgmlrs-0.1.1-cp313-cp313-win_amd64.whl
cargo run --release --example process_dir

# 20040101

Files benchmarked: 3530
Python: 1572.03 ms
Python Rust Bindings:   1051.08 ms (we removed parallelism to support better parellism in other areas)
Rust: 500ms

# Todo
make into crate for installation + setup github workflow