---
decision_date: 2025-01-01
pattern: scanline-rendering
repository: gb-emulator
status: Discovered
tags: [adr, ppu, rendering, accuracy]
title: "0002. Scanline-Based PPU Rendering"
type: adr
---

# 0002. Scanline-Based PPU Rendering

Date: 2025-01-01

## Status

Discovered (existing architecture)

## Context

The Game Boy PPU can be emulated at different accuracy levels:

1. **Frame-at-once**: Render the entire 160x144 frame when VBlank starts. Simple but misses mid-frame register changes.
2. **Scanline-based**: Render one 160-pixel line at the end of each Drawing phase (mode 3). Catches most mid-frame effects.
3. **Pixel FIFO**: Emulate the hardware pixel pipeline dot-by-dot. Maximum accuracy but significantly more complex and slower.

Evidence:

- `gb-core/src/ppu/mod.rs:188-190`: Scanline rendered at end of Drawing mode (`render_scanline()` called when `dot >= OAM_SCAN_DOTS + DRAWING_DOTS`)
- `gb-core/src/ppu/mod.rs:277-328`: `render_scanline()` renders BG, window, and sprites for current LY
- `gb-core/src/ppu/mod.rs:177-227`: `tick_one_dot()` advances state machine dot-by-dot but renders per-scanline
- Drawing mode uses a fixed 172 dots (simplified from variable hardware timing)

## Decision

The system uses **scanline-based rendering** where the PPU state machine advances dot-by-dot (for accurate mode transitions and STAT interrupt timing) but renders an entire scanline at the boundary between Drawing and HBlank modes.

## Consequences

**Benefits**:

- Sufficient accuracy for the vast majority of commercial games (including Pokemon Yellow)
- Passes all 11 Blargg `cpu_instrs` tests
- STAT interrupt timing is dot-accurate (rising-edge detection on composite STAT line)
- Mode transitions and memory access locks are cycle-accurate
- Significantly simpler than pixel FIFO (~200 lines vs ~500+ lines for FIFO)
- Better performance: one scanline render call vs 160 individual pixel pushes

**Trade-offs**:

- Fixed Drawing mode duration (172 dots) instead of variable (depends on sprites, scroll, window)
- Mid-scanline register writes (e.g., changing SCX partway through a line) are not captured
- A small number of games that rely on pixel-precise timing (e.g., some demo scene ROMs) may not render correctly
- Cannot accurately emulate the pixel FIFO's sprite fetch penalty
