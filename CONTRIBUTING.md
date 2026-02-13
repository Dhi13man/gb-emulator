# Contributing to gb-emulator

[![License](https://img.shields.io/github/license/dhi13man/gb-emulator)](https://github.com/Dhi13man/gb-emulator/blob/main/LICENSE)
[![Contributors](https://img.shields.io/github/contributors-anon/dhi13man/gb-emulator?style=flat)](https://github.com/Dhi13man/gb-emulator/graphs/contributors)

Thank you for your interest in contributing! Whether it's a bug fix, new feature, or documentation improvement, contributions are welcome.

## Getting Started

1. **Prerequisites**: Install [Rust](https://www.rust-lang.org/tools/install) (stable, 1.70+).

2. **Fork and clone**:

   ```bash
   git clone https://github.com/<your-username>/gb-emulator.git
   cd gb-emulator
   ```

3. **Build**:

   ```bash
   cargo build --release
   ```

4. **Run with a test ROM**:

   ```bash
   ./target/release/gb-frontend path/to/rom.gb
   ```

## Project Structure

| Crate | Description |
|-------|-------------|
| `gb-core` | `no_std` emulator library: CPU, PPU, APU, bus, cartridge, timer, joypad |
| `gb-frontend` | Desktop binary: windowing (winit), rendering (pixels), audio (cpal), input |

### Key Design Decisions

- **CPU**: Match-based opcode dispatch (compiler optimizes to jump table)
- **Memory Bus**: Struct with match on address ranges (no trait objects)
- **PPU**: Scanline-based rendering (not pixel FIFO)
- **MBC**: Enum dispatch wrapping MBC structs (zero-cost abstraction)
- **Timing**: "Step CPU, catch up subsystems" model
- **Core**: `no_std` compatible via `extern crate alloc`

## How to Contribute

### Bug Fixes

1. Create a branch: `git checkout -b fix/description`
2. Fix the issue and verify with relevant test ROMs
3. Run `cargo build --release` and `cargo clippy` to ensure no warnings
4. Commit and open a PR

### New Features

Possible areas for contribution:

- **Accuracy**: Pixel FIFO PPU, T-cycle accurate CPU, APU edge cases
- **Cartridge**: MBC3 RTC, MBC6, MBC7, HuC1, HuC3
- **Testing**: Mooneye, dmg-acid2, other test ROM suites
- **Frontend**: Save states, fast forward, rewind, debugger UI
- **Platforms**: WASM/web frontend, mobile

### Test ROMs

Before submitting, verify your changes don't break existing functionality:

- [Blargg's cpu_instrs](https://github.com/retrio/gb-test-roms) (all 11 must pass)
- Boot a commercial game (e.g., Tetris) and verify gameplay

## Code Style

- Follow standard Rust conventions (`cargo fmt`)
- Run `cargo clippy` before committing
- Keep `gb-core` free of `std` dependencies
- Minimize unsafe code

## Reporting Issues

File issues on [GitHub Issues](https://github.com/Dhi13man/gb-emulator/issues). Include:

- ROM name and MBC type (if applicable)
- Expected vs actual behavior
- Screenshot if visual bug
