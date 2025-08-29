# GitHub Actions Workflows

This ESP32 Rust project contains three main GitHub Actions workflows for automated building, testing, and release processes.

## Workflow Descriptions

### 1. Code Check (`check.yml`)

**Trigger Conditions:**

- Push to `main` or `develop` branches
- Pull Requests targeting `main` or `develop` branches

**Features:**

- Code formatting check (`cargo +esp fmt`)
- Code quality check (`cargo +esp clippy`)
- Build main program for ESP32-C3 (debug and release)
- Upload build artifacts

**Purpose:** Ensure code quality and successful builds

### 2. Development Release (`dev-release.yml`)

**Trigger Conditions:**

- Push to `main` branch

**Features:**

- Automatically build release version
- Generate development version number (format: `dev-YYYYMMDD-HHMMSS-commit_hash`)
- Create prerelease version with ESP32 binary files
- Automatically clean up old development versions (keep latest 10)

**Purpose:** Provide testable build versions for each main branch update

### 3. Release (`release.yml`)

**Trigger Conditions:**

- Manual trigger (workflow_dispatch)

**Features:**

- Support version type selection (patch/minor/major)
- Support prerelease versions
- Automatically generate semantic version numbers
- Generate changelog
- Create official release versions with ESP32-specific instructions

**Purpose:** Create official software release versions

## Usage

### Development Workflow

1. **Daily Development:** Develop in feature branches, create PRs to the `develop` branch
2. **Code Check:** PRs will automatically trigger `check.yml` for code quality checks
3. **Integration Testing:** Continue testing after merging to the `develop` branch
4. **Release Preparation:** Merge from `develop` to the `main` branch
5. **Automatic Build:** Pushing to `main` will automatically trigger `dev-release.yml` to create development versions
6. **Official Release:** Manually trigger `release.yml` to create official versions

### Manual Release Steps

1. Go to the GitHub repository's Actions page
2. Select the "Release" workflow
3. Click "Run workflow"
4. Select version type:
   - **patch**: Bug fix version (1.0.0 → 1.0.1)
   - **minor**: Feature version (1.0.0 → 1.1.0)
   - **major**: Major version (1.0.0 → 2.0.0)
5. Choose whether it's a prerelease version
6. Click "Run workflow" to start the release

### Version Number Rules

- **Development Version:** `dev-20240101-120000-abc1234`
- **Official Version:** `v1.2.3`
- **Prerelease Version:** `v1.2.3-rc.20240101120000`

## Build Artifacts

Each workflow generates the following build artifacts:

- `network-monitor-esp-rs-no-std` - Main firmware file (ELF format)
- `network-monitor-esp-rs-no-std.bin` - Binary firmware file for flashing
- Build logs and debug information

## ESP32-Specific Configuration

### Target Architecture

- **Chip:** ESP32-C3 (RISC-V)
- **Target:** `riscv32imc-unknown-none-elf`
- **Toolchain:** ESP Rust toolchain

### Flashing Tools

- **espflash:** Primary flashing tool
- **esptool.py:** Alternative flashing tool
- **cargo run:** Development flashing

## Important Notes

1. **Permission Requirements:** Requires `contents: write` permission to create releases
2. **Cache Optimization:** Uses Cargo cache to speed up builds
3. **Target Architecture:** Builds for `riscv32imc-unknown-none-elf` (ESP32-C3)
4. **Automatic Cleanup:** Development versions automatically keep the latest 10, avoiding repository bloat
5. **ESP Toolchain:** Uses ESP-specific Rust toolchain for proper ESP32 support

## Troubleshooting

### Build Failures

- Check ESP Rust toolchain installation
- Verify ESP32-C3 target is available
- Review error messages in build logs
- Ensure environment variables are set correctly

### Release Failures

- Confirm GitHub Token permissions
- Check for version number conflicts
- Verify build artifacts exist
- Ensure espflash installation succeeds

### Cache Issues

- Can manually clear cache on Actions page
- Or modify `Cargo.lock` file to trigger cache update

## Environment Variables

For successful builds and flashing, ensure these environment variables are configured:

- `SSID`: WiFi network name
- `PASSWORD`: WiFi password
- `SERVER_ADDRESS`: Network monitoring server address

## Hardware Requirements

- ESP32-C3 microcontroller
- ST7735 TFT display
- WiFi connectivity
- USB-C programming interface
