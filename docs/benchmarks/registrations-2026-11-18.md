# Output-side registrations: CPU measurements — 2026-11-18

Environment: macOS, Apple M2 Pro / Metal as reported by wgpu, Rust 1.99.1,
release with thin LTO. These measurements cover CPU operations, not node rendering,
GPU execution, display refresh, or FPS.

Before the change, connection lists were converted into an input-lookup HashMap and
reconnection cloned the entire Graph. The revised implementation combined output-owned
registrations, array-based binding resolution, and reconnect rollback of changed entries.
The individual contribution of each change was not measured separately.

The benchmark is [`examples/connection_bench.rs`](../../examples/connection_bench.rs).
It uses 20 warm-up iterations and nine samples. Planning and wiring use 300 iterations
per sample; steady evaluation uses 10,000. The old and new release executables were each
run five times with alternating order. The table compares medians of the per-run medians.
Other applications were not stopped. [Raw results](connections.csv) retain the five runs'
median, minimum, and maximum values. Units are microseconds.

| Patch | Operation | Before | After |
| --- | --- | ---: | ---: |
| 48 nodes, 92 connections | Image planning | 11.126 | 5.317 |
| Same | Signal planning | 16.192 | 12.393 |
| Same | Replace a source | 3.296 | 2.004 |
| Same | Reconnect a cable | 7.372 | 2.078 |
| Same | Signal tick + image modulation | 0.412 | 0.408 |
| 256 nodes, 508 connections | Image planning | 65.251 | 27.523 |
| Same | Signal planning | 78.453 | 59.052 |
| Same | Replace a source | 17.184 | 11.175 |
| Same | Reconnect a cable | 39.420 | 11.421 |
| Same | Signal tick + image modulation | 1.832 | 1.803 |
| 67 audio nodes, 129 connections | Audio planning | 12.996 | 10.182 |
| Same | Audio route update | 0.082 | 0.082 |

The image/signal patches include serial image processing, serial numeric processing, and
fan-out. The audio patch uses 64 Audio gain stages and numeric control input. The measured
improvement concerns planning and connection edits. Steady evaluation already read resolved
slots directly; its small differences do not establish an FPS improvement.

Run the current benchmark with:

```sh
cargo run --release --example connection_bench
```

SHA-256 of the executables used for this comparison:

- Before: `ec2f6cbf92ce1dcdbd7279400b59d9580e3bfe6b1d573d6a1f967c5adae28c55`
- After: `d1861491633206af51a264a8da3393fbff703ba9320f0851dee0143e2a3a89ad`
