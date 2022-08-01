# blockstats

Library to connect to a parachain RPC node and monitor stats about its blocks.
This includes the PoV (witness vs. transactions), weight and TX
pool fullness. This is useful to gain insights where about bottlenecks
(computation vs bandwith).

## Use for benchmarking

[`smart-bench`](https://github.com/paritytech/smart-bench) uses this library to benchmark
smart contract execution performance on a parachain.
