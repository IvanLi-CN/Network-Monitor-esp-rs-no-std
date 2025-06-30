# Network-Monitor-esp-rs-no-std

This is the MCU software part of an Network speed monitor based on ESP32C3. The project collects network speed information from OpenWRT and OpenClash through a Rust server and sends the result to the monitor via UDP to display to the user.

![Finished Product](https://s3.ivanli.cc/ivan-public/uPic/2024/2atMTj.png)

## Hardwares

- MCU: ESP3-C3FH4
- Display: ST7735 80x160 RGB 0.96 inch

## Configuration

The project requires three environment variables to be configured at compile time:

- `SSID`: WiFi network name
- `PASSWORD`: WiFi password
- `SERVER_ADDRESS`: Server address in format "IP:PORT" (e.g., "192.168.1.100:17890")

These variables are read using Rust's `env!` macro during compilation. If any variable is missing, compilation will fail with an error.

### Local Development

You can set these variables in several ways:

#### Option 1: Configure in `.cargo/config.toml`

```toml
[env]
SSID = "your-wifi-ssid"
PASSWORD = "your-wifi-password"
SERVER_ADDRESS = "your-server-ip:port"
```

#### Option 2: Set environment variables before building

```bash
export SSID="your-wifi-ssid"
export PASSWORD="your-wifi-password"
export SERVER_ADDRESS="192.168.1.100:17890"
cargo build --release
```

#### Option 3: Set variables for a single build

```bash
SSID="your-wifi-ssid" PASSWORD="your-wifi-password" SERVER_ADDRESS="192.168.1.100:17890" cargo build --release
```

### Building Firmware via GitHub Actions

This repository includes a GitHub Actions workflow for building firmware with custom configurations:

1. **Navigate to Actions**: Go to the "Actions" tab in your GitHub repository
2. **Select Workflow**: Choose "Build Firmware" from the workflow list
3. **Run Workflow**: Click "Run workflow" button
4. **Fill Configuration**: Enter the required parameters:
   - **WiFi SSID**: Your WiFi network name
   - **WiFi Password**: Your WiFi password
   - **Server Address**: Server IP and port in "IP:PORT" format
5. **Start Build**: Click "Run workflow" to begin the build process

#### Build Artifacts

After the workflow completes successfully, you'll find the following artifacts:

- **ELF executable**: `network-monitor-esp-rs-no-std` (original binary)
- **Binary firmware**: `firmware.bin` (ready for flashing)
- **Build information**: `build-info.txt` (contains build details and configuration)

The artifacts are retained for 30 days and can be downloaded from the workflow run page.

#### Technical Implementation

The GitHub Actions workflow:

- Uses Ubuntu latest runner
- Installs Rust nightly toolchain with RISC-V target
- Installs required tools (`espflash`, `cargo-binutils`)
- Sets the three environment variables from user input
- Builds the release version of the firmware
- Generates a flashable binary file using `cargo objcopy`
- Creates a build information file with configuration details
- Uploads all artifacts for download

## Dependencies

- [esp-hal](https://github.com/esp-rs/esp-hal) (`no-std`)
- [esp-wifi](https://github.com/esp-rs/esp-wifi)
- [embassy](https://embassy.dev/)
- [st7735](https://github.com/kalkyl/st7735-embassy) (forked)

## Other Resources

- [Server Code](https://git.ivanli.cc/display-ambient-light/network-monitor);
- [Shell model](https://s.ivanli.cc/s/network-monitor-shell);

## License

MIT.
