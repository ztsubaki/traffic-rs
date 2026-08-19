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
RUST_LOG=info traffic-rs -c ./conf.toml
```


