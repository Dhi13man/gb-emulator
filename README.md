# gb-emulator

[![License](https://img.shields.io/github/license/dhi13man/gb-emulator)](https://github.com/Dhi13man/gb-emulator/blob/main/LICENSE)
[![Language](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Contributors](https://img.shields.io/github/contributors-anon/dhi13man/gb-emulator?style=flat)](https://github.com/Dhi13man/gb-emulator/graphs/contributors)
[![GitHub forks](https://img.shields.io/github/forks/dhi13man/gb-emulator?style=social)](https://github.com/Dhi13man/gb-emulator/network/members)
[![GitHub Repo stars](https://img.shields.io/github/stars/dhi13man/gb-emulator?style=social)](https://github.com/Dhi13man/gb-emulator/stargazers)
[![Last Commit](https://img.shields.io/github/last-commit/dhi13man/gb-emulator)](https://github.com/Dhi13man/gb-emulator/commits/main)

A scanline-accurate Game Boy (DMG-01) emulator written in Rust, with a `no_std`-compatible core library and a desktop frontend using pixels + winit + cpal.

Passes all 11 Blargg `cpu_instrs` tests. Boots and plays Pokemon Yellow correctly.

## Features

- **Accurate CPU**: Full SM83 instruction set (256 base + 256 CB-prefixed opcodes) with correct timing
- **Scanline PPU**: Background, window, and sprite rendering with proper priority handling
- **APU**: All 4 audio channels (2x square, wave, noise) with frame sequencer, envelopes, sweep, and high-pass filtering
- **MBC Support**: No MBC, MBC1, MBC2, MBC3 (no RTC), MBC5
- **Input**: Keyboard-mapped joypad with proper interrupt generation
- **Timer**: DIV/TIMA with falling-edge detection and delayed TMA reload
- **DMA**: OAM DMA with bus conflict emulation

## Architecture

```
gb-emulator/
  gb-core/       # no_std library crate — the emulator engine
    src/
      cpu/       # SM83 CPU, registers, opcode dispatch
      ppu/       # Scanline renderer, OAM handling
      apu/       # Audio channels, frame sequencer, mixer
      cartridge/ # MBC implementations (enum dispatch)
      bus.rs     # Memory bus (address decoding)
      timer.rs   # DIV/TIMA timer subsystem
      joypad.rs  # Joypad input register
      ...
  gb-frontend/   # Desktop binary crate
    src/
      main.rs    # winit event loop, frame pacing
      renderer.rs# pixels surface rendering
      audio.rs   # cpal ring buffer audio output
      input.rs   # Keyboard to joypad mapping
```

The core is `no_std` compatible (uses `extern crate alloc`) so it can be embedded in WASM, embedded targets, or any other Rust environment. The frontend is a thin shell that handles windowing, rendering, and audio I/O.

For comprehensive architectural documentation including C4 diagrams, state machines, data transformation flows, and ADRs, see [docs/architecture/ARCHITECTURE.md](docs/architecture/ARCHITECTURE.md).

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.70+ (stable)

### Build and Run

```bash
cargo build --release
./target/release/gb-frontend path/to/rom.gb
```

### Controls

| Key | Button |
|-----|--------|
| Arrow keys | D-pad |
| Z | A |
| X | B |
| Enter | Start |
| Backspace | Select |
| Escape | Quit |

## Test Results

### Blargg cpu_instrs

| Test | Result |
|------|--------|
| 01-special | Pass |
| 02-interrupts | Pass |
| 03-op sp,hl | Pass |
| 04-op r,imm | Pass |
| 05-op rp | Pass |
| 06-ld r,r | Pass |
| 07-jr,jp,call,ret,rst | Pass |
| 08-misc instrs | Pass |
| 09-op r,r | Pass |
| 10-bit ops | Pass |
| 11-op a,(hl) | Pass |

## Technical References

- [Pan Docs](https://gbdev.io/pandocs/) — Comprehensive Game Boy documentation
- [SM83 Opcodes](https://gbdev.io/gb-opcodes/optables/errata) — Opcode table with errata
- [Game Boy Test ROMs](https://github.com/c-sp/game-boy-test-roms) — Test suite collection

## Contributing

Contributions are welcome. Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
