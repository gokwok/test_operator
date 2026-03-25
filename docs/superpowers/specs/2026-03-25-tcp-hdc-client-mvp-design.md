# TCP HDC Client MVP Design

**Date:** 2026-03-25

**Goal:** Build a standalone Rust MVP that connects directly to an OpenHarmony/HarmonyOS device daemon over TCP, completes the official HDC session handshake including RSA authentication, executes one single-shot shell command, and closes the channel cleanly.

**Target Test Device:** `192.168.8.43:35319`

## Scope

This MVP implements only the minimum protocol surface needed to:

- Connect to a device daemon over TCP.
- Complete `CMD_KERNEL_HANDSHAKE`.
- Execute exactly one single-shot shell command through `CMD_UNITY_EXECUTE`.
- Receive output through `CMD_KERNEL_ECHO` / `CMD_KERNEL_ECHO_RAW`.
- Close the channel through `CMD_KERNEL_CHANNEL_CLOSE`.

## Non-Goals

This MVP does not implement:

- The official local `hdc client <-> hdc server` protocol.
- A background host server process.
- USB, UART, BT, bridge, or UDP discover.
- Multi-device management or `list targets`.
- File transfer, app install, forward/reverse forward, JDWP, bugreport.
- Interactive shell streaming with `CMD_SHELL_DATA`.
- Compatibility with every historical HDC release.

## Protocol Boundary

The implementation will speak the device-facing session protocol only.

It will not implement the host-facing channel protocol used by the official `hdc` CLI to talk to the local host server.

This distinction is critical:

- Official local client/server framing uses a length-prefixed channel message format.
- Device daemon communication uses the session packet format built from `PayloadHead + PayloadProtect + payload`.

The MVP will implement the second one only.

## Source References

Primary protocol references in the vendored source tree:

- `third_party/developtools_hdc/src/common/define.h`
- `third_party/developtools_hdc/src/common/define_enum.h`
- `third_party/developtools_hdc/src/common/session.h`
- `third_party/developtools_hdc/src/common/session.cpp`
- `third_party/developtools_hdc/src/daemon/daemon.cpp`
- `third_party/developtools_hdc/src/daemon/daemon_tcp.cpp`
- `third_party/developtools_hdc/hdc_rust/src/config.rs`
- `third_party/developtools_hdc/hdc_rust/src/serializer/*`
- `third_party/developtools_hdc/hdc_rust/src/daemon_lib/auth.rs`
- `third_party/developtools_hdc/hdc_rust/src/transfer/tcp.rs`

## MVP Architecture

The program will be a direct daemon client with a single TCP connection, one session ID, and one channel ID.

Suggested runtime flow:

1. Parse CLI arguments.
2. Open a TCP connection to the daemon.
3. Generate or load host RSA keys.
4. Send initial `CMD_KERNEL_HANDSHAKE`.
5. Drive the RSA authentication round-trips until `AUTH_OK`.
6. Send one `CMD_UNITY_EXECUTE` payload such as `ls /data/local/tmp`.
7. Stream output until the remote side closes the channel or no more data arrives.
8. Send `CMD_KERNEL_CHANNEL_CLOSE` if still connected.
9. Exit with a clear success or failure status.

## Protocol Constants

The implementation will use these protocol constants:

- `HANDSHAKE_MESSAGE = "OHOS HDC"`
- `PACKET_FLAG = "HW"`
- `VER_PROTOCOL = 1`
- `PAYLOAD_VCODE = 0x09`
- `channel_id = 1` for the single command path

## Session Packet Format

Each daemon-facing packet is:

1. `PayloadHead`
2. serialized `PayloadProtect`
3. raw payload bytes

`PayloadHead` fields:

- `flag[2] = "HW"`
- `reserve[2] = 0`
- `protocol_ver = 1`
- `head_size = be16(serialized PayloadProtect length)`
- `data_size = be32(payload length)`

`PayloadProtect` fields:

- `channel_id: u32`
- `command_flag: u32`
- `check_sum: u8`
- `v_code: u8`

The MVP will keep checksum disabled and send `check_sum = 0`, matching the current upstream default.

## Handshake State Machine

The client will implement this state machine:

### 1. Initial Hello

Send `CMD_KERNEL_HANDSHAKE` with:

- `banner = "OHOS HDC"`
- `auth_type = AUTH_NONE`
- `session_id = random u32`
- `connect_key = "<target-ip>:<target-port>"` string provided by CLI or derived from the daemon address
- `buf = TLV(TAG_AUTH_TYPE = RSA_3072_SHA512)`
- `version = configurable version string`

### 2. Daemon Requests Public Key

