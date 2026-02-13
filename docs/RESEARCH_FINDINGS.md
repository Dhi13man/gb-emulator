# Gameboy (DMG) Emulator Development in Rust: Comprehensive Research Findings

**Research Date**: 2026-02-13
**Scope**: Architecture patterns, implementation strategies, and ecosystem analysis for building a Gameboy emulator in Rust
**Confidence Level**: High (multiple authoritative sources cross-referenced)

---

## Table of Contents

1. [Existing Rust GB Emulators](#1-existing-rust-gb-emulators)
2. [CPU Emulation Patterns](#2-cpu-emulation-patterns)
3. [Memory Bus Architecture](#3-memory-bus-architecture)
4. [PPU (Picture Processing Unit)](#4-ppu-picture-processing-unit)
5. [APU (Audio Processing Unit)](#5-apu-audio-processing-unit)
6. [Cartridge/MBC Handling](#6-cartridgembc-handling)
7. [Timer Subsystem](#7-timer-subsystem)
8. [Testing Strategies](#8-testing-strategies)
9. [Rust-Specific Idioms](#9-rust-specific-idioms)
10. [Frontend/Rendering](#10-frontendrendering)
11. [Sources](#sources)

---

## 1. Existing Rust GB Emulators

### Notable Projects

| Project | Repository | Highlights |
|---------|-----------|------------|
| **rboy** | [mvdnes/rboy](https://github.com/mvdnes/rboy) | Mature, GBC support, MBC1/3/5, serial/printer emulation, GBEmulatorShootout integration |
| **Boytacean** | [joamag/boytacean](https://github.com/joamag/boytacean) | Multi-frontend (Web/SDL/Libretro), WASM support, PyO3 Python bindings, BESS save states, passes dmg-acid2/cgb-acid2 |
| **DMG-01** | [rylev/DMG-01](https://github.com/rylev/DMG-01) | Excellent educational resource with full companion book, clean architecture, beginner-friendly |
| **mohanson/gameboy** | [mohanson/gameboy](https://github.com/mohanson/gameboy) | Full-featured, cross-platform, uses blip_buf for audio synthesis |
| **Mooneye GB** | [Gekkio/mooneye-gb](https://github.com/Gekkio/mooneye-gb) | Research-focused, writes custom test ROMs, gold standard for hardware accuracy research |
| **gib** | [plorefice/gib](https://github.com/plorefice/gib) | Clean Rust implementation |
| **gb-rs** | [simias/gb-rs](https://github.com/simias/gb-rs) | Detailed commentary on edge cases, good for learning |
| **raphamorim/gameboy** | [raphamorim/gameboy](https://github.com/raphamorim/gameboy) | Runs on Terminal, Web, and Desktop |
| **Mimic** | [jawline/Mimic](https://github.com/jawline/Mimic) | Rust Gameboy emulator with clean code |

### Common Architecture Patterns

Most Rust GB emulators follow a similar high-level module structure:

```text
src/
  cpu.rs          -- CPU (SM83) emulation, registers, instruction execution
  registers.rs    -- Register definitions, flag register, 16-bit pairs
  instructions.rs -- Instruction enum/table, decode logic
  mmu.rs / bus.rs -- Memory bus, address routing, I/O register dispatch
  ppu.rs / gpu.rs -- Picture Processing Unit, tile/sprite rendering
  apu.rs          -- Audio Processing Unit, channel emulation
  timer.rs        -- DIV/TIMA/TMA/TAC timer system
  cartridge.rs    -- ROM loading, MBC dispatch
  mbc/            -- Individual MBC implementations (mbc1.rs, mbc3.rs, mbc5.rs)
  joypad.rs       -- Input handling
  interrupts.rs   -- Interrupt controller (IE, IF, IME)
  serial.rs       -- Serial link cable
  gameboy.rs      -- Top-level system struct tying everything together
  main.rs         -- Frontend entry point
```

**Key architectural decision**: Separate the core emulation library from the frontend. This enables `no_std` compatibility and multiple frontends (native, WASM, Libretro). Boytacean demonstrates this with a workspace of multiple crates. The core library handles emulation; frontend crates handle windowing, audio output, and input.

---

## 2. CPU Emulation Patterns

### The SM83 (Sharp LR35902) CPU

The Gameboy CPU is officially based on the Sharp SM83 core (previously misidentified as LR35902). It is an 8-bit CPU with a 16-bit address bus, with an ISA derived from both the Zilog Z80 and Intel 8080. There are 256 base opcodes plus 256 CB-prefixed opcodes (512 total instructions).

### Register Modeling

**Approach 1: Struct with individual fields (most common)**

```rust
struct Registers {
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    f: FlagsRegister,  // or u8
    h: u8,
    l: u8,
    pc: u16,
    sp: u16,
}
```

**Virtual 16-bit register pairs** (AF, BC, DE, HL) are implemented via getter/setter methods using bit shifting:

```rust
impl Registers {
    fn bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }
    fn set_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = value as u8;
    }
}
```

**Flags register**: Rather than raw u8, many implementations use a dedicated struct with `From<u8>` / `Into<u8>` conversions:

```rust
struct FlagsRegister {
    zero: bool,       // Z flag, bit 7
    subtract: bool,   // N flag, bit 6
    half_carry: bool,  // H flag, bit 5
    carry: bool,      // C flag, bit 4
}

impl From<FlagsRegister> for u8 {
    fn from(flag: FlagsRegister) -> u8 {
        (if flag.zero { 1 } else { 0 }) << 7
            | (if flag.subtract { 1 } else { 0 }) << 6
            | (if flag.half_carry { 1 } else { 0 }) << 5
            | (if flag.carry { 1 } else { 0 }) << 4
    }
}
```

The lower 4 bits of the F register always read as zero, even if written with ones.

### Opcode Dispatch Strategies

**Strategy 1: Giant match statement (most common, recommended)**

```rust
fn execute(&mut self, opcode: u8) -> u8 {
    match opcode {
        0x00 => { /* NOP */ 4 }
        0x01 => { /* LD BC, d16 */ self.ld_r16_d16(Reg16::BC); 12 }
        // ... 256 arms
        0xCB => { self.execute_cb() }
        _ => panic!("Unknown opcode: {:#04x}", opcode),
    }
}
```

Pros: Simple, compiler can optimize, easy to debug, explicit cycle counts.
Cons: Verbose (~500+ lines), repetitive.

**Strategy 2: Instruction enum with from_byte decode**

```rust
enum Instruction {
    NOP,
    LD(LoadTarget, LoadSource),
    ADD(ArithmeticTarget),
    JP(JumpCondition, u16),
    // ...
}

impl Instruction {
    fn from_byte(byte: u8, prefixed: bool) -> Option<Instruction> {
        if prefixed {
            Instruction::from_byte_prefixed(byte)
        } else {
            Instruction::from_byte_not_prefixed(byte)
        }
    }
}
```

Pros: More semantic, fewer instruction handler functions needed, pattern matching on instruction types rather than raw opcodes.
Cons: Additional indirection layer, can be harder to get cycle counts exactly right.

**Strategy 3: Macro/code generation (advanced)**

Used by Yushi Omote's emulator: 52 hand-written macros generate ~7000 lines for all 501 instructions using Tera templates. A PEG grammar parses specification data from web resources to auto-generate the opcode table.

Pros: Minimal hand-written code, less error-prone for large instruction sets.
Cons: Higher setup complexity, harder to debug generated code.

**Strategy 4: Opcode pattern matching with masks**

```rust
pub struct OpCode(u8, u8);  // (template, mask)
// LD r,r: template = 0b01000000, mask = 0b11000000
// matches when (byte & mask) == template
```

Pros: Compact, handles families of instructions elegantly.
Cons: Less obvious, harder for newcomers.

**Community recommendation**: Strategy 1 (giant match) for simplicity and debuggability. Strategy 2 for cleaner architecture if you invest in the enum design upfront.

### CB-Prefixed Instructions

CB-prefixed opcodes are the second set of 256 instructions accessed when the CPU reads byte `0xCB`. The execution flow:

```rust
fn step(&mut self) -> u8 {
    let opcode = self.read_byte(self.pc);
    self.pc += 1;

    if opcode == 0xCB {
        let cb_opcode = self.read_byte(self.pc);
        self.pc += 1;
        self.execute_cb(cb_opcode)
    } else {
        self.execute(opcode)
    }
}
```

CB instructions follow a regular encoding pattern. The upper 2 bits select the operation, the next 3 bits select the bit index (for BIT/SET/RES), and the lower 3 bits select the register:

```text
Bits 7-6: Operation type
  00 = Rotate/Shift (RLC, RRC, RL, RR, SLA, SRA, SWAP, SRL)
  01 = BIT (test bit)
  10 = RES (reset bit)
  11 = SET (set bit)

Bits 5-3: Bit index (0-7) for BIT/SET/RES, or shift variant for 00
Bits 2-0: Register (B=0, C=1, D=2, E=3, H=4, L=5, (HL)=6, A=7)
```

This regularity means CB instructions can be decoded programmatically rather than with 256 match arms:

```rust
fn execute_cb(&mut self, opcode: u8) -> u8 {
    let reg = opcode & 0x07;
    let bit = (opcode >> 3) & 0x07;

    match opcode & 0xC0 {
        0x00 => self.cb_rotate_shift(bit, reg),
        0x40 => self.cb_bit(bit, reg),
        0x80 => self.cb_res(bit, reg),
        0xC0 => self.cb_set(bit, reg),
        _ => unreachable!(),
    }
}
```

### Arithmetic Overflow Handling

Rust forces explicit overflow handling, which is actually beneficial for emulation:

```rust
let (result, carry) = self.registers.a.overflowing_add(value);
let half_carry = (self.registers.a & 0x0F) + (value & 0x0F) > 0x0F;
```

Use `overflowing_add`, `overflowing_sub`, `wrapping_add`, `wrapping_sub` instead of raw operators.

### Interrupt Handling

The interrupt system uses three key registers:

- **IME** (Interrupt Master Enable): Global interrupt on/off, not memory-mapped
- **IE** (0xFFFF): Interrupt enable flags per interrupt type
- **IF** (0xFF0F): Interrupt request flags

Key edge cases:

- `EI` instruction delays IME enable by one instruction
- **HALT bug**: When HALT executes with IME=0 and (IE & IF) != 0, the PC fails to increment after HALT, causing the next instruction to execute twice
- Interrupt dispatch takes 20 T-cycles (5 M-cycles)

```rust
fn handle_interrupts(&mut self) -> u8 {
    if !self.ime { return 0; }

    let triggered = self.ie & self.if_reg & 0x1F;
    if triggered == 0 { return 0; }

    self.ime = false;
    self.halted = false;

    // Find highest priority interrupt (lowest bit)
    let interrupt = triggered.trailing_zeros();
    self.if_reg &= !(1 << interrupt);

    // Push PC onto stack
    self.sp = self.sp.wrapping_sub(2);
    self.write_word(self.sp, self.pc);

    // Jump to interrupt vector
    self.pc = match interrupt {
        0 => 0x0040, // VBlank
        1 => 0x0048, // LCD STAT
        2 => 0x0050, // Timer
        3 => 0x0058, // Serial
        4 => 0x0060, // Joypad
        _ => unreachable!(),
    };

    20 // cycles consumed
}
```

---

## 3. Memory Bus Architecture

### Gameboy Memory Map

The Gameboy has a 16-bit address bus (64KB addressable):

```text
0x0000 - 0x00FF  Boot ROM (unmapped after boot)
0x0000 - 0x3FFF  ROM Bank 0 (16KB, always mapped)
0x4000 - 0x7FFF  ROM Bank 1-N (16KB, switchable via MBC)
0x8000 - 0x9FFF  Video RAM (8KB)
  0x8000 - 0x97FF  Tile Data (6KB, 384 tiles)
  0x9800 - 0x9BFF  BG Tile Map 1 (1KB)
  0x9C00 - 0x9FFF  BG Tile Map 2 (1KB)
0xA000 - 0xBFFF  External/Cartridge RAM (8KB, switchable)
0xC000 - 0xDFFF  Work RAM (8KB)
0xE000 - 0xFDFF  Echo RAM (mirror of 0xC000-0xDDFF, do not use)
0xFE00 - 0xFE9F  OAM (Sprite Attribute Table, 160 bytes)
0xFEA0 - 0xFEFF  Unusable region
0xFF00 - 0xFF7F  I/O Registers
0xFF80 - 0xFFFE  High RAM (HRAM, 127 bytes)
0xFFFF           Interrupt Enable (IE) register
```

### Implementation Approaches

**Approach 1: Direct match on address ranges (most common)**

```rust
struct Bus {
    boot_rom: [u8; 256],
    cartridge: Cartridge,
    vram: [u8; 0x2000],
    wram: [u8; 0x2000],
    oam: [u8; 0xA0],
    io: IoRegisters,
    hram: [u8; 0x7F],
    ie: u8,
    boot_rom_mapped: bool,
}

impl Bus {
    fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00FF if self.boot_rom_mapped => self.boot_rom[addr as usize],
            0x0000..=0x7FFF => self.cartridge.read(addr),
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xA000..=0xBFFF => self.cartridge.read_ram(addr),
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize], // Echo
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],
            0xFEA0..=0xFEFF => 0xFF, // Unusable
            0xFF00..=0xFF7F => self.io.read(addr),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.ie,
        }
    }
}
```

Pros: Simple, fast, no dynamic dispatch overhead.
Cons: Bus struct needs references to all subsystems.

**Approach 2: Trait-based handler registration**

Used by Yushi Omote's emulator:

```rust
trait MemHandler {
    fn on_read(&self, mmu: &Mmu, addr: u16) -> MemRead;
    fn on_write(&self, mmu: &Mmu, addr: u16, value: u8) -> MemWrite;
}

enum MemRead {
    Replace(u8),  // Override the value
    PassThrough,  // Use underlying byte array
}

enum MemWrite {
    Replace(u8),  // Override what gets written
    PassThrough,  // Write the given value
    Block,        // Prevent the write
}
```

Handlers are registered to address ranges and chain sequentially. This decouples I/O devices from the bus implementation and supports debugging interceptors.

Pros: Highly modular, devices are self-contained, supports chaining/intercepting.
Cons: More complex, potential performance overhead from dynamic dispatch.

**Approach 3: Component reference struct**

```rust
struct Bus<'a> {
    ppu: &'a mut Ppu,
    apu: &'a mut Apu,
    timer: &'a mut Timer,
    cartridge: &'a mut Cartridge,
    // ...
}
```

Pros: Avoids interior mutability, direct field access.
Cons: Lifetime management can be complex in Rust.

**Community recommendation**: Approach 1 for simplicity. The monolithic match approach is by far the most common in Rust GB emulators because it avoids lifetime/borrowing headaches with Rust's ownership model. Approach 2 if you want maximum modularity and are comfortable with `Rc<RefCell<>>` or similar patterns.

### Memory-Mapped I/O Registers

I/O registers (0xFF00-0xFF7F) require special handling as reads/writes trigger hardware behavior:

```text
0xFF00  P1/JOYP  - Joypad
0xFF01  SB       - Serial transfer data
0xFF02  SC       - Serial transfer control
0xFF04  DIV      - Divider register
0xFF05  TIMA     - Timer counter
0xFF06  TMA      - Timer modulo
0xFF07  TAC      - Timer control
0xFF0F  IF       - Interrupt flags
0xFF10-0xFF3F    - Audio registers (APU)
0xFF40  LCDC     - LCD control
0xFF41  STAT     - LCD status
0xFF42  SCY      - Scroll Y
0xFF43  SCX      - Scroll X
0xFF44  LY       - Current scanline
0xFF45  LYC      - LY compare
0xFF46  DMA      - OAM DMA transfer
0xFF47  BGP      - BG palette data
0xFF48  OBP0     - Object palette 0
0xFF49  OBP1     - Object palette 1
0xFF4A  WY       - Window Y position
0xFF4B  WX       - Window X position
```

These are typically dispatched to subsystem modules:

```rust
fn io_read(&self, addr: u16) -> u8 {
    match addr {
        0xFF00 => self.joypad.read(),
        0xFF04..=0xFF07 => self.timer.read(addr),
        0xFF0F => self.interrupt_flags,
        0xFF10..=0xFF3F => self.apu.read(addr),
        0xFF40..=0xFF4B => self.ppu.read(addr),
        0xFF46 => self.dma.read(),
        _ => 0xFF, // Unused registers return 0xFF
    }
}
```

---

## 4. PPU (Picture Processing Unit)

### Rendering Approaches

There are three main strategies, ranging from simplest to most accurate:

#### Approach 1: Full Framebuffer / Per-Frame Rendering

Render the entire 160x144 frame at once from VRAM state when VBlank begins.

- **Simplicity**: Highest
- **Accuracy**: Lowest (misses mid-scanline effects)
- **When to use**: Initial prototyping only
- **Limitations**: Cannot handle any raster effects, breaks games that modify scroll/palette mid-frame

#### Approach 2: Scanline-Based Rendering (Recommended Starting Point)

Render one complete scanline (160 pixels) at the end of Mode 3 or beginning of HBlank for each of the 144 visible scanlines.

```rust
fn render_scanline(&mut self) {
    let ly = self.ly;
    self.render_bg_scanline(ly);
    self.render_window_scanline(ly);
    self.render_sprites_scanline(ly);
}
```

- **Simplicity**: Medium
- **Accuracy**: Good (handles most games correctly)
- **When to use**: Primary implementation for most emulators
- **Limitations**: Cannot handle mid-scanline palette changes, mid-scanline scroll changes, or Mode 3 timing-dependent tricks

**Community recommendation**: This is what most emulators use. The vast majority of commercial games work correctly with scanline rendering.

#### Approach 3: Pixel FIFO (Cycle-Accurate)

Emulates the actual PPU hardware pipeline: a pixel fetcher fills a FIFO, the LCD shifts pixels out one at a time.

```rust
enum PpuState {
    OamScan { dot: u16 },
    RenderingBgTile { fields: RenderingBgTileFields },
    SpriteFetch { fields: SpriteFetchFields },
    HBlank { dot: u16 },
    VBlank { dot: u16 },
}

struct PixelFifo {
    bg: VecDeque<BgPixel>,
    sprites: VecDeque<SpritePixel>,
    scanned_sprites: Vec<OamEntry>,
}
```

- **Simplicity**: Lowest (significantly more complex)
- **Accuracy**: Highest (handles all known edge cases)
- **When to use**: When targeting maximum hardware accuracy
- **Notable games that need FIFO**: Prehistorik Man (mid-scanline palette changes), "Is That a Demo in Your Pocket?" demo

**Key FIFO details from jsgroth's blog**:

- Background and sprite FIFOs are maintained separately (hardware keeps separate queues, they do not merge early)
- Each tile fetch takes 6 cycles (2 cycles per step: tile number, low data byte, high data byte)
- Sprite fetches add 6-11 cycles per sprite to rendering
- Minimum rendering time: 174 cycles (includes offscreen tile fetch and first 8-pixel discard)
- The Window Y trigger should activate whenever WY matches the scanline, regardless of window enable state

### PPU Timing

```text
Frame: 154 scanlines x 456 dots = 70,224 dots (~59.7 FPS)

Per visible scanline (0-143):
  Mode 2 (OAM Scan):    80 dots
  Mode 3 (Drawing):     172-289 dots (variable)
  Mode 0 (HBlank):      87-204 dots (fills remainder to 456)

Scanlines 144-153:
  Mode 1 (VBlank):      4560 dots total (10 scanlines)

One "dot" = 1 T-cycle at 4.194304 MHz
```

Mode 3 duration varies based on:

1. SCX % 8 additional dots for initial scroll offset
2. 6-dot penalty when Window activates mid-scanline
3. 6-11 dot penalty per sprite encountered

### Tile Data Encoding

Each tile is 8x8 pixels, 2 bits per pixel (4 colors). Each row uses 2 bytes:

```rust
fn decode_tile_row(byte1: u8, byte2: u8) -> [u8; 8] {
    let mut pixels = [0u8; 8];
    for bit in 0..8 {
        let lo = (byte1 >> (7 - bit)) & 1;
        let hi = (byte2 >> (7 - bit)) & 1;
        pixels[bit] = (hi << 1) | lo;
    }
    pixels
}
```

### OAM Sprite Handling

- 40 sprites in OAM (0xFE00-0xFE9F), 4 bytes each
- Maximum 10 sprites per scanline
- Sprite priority: lower X position wins; on tie, lower OAM index wins
- During Mode 2, the PPU scans OAM for sprites overlapping the current scanline

---

## 5. APU (Audio Processing Unit)

### Channel Architecture

The Gameboy APU has 4 channels:

| Channel | Type | Features |
|---------|------|----------|
| CH1 | Pulse/Square wave | Frequency sweep, envelope, 4 duty cycles |
| CH2 | Pulse/Square wave | Envelope, 4 duty cycles (no sweep) |
| CH3 | Wave | Custom 32-sample waveform (4-bit), programmable via Wave RAM |
| CH4 | Noise | LFSR-based, envelope, 7-bit or 15-bit shift register |

### Frame Sequencer

Operates at 512 Hz, drives modulation units:

```text
Step  Length  Envelope  Sweep
0     Clock   -         -
1     -       -         -
2     Clock   -         Clock
3     -       -         -
4     Clock   -         -
5     -       -         -
6     Clock   -         Clock
7     -       Clock     -
```

Length counters tick at 256 Hz (every other step), envelope at 64 Hz (step 7 only), sweep at 128 Hz (steps 2 and 6).

### Pulse Channel Implementation

```rust
struct PulseChannel {
    enabled: bool,
    duty: u8,           // 0-3 selects duty cycle pattern
    phase: u8,          // 0-7 position in duty cycle
    frequency: u16,     // 11-bit frequency value
    timer: u16,         // Countdown timer
    volume: u8,         // Current volume (0-15)
    envelope: Envelope,
    length_counter: LengthCounter,
    sweep: Option<FrequencySweep>, // Only CH1
}

const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1], // 12.5%
    [1, 0, 0, 0, 0, 0, 0, 1], // 25%
    [1, 0, 0, 0, 0, 1, 1, 1], // 50%
    [0, 1, 1, 1, 1, 1, 1, 0], // 75%
];

impl PulseChannel {
    fn tick(&mut self) {
        self.timer = self.timer.wrapping_sub(1);
        if self.timer == 0 {
            self.timer = (2048 - self.frequency) * 4;
            self.phase = (self.phase + 1) & 7;
        }
    }

    fn sample(&self) -> u8 {
        if !self.enabled { return 0; }
        DUTY_TABLE[self.duty as usize][self.phase as usize] * self.volume
    }
}
```

### Mixing and Output

All 4 channels produce 4-bit samples (0-15). The mixer:

1. Routes each channel to left/right based on NR51 register bits
2. Applies master volume from NR50 register (3-bit per side, 0-7)
3. Combines via simple addition

```rust
fn mix(&self) -> (f32, f32) {
    let mut left = 0.0f32;
    let mut right = 0.0f32;

    for (i, sample) in self.channel_samples.iter().enumerate() {
        let s = *sample as f32 / 15.0;
        if self.nr51 & (1 << (i + 4)) != 0 { left += s; }
        if self.nr51 & (1 << i) != 0 { right += s; }
    }

    let left_vol = ((self.nr50 >> 4) & 0x07) as f32 / 7.0;
    let right_vol = (self.nr50 & 0x07) as f32 / 7.0;

    (left * left_vol * 0.25, right * right_vol * 0.25)
}
```

### Resampling

The APU runs at ~1 MHz internally but audio output is typically 44.1 kHz or 48 kHz. A low-pass FIR filter is recommended before downsampling to prevent aliasing artifacts. The `blip_buf` crate is commonly used for band-limited synthesis.

### Audio Backend Libraries

| Library | Level | Description | Used By |
|---------|-------|-------------|---------|
| **cpal** | Low | Pure Rust, cross-platform PCM output, supports ALSA/CoreAudio/WASAPI | Most Rust emulators |
| **rodio** | High | Built on cpal, easier API with decoding/playback | Some emulators |
| **blip_buf** | Synthesis | Band-limited buffer for delta-based sample generation | mohanson/gameboy |
| **SDL2 audio** | Low | Part of SDL2 bindings, callback-based | rboy, emulators using SDL2 frontend |

**Community recommendation**: `cpal` for pure Rust projects, SDL2 audio if already using SDL2 for video/input. `blip_buf` for high-quality sample synthesis.

---

## 6. Cartridge/MBC Handling

### MBC Types and Market Coverage

| MBC Type | Games | Features |
|----------|-------|----------|
| ROM Only | Simple games (Tetris) | No banking |
| MBC1 | ~25% of games | 2MB ROM, 32KB RAM, banking modes |
| MBC2 | Few games | 256KB ROM, built-in 512x4 RAM |
| MBC3 | ~20% of games | 2MB ROM, 32KB RAM, Real-Time Clock |
| MBC5 | ~40% of games (GBC era) | 8MB ROM, 128KB RAM, rumble support |

Implementing MBC1, MBC3, and MBC5 covers the vast majority of commercial titles.

### Cartridge Type Detection

The cartridge header at address 0x0147 identifies the MBC type:

```rust
fn detect_mbc(header_byte: u8) -> MbcType {
    match header_byte {
        0x00 => MbcType::RomOnly,
        0x01..=0x03 => MbcType::Mbc1,
        0x05..=0x06 => MbcType::Mbc2,
        0x0F..=0x13 => MbcType::Mbc3,
        0x19..=0x1E => MbcType::Mbc5,
        _ => MbcType::Unknown,
    }
}
```

### Implementation Approaches

**Approach 1: Trait-based polymorphism (recommended)**

```rust
trait Mbc {
    fn read_rom(&self, addr: u16) -> u8;
    fn write_rom(&mut self, addr: u16, value: u8); // Bank switching
    fn read_ram(&self, addr: u16) -> u8;
    fn write_ram(&mut self, addr: u16, value: u8);
}

struct Mbc1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_bank: u8,
    ram_bank: u8,
    ram_enabled: bool,
    banking_mode: bool, // 0 = ROM mode, 1 = RAM mode
}

impl Mbc for Mbc1 {
    fn write_rom(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = (value & 0x0F) == 0x0A,
            0x2000..=0x3FFF => {
                let bank = value & 0x1F;
                self.rom_bank = if bank == 0 { 1 } else { bank };
            }
            0x4000..=0x5FFF => self.ram_bank = value & 0x03,
            0x6000..=0x7FFF => self.banking_mode = (value & 1) != 0,
            _ => {}
        }
    }
    // ...
}
```

Used by `Box<dyn Mbc>` for runtime polymorphism. Simple, extensible, one file per MBC type.

**Approach 2: Enum dispatch (zero-cost)**

```rust
enum Cartridge {
    RomOnly(RomOnly),
    Mbc1(Mbc1),
    Mbc3(Mbc3),
    Mbc5(Mbc5),
}

impl Cartridge {
    fn read(&self, addr: u16) -> u8 {
        match self {
            Cartridge::RomOnly(c) => c.read(addr),
            Cartridge::Mbc1(c) => c.read(addr),
            Cartridge::Mbc3(c) => c.read(addr),
            Cartridge::Mbc5(c) => c.read(addr),
        }
    }
}
```

Pros: No vtable overhead, compiler can inline.
Cons: Adding new MBC types requires touching the enum and all match arms.

**Community recommendation**: Trait-based with `Box<dyn Mbc>` is more common and cleaner. The dynamic dispatch overhead is negligible compared to memory access patterns. Enum dispatch if you want to avoid heap allocation and maximize inlining.

### MBC1 Bank Switching Details

```text
Address Range     Write Effect
0x0000-0x1FFF     RAM Enable (0x0A enables, other values disable)
0x2000-0x3FFF     ROM Bank Number (lower 5 bits, 0 maps to 1)
0x4000-0x5FFF     RAM Bank / Upper ROM bits (2 bits)
0x6000-0x7FFF     Banking Mode (0=ROM, 1=RAM)

ROM Bank 0 Read: Always from physical bank 0 (except in advanced mode)
ROM Bank N Read: 0x4000 + (addr - 0x4000) + (bank * 0x4000)
```

---

## 7. Timer Subsystem

### Register Overview

| Register | Address | Description |
|----------|---------|-------------|
| DIV | 0xFF04 | Divider register, increments at 16384 Hz, any write resets to 0 |
| TIMA | 0xFF05 | Timer counter, increments at frequency set by TAC, generates interrupt on overflow |
| TMA | 0xFF06 | Timer modulo, value loaded into TIMA on overflow |
| TAC | 0xFF07 | Timer control (bit 2 = enable, bits 1-0 = clock select) |

### Clock Select (TAC bits 1-0)

| Value | Frequency | CPU Clock Divider | DIV Bit Monitored |
|-------|-----------|-------------------|-------------------|
| 0b00 | 4096 Hz | / 1024 | Bit 9 |
| 0b01 | 262144 Hz | / 16 | Bit 3 |
| 0b10 | 65536 Hz | / 64 | Bit 5 |
| 0b11 | 16384 Hz | / 256 | Bit 7 |

### Internal Mechanism

The timer is actually driven by a 16-bit internal counter (the "system counter" or "DIV counter") that increments every T-cycle. The DIV register at 0xFF04 exposes the upper 8 bits (bits 8-15).

The TIMA increment uses a **falling edge detector**:

```text
1. Select a bit from the system counter based on TAC clock select
2. AND this bit with the TAC timer-enable flag
3. TIMA increments on a falling edge (1 -> 0) of this AND result
```

### Implementation

```rust
struct Timer {
    div_counter: u16,      // Internal 16-bit counter, DIV = upper 8 bits
    tima: u8,
    tma: u8,
    tac: u8,
    prev_and_result: bool, // For falling edge detection
    overflow_countdown: i8, // For delayed reload (-1 = inactive)
}

impl Timer {
    fn tick(&mut self, cycles: u8) -> bool {
        let mut interrupt = false;

        for _ in 0..cycles {
            self.div_counter = self.div_counter.wrapping_add(1);

            // Handle delayed TIMA reload
            if self.overflow_countdown > 0 {
                self.overflow_countdown -= 1;
                if self.overflow_countdown == 0 {
                    self.tima = self.tma;
                    interrupt = true;
                    self.overflow_countdown = -1;
                }
            }

            // Falling edge detection
            let bit_pos = match self.tac & 0x03 {
                0 => 9,
                1 => 3,
                2 => 5,
                3 => 7,
                _ => unreachable!(),
            };
            let timer_enabled = (self.tac & 0x04) != 0;
            let bit = (self.div_counter >> bit_pos) & 1 != 0;
            let and_result = bit && timer_enabled;

            // Falling edge: was 1, now 0
            if self.prev_and_result && !and_result {
                self.tima = self.tima.wrapping_add(1);
                if self.tima == 0 {
                    // Overflow: reload delayed by 1 M-cycle (4 T-cycles)
                    self.overflow_countdown = 4;
                }
            }
            self.prev_and_result = and_result;
        }

        interrupt
    }

    fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (self.div_counter >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac | 0xF8, // Upper 5 bits read as 1
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF04 => {
                // Writing ANY value resets the entire 16-bit counter
                // This can trigger TIMA increment via falling edge!
                self.div_counter = 0;
            }
            0xFF05 => {
                // Writing during overflow countdown can cancel reload
                if self.overflow_countdown <= 0 {
                    self.tima = value;
                }
                // If written on same cycle as reload, write is ignored
            }
            0xFF06 => self.tma = value,
            0xFF07 => self.tac = value,
            _ => {}
        }
    }
}
```

### Obscure Timer Behaviors (Important for Accuracy)

1. **DIV write triggers TIMA**: Writing to DIV resets the system counter to 0. If the monitored bit was 1, this causes a falling edge, incrementing TIMA.

2. **TAC clock select change**: Changing the clock select bits can trigger a spurious TIMA increment if the previously-selected bit was 1 and the newly-selected bit is 0.

3. **TIMA overflow delay**: TIMA reads as 0x00 for 4 T-cycles (1 M-cycle) after overflow, then TMA is loaded and the timer interrupt flag is set.

4. **Write-during-reload edge cases**:
   - Writing to TIMA during the 4-cycle overflow window cancels the reload and prevents the interrupt
   - Writing to TIMA on the exact cycle of reload: the write is ignored, TMA value wins
   - Writing to TMA during reload: the new TMA value is simultaneously loaded into TIMA

---

## 8. Testing Strategies

### Standard Test ROM Suites

#### Tier 1: Essential (implement these first)

| Suite | Tests | Purpose | Source |
|-------|-------|---------|--------|
| **Blargg's cpu_instrs** | 11 tests | CPU instruction correctness | [c-sp/game-boy-test-roms](https://github.com/c-sp/game-boy-test-roms) |
| **Blargg's instr_timing** | 1 test | CPU instruction cycle timing | Same |
| **dmg-acid2** | 1 test | PPU rendering accuracy (visual) | [mattcurrie/dmg-acid2](https://github.com/mattcurrie/dmg-acid2) |
| **Blargg's mem_timing** | 3 tests | Memory access timing | Same |

#### Tier 2: Intermediate

| Suite | Tests | Purpose |
|-------|-------|---------|
| **Mooneye Test Suite** | ~80 tests | Hardware accuracy (CPU, timer, PPU, MBC) |
| **Blargg's halt_bug** | 1 test | HALT instruction edge cases |
| **Blargg's dmg_sound** | 12 tests | APU accuracy |
| **Gambatte test suite** | 100+ tests | Comprehensive accuracy |

#### Tier 3: Advanced/Exhaustive

| Suite | Purpose |
|-------|---------|
| **GBMicrotest** | Sub-instruction-level timing |
| **Mealybug Tearoom Tests** | PPU edge cases, mid-scanline effects |
| **AGE test roms** | Additional accuracy tests |
| **Scribbltests / Strikethrough** | PPU rendering edge cases |
| **MBC3 Tester / rtc3test** | RTC accuracy for MBC3 |
| **SameSuite** | APU-specific tests |

### Testing Methodology

**Progressive accuracy approach**:

1. Implement basic CPU and get `blargg/cpu_instrs` passing (covers ~80% of games)
2. Fix instruction timing with `blargg/instr_timing`
3. Validate PPU with `dmg-acid2` (visual comparison test)
4. Address timer accuracy with Mooneye timer tests
5. Validate APU with `blargg/dmg_sound`
6. Pursue Mooneye acceptance tests for high accuracy
7. Use GBEmulatorShootout for comparative benchmarking

**Mooneye test success criteria**: CPU registers must contain Fibonacci values after test completion:
B=3, C=5, D=8, E=13, H=21, L=34. Each test executes opcode 0x40 (LD B,B) when finished. Maximum runtime: 120 emulated seconds.

**Important**: Always record which version of test ROMs you are testing against, as tests evolve over time. The [GBEmulatorShootout](https://daid.github.io/GBEmulatorShootout/) provides a standardized comparison framework.

### Automated Testing in Rust

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn run_test_rom(path: &str, max_cycles: u64) -> Gameboy {
        let rom = std::fs::read(path).unwrap();
        let mut gb = Gameboy::new(rom);

        for _ in 0..max_cycles {
            gb.step();
            // Check for test completion (LD B,B opcode)
            if gb.cpu.last_opcode == 0x40 {
                break;
            }
        }
        gb
    }

    #[test]
    fn test_blargg_cpu_instrs() {
        let gb = run_test_rom("tests/roms/cpu_instrs.gb", 100_000_000);
        // Check serial output for "Passed" or check registers
        assert!(gb.serial_output.contains("Passed"));
    }
}
```

---

## 9. Rust-Specific Idioms

### Enum State Machines

Rust enums are ideal for modeling PPU modes, CPU states, and other finite state machines:

```rust
enum PpuMode {
    OamScan,
    Drawing,
    HBlank,
    VBlank,
}

enum CpuState {
    Running,
    Halted,
    Stopped,
}
```

The compiler enforces exhaustive matching, preventing missed state transitions.

### Bitfield Manipulation

**Option A: Manual bit operations (most common)**

```rust
fn bit(value: u8, n: u8) -> bool { (value >> n) & 1 != 0 }
fn set_bit(value: &mut u8, n: u8) { *value |= 1 << n; }
fn clear_bit(value: &mut u8, n: u8) { *value &= !(1 << n); }
```

**Option B: The `bitflags` crate**

```rust
use bitflags::bitflags;

bitflags! {
    struct LcdControl: u8 {
        const BG_ENABLE     = 0b0000_0001;
        const OBJ_ENABLE    = 0b0000_0010;
        const OBJ_SIZE      = 0b0000_0100;
        const BG_TILE_MAP   = 0b0000_1000;
        const TILE_DATA     = 0b0001_0000;
        const WINDOW_ENABLE = 0b0010_0000;
        const WINDOW_MAP    = 0b0100_0000;
        const LCD_ENABLE    = 0b1000_0000;
    }
}
```

**Community recommendation**: Manual bit operations for register fields, `bitflags` for complex flag registers with named semantics (LCDC, STAT, TAC, interrupt flags).

### Type Aliases for Clarity

```rust
type Address = u16;
type Byte = u8;
type Word = u16;
type Cycles = u32;
```

### Avoiding Unnecessary Trait Objects

Prefer static dispatch where the set of types is known at compile time:

```rust
// Prefer this (enum dispatch, zero-cost):
enum Cartridge {
    RomOnly(RomOnly),
    Mbc1(Mbc1),
    Mbc3(Mbc3),
}

// Over this (dynamic dispatch, heap allocation):
struct Bus {
    cartridge: Box<dyn Mbc>,
}
```

Exception: `Box<dyn Mbc>` is acceptable when you want easier extensibility and the dispatch overhead is negligible.

### Explicit Overflow Handling

Rust's overflow checking is an advantage:

```rust
// Explicit wrapping (no panic in debug mode)
let result = a.wrapping_add(b);
let result = sp.wrapping_sub(2);

// Get carry information
let (result, carry) = a.overflowing_add(b);

// Half-carry detection
let half_carry = (a & 0x0F) + (b & 0x0F) > 0x0F;
```

### Interior Mutability Patterns

When multiple subsystems need shared mutable access (e.g., interrupt flags):

```rust
use std::cell::RefCell;
use std::rc::Rc;

// Shared interrupt flags
let interrupt_flags = Rc::new(RefCell::new(0u8));

// Timer can set flags
timer.interrupt_flags = Rc::clone(&interrupt_flags);
// PPU can set flags
ppu.interrupt_flags = Rc::clone(&interrupt_flags);
```

Alternative: Collect pending interrupts as return values and let the top-level system struct combine them.

### `no_std` Compatibility

For WASM and embedded targets, structure the core library to avoid `std`:

```rust
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::vec::Vec;
use alloc::boxed::Box;
```

This enables compilation to WASM without `std` while keeping `Vec`, `Box`, etc. via `alloc`.

### Const Arrays for Lookup Tables

```rust
const CYCLE_TABLE: [u8; 256] = [
    4, 12, 8, 8, 4, 4, 8, 4, 20, 8, 8, 8, 4, 4, 8, 4,
    // ... 256 entries for base opcodes
];

const CB_CYCLE_TABLE: [u8; 256] = [
    8, 8, 8, 8, 8, 8, 16, 8, // CB00-CB07
    // ... 256 entries for CB-prefixed opcodes
];
```

---

## 10. Frontend/Rendering

### Library Comparison

| Library | Type | GPU Accel | Audio | Input | WASM | Dependencies |
|---------|------|-----------|-------|-------|------|-------------|
| **pixels + winit** | Framebuffer | Yes (wgpu) | No (need cpal) | Via winit | Planned | Medium (~500K SLoC) |
| **SDL2** (rust-sdl2) | Full framework | Yes (OpenGL) | Yes (built-in) | Yes (built-in) | No | Heavy (C dependency) |
| **minifb** | Simple framebuffer | Partial | No | Basic | No | Light |
| **wgpu** (direct) | Low-level GPU | Yes | No | No | Yes | Heavy |
| **macroquad** | Game framework | Yes | Yes | Yes | Yes | Medium |

### Detailed Tradeoffs

#### pixels + winit + cpal

```rust
use pixels::{Pixels, SurfaceTexture};
use winit::event_loop::EventLoop;

let event_loop = EventLoop::new();
let window = WindowBuilder::new().build(&event_loop).unwrap();
let surface = SurfaceTexture::new(width, height, &window);
let mut pixels = Pixels::new(160, 144, surface).unwrap();

// In render loop:
let frame = pixels.frame_mut(); // &mut [u8] RGBA
for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
    let x = i % 160;
    let y = i / 160;
    let color = gb.framebuffer[y][x];
    pixel.copy_from_slice(&palette[color]);
}
pixels.render().unwrap();
```

**Pros**:

- Pure Rust, no C dependencies
- Hardware-accelerated via wgpu (Vulkan/Metal/DX12)
- Custom shader support (CRT effects, scaling filters)
- Clean separation of concerns
- Growing ecosystem

**Cons**:

- Three crates to coordinate (pixels + winit + cpal)
- More boilerplate than SDL2
- WASM support still work-in-progress for pixels
- winit API changes frequently between versions

**Best for**: Pure Rust projects, projects that want GPU shader effects.

#### SDL2 (rust-sdl2)

```rust
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::TextureAccess;

let sdl = sdl2::init().unwrap();
let video = sdl.video().unwrap();
let window = video.window("GB Emulator", 640, 576).build().unwrap();
let mut canvas = window.into_canvas().build().unwrap();
let creator = canvas.texture_creator();
let mut texture = creator.create_texture_streaming(
    PixelFormatEnum::RGB24, 160, 144
).unwrap();

// In render loop:
texture.update(None, &gb.framebuffer_rgb, 160 * 3).unwrap();
canvas.copy(&texture, None, None).unwrap();
canvas.present();
```

**Pros**:

- Battle-tested, stable API
- Video + audio + input in one library
- Well-documented, many examples
- Consistent behavior across platforms

**Cons**:

- C dependency (requires SDL2 development libraries installed)
- Not pure Rust
- No WASM support
- Bundling can be tricky for distribution

**Best for**: Desktop-only projects, developers familiar with SDL2, rapid prototyping.

#### minifb

```rust
use minifb::{Window, WindowOptions};

let mut window = Window::new("GB", 160, 144, WindowOptions::default()).unwrap();
let mut buffer: Vec<u32> = vec![0; 160 * 144];

// In render loop:
for (i, pixel) in buffer.iter_mut().enumerate() {
    let color = gb.framebuffer[i / 160][i % 160];
    *pixel = palette_u32[color];
}
window.update_with_buffer(&buffer, 160, 144).unwrap();
```

**Pros**:

- Extremely simple API
- Minimal dependencies
- Fast to prototype
- Built-in input handling

**Cons**:

- No hardware acceleration (except macOS)
- Limited scaling options
- No audio support
- Fewer features

**Best for**: Quick prototypes, proof-of-concept, minimal dependency preference.

#### Web (WASM + Canvas/WebGL)

For web deployment, compile the core emulator to WASM and use browser APIs:

```rust
// Core emulator compiled with wasm-pack
#[wasm_bindgen]
pub struct GameBoy { /* ... */ }

#[wasm_bindgen]
impl GameBoy {
    pub fn frame(&mut self) -> *const u8 {
        // Run one frame, return pointer to framebuffer
        self.run_frame();
        self.framebuffer.as_ptr()
    }
}
```

JavaScript side reads the framebuffer via shared WASM memory and renders to Canvas2D or WebGL.

**Best for**: Accessible demos, educational projects, maximum reach.

### Community Recommendation

For a new Gameboy emulator project:

1. **Starting out**: `minifb` for rapid iteration during development
2. **Desktop release**: `pixels + winit + cpal` (pure Rust) or `SDL2` (proven)
3. **Cross-platform with web**: Core library as `no_std` crate + `pixels`/`winit` for native + WASM for web
4. **Maximum compatibility**: `SDL2` remains the most battle-tested option

---

## Sources

### Primary Documentation

- [Pan Docs (gbdev.io)](https://gbdev.io/pandocs/) -- The definitive Gameboy technical reference
- [Gameboy Complete Technical Reference (gekkio)](https://gekkio.fi/files/gb-docs/gbctr.pdf) -- Hardware research documentation
- [Gameboy Opcodes Table](https://pastraiser.com/cpu/gameboy/gameboy_opcodes.html) -- Complete opcode reference
- [SM83 Instruction Set (gbdev.io)](https://gbdev.io/gb-opcodes/optables/errata) -- Official errata-corrected opcode tables
- [GbdevWiki Memory Map](https://gbdev.gg8.se/wiki/articles/Memory_Map) -- Memory layout reference

### Rust Emulator Repositories

- [mvdnes/rboy](https://github.com/mvdnes/rboy) -- Mature Gameboy Color emulator in Rust
- [joamag/boytacean](https://github.com/joamag/boytacean) -- Multi-frontend GB emulator (Web/SDL/Libretro)
- [rylev/DMG-01](https://github.com/rylev/DMG-01) -- Educational GB emulator with companion book
- [mohanson/gameboy](https://github.com/mohanson/gameboy) -- Full-featured cross-platform GB emulator
- [Gekkio/mooneye-gb](https://github.com/Gekkio/mooneye-gb) -- Research-focused GB emulator and test suite
- [plorefice/gib](https://github.com/plorefice/gib) -- GB emulator in Rust
- [simias/gb-rs](https://github.com/simias/gb-rs) -- GB emulator with detailed commentary
- [jawline/Mimic](https://github.com/jawline/Mimic) -- GB emulator in Rust
- [kdar/gameboy-rs](https://github.com/kdar/gameboy-rs/blob/master/src/cartridge/mbc.rs) -- MBC implementation reference
- [raphamorim/gameboy](https://github.com/raphamorim/gameboy) -- Terminal/Web/Desktop GB emulator

### Tutorial Blog Posts and Guides

- [DMG-01: How to Emulate a Game Boy (rylev)](https://rylev.github.io/DMG-01/public/book/print.html) -- Comprehensive companion book
- [Rewriting My Game Boy Emulator: The Pixel FIFO (jsgroth)](https://jsgroth.dev/blog/posts/gb-rewrite-pixel-fifo/) -- Detailed FIFO PPU analysis
- [The Game Boy APU (jsgroth)](https://jsgroth.dev/blog/posts/gb-rewrite-apu/) -- APU implementation deep-dive
- [Writing Gameboy Emulator in Rust (Yushi Omote)](https://yushiomote.org/posts/gameboy-emu/) -- Trait-based architecture with code generation
- [Building a Gameboy Emulator in Rust (sogood99)](https://sogood99.github.io/posts/gameboy_rust_0/) -- Step-by-step CPU guide (Aug 2024)
- [Rust Adventure to Develop a Game Boy Emulator (Medium/CodeX)](https://medium.com/codex/rust-adventure-to-develop-a-game-boy-emulator-part-1-memory-3ea6e29c254c) -- Memory and register series (Sep 2024)
- [0dmg: Learning Rust by Building a Partial Game Boy Emulator](https://jeremybanks.github.io/0dmg/) -- Learning-focused
- [TLMBoy: APU Square Channel (chciken)](https://www.chciken.com/tlmboy/2025/03/28/gameboy-apu-square.html) -- APU square channel analysis (Mar 2025)
- [TLMBoy: APU Wave Channel (chciken)](https://www.chciken.com/tlmboy/2025/04/22/gameboy-apu-wave.html) -- APU wave channel analysis (Apr 2025)

### Testing Resources

- [c-sp/game-boy-test-roms](https://github.com/c-sp/game-boy-test-roms) -- Comprehensive test ROM collection (15 suites)
- [GBEmulatorShootout](https://daid.github.io/GBEmulatorShootout/) -- Standardized emulator accuracy comparison
- [GB/C Tests (Emulation General Wiki)](https://emulation.gametechwiki.com/index.php/GB/C_Tests) -- Test ROM catalog
- [Game Boy Test ROM Do's and Don'ts (gekkio)](https://gekkio.fi/blog/2016/game-boy-test-rom-dos-and-donts/) -- Testing best practices
- [Pan Docs: Timer Obscure Behaviour](https://gbdev.io/pandocs/Timer_Obscure_Behaviour.html) -- Timer edge cases
- [Pan Docs: HALT Bug](https://gbdev.io/pandocs/halt.html) -- HALT instruction quirks

### Rust Ecosystem / Frontend Libraries

- [pixels crate](https://crates.io/crates/pixels) -- Hardware-accelerated pixel framebuffer
- [pixels GitHub](https://github.com/parasyte/pixels) -- Source with examples
- [rust-sdl2](https://github.com/Rust-SDL2/rust-sdl2) -- SDL2 Rust bindings
- [cpal](https://github.com/RustAudio/cpal) -- Cross-platform audio I/O in pure Rust
- [rodio](https://github.com/RustAudio/rodio) -- High-level audio playback (built on cpal)
- [bitflags crate](https://lib.rs/crates/bitflags) -- Bitflag struct generation
- [lr35902 crate](https://crates.io/crates/lr35902) -- SM83 disassembler/emulator crate
- [Rust State Machine Patterns](https://hoverbear.org/blog/rust-state-machine-pattern/) -- Enum state machine idioms
- [Rust Design Patterns](https://rust-unofficial.github.io/patterns/) -- General Rust patterns catalog

### Accuracy Analysis

- [GameBoy Emulator Accuracy for Speedrunning](https://www.kleemans.ch/static/gb-emulator-timing/) -- Timing accuracy comparison
- [Emulation Accuracy, Speed, and Optimization (mGBA)](https://mgba.io/2017/04/30/emulation-accuracy/) -- Accuracy philosophy
- [Pan Docs: Rendering](https://gbdev.io/pandocs/Rendering.html) -- PPU rendering pipeline reference
- [Pan Docs: Pixel FIFO](https://gbdev.io/pandocs/pixel_fifo.html) -- FIFO hardware details
- [Pan Docs: Interrupts](https://gbdev.io/pandocs/Interrupts.html) -- Interrupt handling reference
- [GBEDG: Timers](https://hacktix.github.io/GBEDG/timers/) -- Timer implementation guide
- [NightShade's GB Sound Emulation](https://nightshade256.github.io/2021/03/27/gb-sound-emulation.html) -- APU implementation guide
