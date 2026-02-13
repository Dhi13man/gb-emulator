# Changelog

## [0.2.0] - 2026-02-13

### Fixed

- APU mixer DC offset causing persistent background noise (silence mapped to -1.0 instead of 0.0)
- APU sample rate now synced to actual audio device rate (fixes pitch/underrun on non-44100Hz devices)

### Added

- High-pass filter on APU output emulating Game Boy hardware capacitor coupling (~20Hz cutoff)
- Configurable APU sample rate via `GameBoy::set_sample_rate()`
- Proper DAC-aware channel mixing (DAC off = disconnected from mixer, DAC on = centered output)

### Changed

- Channel `dac_enabled` fields exposed as `pub(crate)` for mixer access
- APU `mix()` now uses physically correct normalization: `(sample/7.5) - 1.0` per channel

## [0.1.0] - 2026-02-12

### Added

- Complete SM83 CPU with all 256 base and 256 CB-prefixed opcodes
- Scanline-accurate PPU with background, window, and sprite rendering
- APU with all 4 channels (2x square with envelope/sweep, wave, noise)
- Frame sequencer for length, envelope, and sweep clocking
- MBC1, MBC2, MBC3, MBC5 cartridge support
- OAM DMA with bus conflict handling
- Timer with DIV/TIMA, falling-edge detection, and delayed TMA reload
- Joypad input with interrupt generation
- Serial port output capture (for test ROM debugging)
- Desktop frontend with pixels 0.15, winit 0.30, cpal 0.15
- Passes all 11 Blargg cpu_instrs tests
- Pokemon Yellow boots and plays intro correctly
