# SECSGML Rust [NOT READY FOR USE]

A rust parser for SEC SGML with python bindings.

Rust installation [TODO]

Python Installation
```
pip install secsgmlrs
```

To debug
maturin develop --features python
pip install --user --force-reinstall target/wheels/secsgmlrs-0.1.1-cp313-cp313-win_amd64.whl
cargo run --release --example process_dir

# Performance (before optimization)
Files benchmarked: 3530
Python: 1572.03 ms
Python Rust Bindings:   634.44 ms
Rust: 540ms

# rust parallel
540ms
540ms
510ms

## Rust nonparallel
1.31s (after first byte fix)
1s (after wraparound fix)
1s 
# TODO
renable parallelism