When daemon replies with `auth_type = AUTH_PUBLICKEY`, the client:

- Parses daemon capability TLV if present.
- Builds `hostname + separator + public_key_pem` in the expected payload form.
- Sends another `CMD_KERNEL_HANDSHAKE`.

### 3. Daemon Requests Signature

When daemon replies with `auth_type = AUTH_SIGNATURE`, the client:

- Extracts the daemon token from `buf`.
- Signs the token with RSA-PSS-SHA512 using the host private key.
- Base64-encodes the signature.
- Sends another `CMD_KERNEL_HANDSHAKE`.

### 4. Authentication OK

When daemon replies with `auth_type = AUTH_OK`, the session becomes usable.

If daemon security is disabled or `persist.hdc.auth_bypass=1`, the daemon may return `AUTH_OK` immediately. The client must support both paths.

## Version Strategy

Upstream compares the handshake version string against the daemon version string and may reject very old clients.

The MVP will:

- expose a configurable version string through CLI and code,
- default to a modern OpenHarmony-style value,
- keep the exact string isolated in one place so it can be adjusted after the first device-side test if needed.

This is intentionally configurable because the upstream daemon compares strings derived from internal build metadata.

## Command Strategy

Only one single-shot shell command path is supported in MVP:

- `CMD_UNITY_EXECUTE`

The payload will be the exact shell string, for example:

```text
ls /data/local/tmp
```

The first public CLI surface will therefore support:

```bash
hmdriver_rs tcp --addr 192.168.8.43:35319 shell ls /data/local/tmp
```

Internally this becomes one single shell command string.

`CMD_SHELL_INIT` is intentionally out of scope for MVP because upstream uses it for interactive shell initialization rather than single-shot command execution.

## Output Handling

After command dispatch, the client will read packets in a loop and handle:

- `CMD_KERNEL_ECHO_RAW`
  - print bytes directly to stdout
- `CMD_KERNEL_ECHO`
  - decode the first byte as message level and print the remainder as text
- `CMD_KERNEL_CHANNEL_CLOSE`
  - stop reading and exit the command loop
- `CMD_HEARTBEAT_MSG`
  - ignore for MVP or log at debug level

Any unexpected command will be treated as a protocol error and surfaced clearly.

## Channel Close Semantics

The client will send `CMD_KERNEL_CHANNEL_CLOSE` with a one-byte payload count when closing the only active channel.

If the daemon closes first, the client will accept that as normal termination and avoid double-closing where possible.

## Key Management

The MVP will use OpenSSL through the Rust `openssl` crate.

Key requirements:

- Generate an RSA keypair if missing.
- Store keys in a dedicated configurable directory.
- Avoid depending on any existing official `hdc` key path.
- Export the public key in the daemon-expected PEM form.

Default key directory will be a user-local app-specific path, with a CLI override for deterministic testing.

## File Layout

Planned file layout:

- `src/main.rs`
  - CLI entrypoint
- `src/cli.rs`
  - minimal argument parsing and command assembly
- `src/error.rs`
  - unified error types
- `src/protocol.rs`
  - constants, enums, TLV tags
- `src/codec.rs`
  - `PayloadHead`, `PayloadProtect`, `SessionHandShake` encode/decode
- `src/auth.rs`
  - key generation, public key export, RSA-PSS-SHA512 signing
- `src/client.rs`
  - connection lifecycle, handshake state machine, shell command execution

## Dependencies

Expected dependencies:

- `openssl`
- `base64`
- `rand`

Additional small utility crates are acceptable if they keep the implementation simpler and more reliable, but the MVP should avoid heavy framework dependencies.

## Verification Plan

Local verification:

- Unit test packet head encode/decode.
- Unit test payload protect encode/decode.
- Unit test TLV append/parse behavior needed for handshake.
- Unit test handshake message round-trip serialization.
- Unit test signature generation on a known token.

Device verification:

1. Connect to `192.168.8.43:35319`.
2. Complete handshake.
3. Execute `ls /data/local/tmp`.
4. Print output.
5. Close channel cleanly.

## Success Criteria

The MVP is successful when all of the following are true:

- It authenticates successfully against the target daemon over TCP.
- It executes `hdc shell ls` equivalent behavior via `CMD_UNITY_EXECUTE`.
- It prints command output.
- It exits cleanly after `CMD_KERNEL_CHANNEL_CLOSE`.

## Known Risks

- The exact handshake version string may need adjustment after first contact with the real target daemon.
- The daemon may require the host public key to be manually approved on-device.
- Public key payload formatting must match the daemon expectation exactly.
- Message ordering around close can differ slightly between daemon builds, so close handling must be tolerant.
