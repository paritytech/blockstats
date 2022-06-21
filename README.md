# blockstats

Connect to a parachain RPC node and monitor stats about its blocks.
This includes the PoV (witness vs. transactions), weight and TX
pool fullness. This is useful to gain insights where about bottlenecks
(computationb vs bandwith).

This crate contains a library you can depend on and also a basic binary that just prints
the block stats to stdout.
