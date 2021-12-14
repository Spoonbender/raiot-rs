# ABANDONED

I created raiot in order to learn rust.
I currently have no interest in pursuing this specific project, as I no longer deal with Auzre IoT Hub as part of my job.

If you are looking for salvage, I suggest looking at the `raiot-protocol` directory (crate), since that crate actually implements the logic of the Azure IoT Hub MQTT protocol codec.
The rest is probably not really worth your time.

# raiot - Rust Azure IoT Device SDK
raiot is an SDK for developing IoT devices that work with the Azure IoT Hub.

Highlights:
- Pure Rust implementation
- MQTT-based
- Enables users to compile just the functionality they need, according to the set of Azure IoT Hub features they use

## Status

raiot is in very early, experimental stage.
I'm experimenting with a few approaches for implementing the SDK, including:
- Blocking API
- Non-blocking API via task-based programming
- Non-blocking, futures-based API

raiot currently lacks a real design, which will be conducted after I'm done experimenting.
The API surface of raiot is also subject for many changes.

## Crates Structure
- `raiot-protocol`: 
  - defines the data contracts of the Azure IoT Hub
  - provides a codec for translating between IoT Hub messages and MQTT packets
  - provides means for SAS-token generation
- `raiot-mqtt`:
  - helper types for packetizing MQTT
  - MQTT connectivity
- `raiot-stclient`:
  - experimental, single-threaded, non-blocking, task-based client
- `raiot-client`:
  - experimental, futures-based client
- `raiot-streams`:
  - helpers for TCP and TLS
- `raiot-test-utils`:
  - helpers for testing MQTT over TCP without actual network calls
- `raiot-buffers`:
  - simple implementation of circular buffer
- `raiot-cli`:
  - helpers for dealing with command line arguments


## Build Features


Cargo Features:
- Basic tier features
  - **telemetry**: adds support for telemetry messages (D2C messages)
  - **twin**: adds support for Twin reads and updates
- Standard tier features
  - **c2d**: adds support for cloud-to-device message
  - **direct-methods**: adds support for direct method invocation


## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
