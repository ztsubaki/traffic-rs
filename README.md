# traffic-rs

A platform to simluate real V2X between Vehicles, RSU, Edge and Cloud using libsumocpp.

## Usage

### Build

Make sure your sumo installed with libsumocpp, then run:

```sh
cargo build
```

If your sumo headers and libsumocpp.so is not in your system path, please use environment varible `SUMO_DIR` to specify your SUMO path and make sure `${SUMO_DIR}/include/*` and `${SUMO_DIR}/lib/libsumocpp.so` exist.

### Run

Run `traffic-rs` without any arguments to generate a default config in current directory.

Use command below to begin simluation:

```sh
traffic-rs -c ./conf.toml
```

### Python Plugin

The traffic algorithm Python plugin defines class Node and Cloud, they can get data from PyNet object, which is a read only reference for Net, you can see `src/simulator/mod.rs` and `src/simulator/net.rs` for details.

The plugin cloud apply V2X communications between Node and Cloud, nodes will receive vehicles data from RSU every interval you configured.

A example plugin generate by LLM is in `examples/plugin.py`.
