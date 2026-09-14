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

### HTTP API

The server exposes a versioned JSON API at `/api/v1` and its OpenAPI 3.1 document at
`/openapi.json`. Resource identifiers are path parameters, for example
`GET /api/v1/traffic-lights/0`. JSON errors use `application/problem+json` and the
RFC 9457 fields `type`, `title`, `status`, and `detail`.

`GET /api/v1/simulation/run?steps=10` is the sole exception: it is a Server-Sent
Events stream. It emits `accepted`, `completed`, or `failed` events; the completed
event contains the run statistics.

### Python Plugin

The traffic algorithm Python plugin defines class Node and Cloud, they can get data from PyNet object, which is a read only reference for Net, you can see `src/simulator/mod.rs` and `src/simulator/net.rs` for details.

The plugin cloud apply V2X communications between Node and Cloud, nodes will receive vehicles data from RSU every interval you configured.

A example plugin generate by LLM is in `examples/plugin.py`.
