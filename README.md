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

`GET /api/v1/traffic-lights/0/state` returns the current SUMO signal state and
traffic metrics for each controlled link. `pressure` is the incoming queue count
minus the outgoing queue count for the movement.

Each ID-based resource endpoint also has a collection endpoint:
`GET /api/v1/traffic-lights`, `/traffic-lights/states`, `/edges`, `/lanes`,
`/junctions`, and `/vehicles`. Entries contain `resourceId` for use with the
corresponding detail endpoint, for example `/api/v1/vehicles/{resourceId}`.

Lane and junction resources (`/lanes` and `/junctions`) include `shape`, an
ordered array of absolute SUMO coordinates (`{"x": number, "y": number}`).
Vehicle resources include their current absolute `position` in the same format.

`GET /api/v1/simulation/run?steps=10` is the sole exception: it is a Server-Sent
Events stream. It emits `accepted`, `completed`, or `failed` events; the completed
event contains the run statistics.

### Python Plugin

The traffic-control Python plugin defines the `Node` and `Cloud` classes. Both receive a `PyNet` object, which is a read-only view of the road network; see `src/simulator/mod.rs` and `src/net.rs` for details.

The plugin Cloud handles V2X communication between Nodes and the Cloud. Nodes receive vehicle data from RSUs at the configured reporting interval.

An example plugin is available at `examples/plugin.py`.
