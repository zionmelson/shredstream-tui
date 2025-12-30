# ShredStream TUI

A terminal user interface (TUI) for monitoring the Jito ShredStream proxy in real-time.

![ShredStream TUI](https://via.placeholder.com/800x400?text=ShredStream+TUI)

## Features

- **Real-time Metrics Dashboard**: Monitor shred reception, entries, and transaction rates
- **Slot History**: Track slot-by-slot data with entry and transaction counts
- **Transaction Sampling**: View sample transaction signatures as they arrive
- **Activity Logs**: Monitor connection events and errors
- **Connection Status**: Live connection state with auto-reconnect support
- **Rate Calculations**: Entries/second and transactions/second metrics

## Tabs

1. **Overview**: Main dashboard with connection info, current metrics, cumulative stats, and rate sparklines
2. **Slots**: Detailed slot history table showing entries and transactions per slot
3. **Transactions**: Sample of recent transaction signatures
4. **Logs**: Application event log with timestamps and severity levels

## Prerequisites

- Rust toolchain (1.70+)
- A running ShredStream proxy with gRPC service enabled (`--grpc-service-port`)

## Installation

```bash
# Clone the repository (if not already cloned)
cd shredstream-tui

# Build the project
cargo build --release

# The binary will be at ./target/release/shredstream-tui
```

## Usage

```bash
# Connect to a local proxy on the default port
./target/release/shredstream-tui --proxy-url http://127.0.0.1:50051

# Or use environment variable
export SHREDSTREAM_PROXY_URL=http://your-proxy:50051
./target/release/shredstream-tui

# Customize refresh rate and metrics window
./target/release/shredstream-tui \
    --proxy-url http://127.0.0.1:50051 \
    --tick-rate 50 \
    --metrics-window 15
```

### Command Line Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--proxy-url` | `-p` | gRPC endpoint for the ShredStream proxy | `http://127.0.0.1:50051` |
| `--tick-rate` | `-t` | UI refresh rate in milliseconds | `100` |
| `--metrics-window` | `-m` | Metrics window duration in seconds | `10` |

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q`, `Ctrl+C` | Quit the application |
| `←`, `→`, `Tab` | Switch between tabs |
| `↑`, `↓` | Scroll up/down |
| `r` | Reset current metrics window |
| `?` | Toggle help overlay |
| `Esc` | Close help overlay |

## Running with ShredStream Proxy

To use this TUI, you need to run the ShredStream proxy with the gRPC service enabled:

```bash
# Start the proxy with gRPC service
./shredstream-proxy shredstream \
    --block-engine-url https://mainnet.block-engine.jito.wtf \
    --auth-keypair /path/to/keypair.json \
    --desired-regions amsterdam,ny \
    --dest-ip-ports 127.0.0.1:8001 \
    --grpc-service-port 50051  # This enables the gRPC service

# Then in another terminal, start the TUI
./shredstream-tui --proxy-url http://127.0.0.1:50051
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    ShredStream TUI                       │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │
│  │   Client    │→ │    State    │→ │       UI        │  │
│  │  (gRPC)     │  │  (AppState) │  │   (ratatui)     │  │
│  └─────────────┘  └─────────────┘  └─────────────────┘  │
│         ↑                                                │
│         │                                                │
└─────────┼────────────────────────────────────────────────┘
          │
          ↓
┌─────────────────────────────────────────────────────────┐
│              ShredStream Proxy (gRPC Service)            │
│                                                          │
│  Receives shreds → Reconstructs entries → Streams data   │
└─────────────────────────────────────────────────────────┘
```

## Metrics Explained

### Current Window Metrics
- **Entries**: Number of Solana entries received in the current window
- **Transactions**: Number of transactions decoded from entries
- **Recovered**: Shreds recovered using FEC (Forward Error Correction)

### Cumulative Statistics
- **Total Entries/Transactions**: All-time counts since TUI started
- **Received**: Total shreds received by the proxy
- **Forwarded**: Successfully forwarded shreds
- **Failed**: Failed forwarding attempts
- **Duplicates**: Duplicate shreds filtered out

## License

MIT License - see the LICENSE file for details.
