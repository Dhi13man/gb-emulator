extern crate alloc;

pub mod oam;

use crate::SCREEN_HEIGHT;
use crate::SCREEN_WIDTH;
use oam::OamEntry;

// ---------------------------------------------------------------------------
// PPU mode state machine
// ---------------------------------------------------------------------------

/// The four PPU modes the LCD controller cycles through.
#[derive(Clone, Copy, PartialEq)]
enum PpuMode {
    /// Mode 2 – OAM scan (80 dots). OAM is locked.
    OamScan,
    /// Mode 3 – Drawing pixels (variable, simplified to 172 dots). VRAM + OAM locked.
    Drawing,
    /// Mode 0 – Horizontal blank (remainder of 456 dots).
    HBlank,
    /// Mode 1 – Vertical blank (scanlines 144-153).
    VBlank,
}

/// Dots consumed by each mode on visible scanlines.
const OAM_SCAN_DOTS: u32 = 80;
const DRAWING_DOTS: u32 = 172;
const _HBLANK_DOTS: u32 = 456 - OAM_SCAN_DOTS - DRAWING_DOTS; // 204
const SCANLINE_DOTS: u32 = 456;

// ---------------------------------------------------------------------------
// LCDC bit masks
// ---------------------------------------------------------------------------

const LCDC_ENABLE: u8 = 0x80;          // Bit 7 – LCD & PPU enable
const LCDC_WIN_TILEMAP: u8 = 0x40;     // Bit 6 – Window tile map area (0=9800, 1=9C00)
const LCDC_WIN_ENABLE: u8 = 0x20;      // Bit 5 – Window enable
const LCDC_BG_WIN_TILEDATA: u8 = 0x10; // Bit 4 – BG & Window tile data area
const LCDC_BG_TILEMAP: u8 = 0x08;      // Bit 3 – BG tile map area (0=9800, 1=9C00)
const LCDC_OBJ_SIZE: u8 = 0x04;        // Bit 2 – OBJ size (0=8x8, 1=8x16)
const LCDC_OBJ_ENABLE: u8 = 0x02;      // Bit 1 – OBJ enable
const LCDC_BG_WIN_ENABLE: u8 = 0x01;   // Bit 0 – BG & Window enable/priority

// ---------------------------------------------------------------------------
// STAT bit masks
// ---------------------------------------------------------------------------

const STAT_LYC_INT: u8 = 0x40;   // Bit 6 – LYC=LY interrupt source
const STAT_MODE2_INT: u8 = 0x20; // Bit 5 – Mode 2 (OAM scan) interrupt source
const STAT_MODE1_INT: u8 = 0x10; // Bit 4 – Mode 1 (VBlank) interrupt source
const STAT_MODE0_INT: u8 = 0x08; // Bit 3 – Mode 0 (HBlank) interrupt source
const STAT_LYC_FLAG: u8 = 0x04;  // Bit 2 – LYC=LY coincidence flag (read-only)

// ---------------------------------------------------------------------------
// PPU
// ---------------------------------------------------------------------------

pub struct Ppu {
    // Video RAM (8 KiB, 0x8000-0x9FFF)
    vram: [u8; 0x2000],
    // Object Attribute Memory (160 bytes, 0xFE00-0xFE9F)
    oam: [u8; 0xA0],
    // Frame buffer: each entry is a 2-bit colour index (0-3).
    frame_buf: [[u8; SCREEN_WIDTH]; SCREEN_HEIGHT],
    // Per-scanline raw BG colour IDs (before palette mapping) for sprite priority.
    bg_color_ids: [u8; SCREEN_WIDTH],

    // ---- LCD registers ----
    lcdc: u8,  // FF40 – LCD Control
    stat: u8,  // FF41 – LCD Status (bits 6-3 writable, bits 2-0 read-only)
    scy: u8,   // FF42 – Scroll Y
    scx: u8,   // FF43 – Scroll X
    ly: u8,    // FF44 – Current scanline (read-only)
    lyc: u8,   // FF45 – LY Compare
    bgp: u8,   // FF47 – BG Palette Data
    obp0: u8,  // FF48 – OBJ Palette 0
    obp1: u8,  // FF49 – OBJ Palette 1
    wy: u8,    // FF4A – Window Y Position
    wx: u8,    // FF4B – Window X Position

    // ---- Internal state ----
    mode: PpuMode,
    /// Current dot (T-cycle) within the scanline, 0-455.
    dot: u32,
    /// Internal window line counter – incremented each scanline the window is active.
    window_line: u8,
    /// Whether the window was triggered on any scanline this frame.
    window_triggered: bool,

    /// Previous STAT interrupt line state (for edge detection).
    prev_stat_line: bool,

    // ---- Interrupt outputs (active for one `step` call, then cleared by bus) ----
    pub stat_interrupt: bool,
    pub vblank_interrupt: bool,

    /// Set when the PPU enters VBlank — indicates the frame buffer is complete.
    pub frame_complete: bool,

    /// Frame counter for debug logging.
    pub frame_number: u32,
    /// Debug: log PPU state per scanline for a specific frame range.
    pub debug_log: bool,
    /// Debug output buffer.
    pub debug_buffer: Option<alloc::string::String>,
}

impl Ppu {
    /// Create a new PPU initialised to post-boot values.
    pub fn new() -> Self {
        Self {
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            frame_buf: [[0u8; SCREEN_WIDTH]; SCREEN_HEIGHT],
            bg_color_ids: [0u8; SCREEN_WIDTH],

            lcdc: 0x91,  // BG on, OBJ on, LCD on
            stat: 0x01,  // Mode 1 (VBlank), no LYC match (LY=153, LYC=0)
            scy: 0x00,
            scx: 0x00,
            ly: 153,     // Last VBlank line — wraps to frame start on next scanline
            lyc: 0x00,
            bgp: 0xFC,   // 11 11 11 00 – colour 0 = lightest
            obp0: 0x00,
            obp1: 0x00,
            wy: 0x00,
            wx: 0x00,

            mode: PpuMode::VBlank,
            dot: 0,
            window_line: 0,
            window_triggered: false,

            prev_stat_line: false,

            stat_interrupt: false,
            vblank_interrupt: false,
            frame_complete: false,

            frame_number: 0,
            debug_log: false,
            debug_buffer: None,
        }
    }

    // -----------------------------------------------------------------------
    // Public accessors
    // -----------------------------------------------------------------------

    /// Return a reference to the completed frame buffer.
    pub fn frame_buffer(&self) -> &[[u8; SCREEN_WIDTH]; SCREEN_HEIGHT] {
        &self.frame_buf
    }

    // -----------------------------------------------------------------------
    // Step – advance the PPU by `t_cycles` dots
    // -----------------------------------------------------------------------

    /// Advance the PPU by the given number of T-cycles (dots).
    ///
    /// The PPU processes one dot at a time so that mode transitions occur at
    /// the correct boundaries. Interrupt flags are raised as side-effects and
    /// must be consumed by the bus after each call.
    pub fn step(&mut self, t_cycles: u32) {
        // When the LCD is disabled nothing happens.
        if self.lcdc & LCDC_ENABLE == 0 {
            return;
        }

        for _ in 0..t_cycles {
            self.tick_one_dot();
        }
    }

    /// Process a single dot (T-cycle).
    fn tick_one_dot(&mut self) {
        self.dot += 1;

        match self.mode {
            PpuMode::OamScan => {
                if self.dot >= OAM_SCAN_DOTS {
                    self.enter_mode(PpuMode::Drawing);
                }
            }
            PpuMode::Drawing => {
                if self.dot >= OAM_SCAN_DOTS + DRAWING_DOTS {
                    // Render the scanline at the end of drawing.
                    self.render_scanline();
                    self.enter_mode(PpuMode::HBlank);
                }
            }
            PpuMode::HBlank => {
                if self.dot >= SCANLINE_DOTS {
                    self.dot = 0;
                    self.ly += 1;

                    if self.ly >= 144 {
                        self.enter_mode(PpuMode::VBlank);
                        self.vblank_interrupt = true;
                        self.frame_complete = true;
                        self.frame_number += 1;
                    } else {
                        self.enter_mode(PpuMode::OamScan);
                    }

                    self.check_lyc();
                }
            }
            PpuMode::VBlank => {
                if self.dot >= SCANLINE_DOTS {
                    self.dot = 0;
                    self.ly += 1;

                    if self.ly > 153 {
                        // Frame complete – wrap to line 0.
                        self.ly = 0;
                        self.window_line = 0;
                        self.window_triggered = false;
                        self.enter_mode(PpuMode::OamScan);
                    }

                    self.check_lyc();
                }
            }
        }
    }

    /// Transition to a new PPU mode, update STAT bits, and raise STAT
    /// interrupt on rising edge when the corresponding enable bit is set.
    fn enter_mode(&mut self, new_mode: PpuMode) {
        self.mode = new_mode;

        // Update the mode bits (bits 1-0) of STAT.
        let mode_bits = match new_mode {
            PpuMode::HBlank => 0,
            PpuMode::VBlank => 1,
            PpuMode::OamScan => 2,
            PpuMode::Drawing => 3,
        };
        self.stat = (self.stat & 0xFC) | mode_bits;

        // Evaluate the combined STAT interrupt line.
        self.evaluate_stat_interrupt();
    }

    /// Check LYC=LY coincidence flag and update STAT accordingly.
    fn check_lyc(&mut self) {
        if self.ly == self.lyc {
            self.stat |= STAT_LYC_FLAG;
        } else {
            self.stat &= !STAT_LYC_FLAG;
        }
        self.evaluate_stat_interrupt();
    }

    /// Evaluate the composite STAT interrupt line. A STAT interrupt is
    /// requested only on a *rising edge* (transition from low to high).
    fn evaluate_stat_interrupt(&mut self) {
        let line = (self.stat & STAT_LYC_INT != 0 && self.stat & STAT_LYC_FLAG != 0)
            || (self.stat & STAT_MODE0_INT != 0 && self.mode == PpuMode::HBlank)
            || (self.stat & STAT_MODE1_INT != 0 && self.mode == PpuMode::VBlank)
            || (self.stat & STAT_MODE2_INT != 0 && self.mode == PpuMode::OamScan);

        // Rising edge detection.
        if line && !self.prev_stat_line {
            self.stat_interrupt = true;
        }
        self.prev_stat_line = line;
    }

    // -----------------------------------------------------------------------
    // Scanline rendering
    // -----------------------------------------------------------------------

    /// Render a full scanline into the frame buffer for the current LY.
    fn render_scanline(&mut self) {
        if self.ly as usize >= SCREEN_HEIGHT {
            return;
        }

        if self.debug_log {
            if let Some(buf) = &mut self.debug_buffer {
                // Only log a few key scanlines to avoid floods
                if self.ly == 0 || self.ly == 72 || self.ly == 143 {
                    use core::fmt::Write;
                    // Read tile map preview before borrowing buf mutably
                    let m: [u8; 8] = core::array::from_fn(|i| self.vram[(0x9800 - 0x8000 + i) as usize]);
                    let ly = self.ly;
                    let lcdc = self.lcdc;
                    let scx = self.scx;
                    let scy = self.scy;
                    let wy = self.wy;
                    let bgp = self.bgp;
                    let fn_ = self.frame_number;
                    let _ = write!(
                        buf,
                        "F{} LY={:3} LCDC={:02X} SCX={:3} SCY={:3} WY={:3} BGP={:02X} MAP={:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}\n",
                        fn_, ly, lcdc, scx, scy, wy, bgp,
                        m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7],
                    );
                }
            }
        }

        // Background / Window enable (DMG: bit 0 = BG/WIN master enable).
        if self.lcdc & LCDC_BG_WIN_ENABLE != 0 {
            self.render_bg_scanline();

            // Window: enabled by LCDC.5, and only if the current scanline
            // is at or past WY, and WX <= 166.
            if self.lcdc & LCDC_WIN_ENABLE != 0 && self.ly >= self.wy && self.wx <= 166 {
                self.render_window_scanline();
            }
        } else {
            // When BG/WIN is off the background is colour 0 (white).
            let line = self.ly as usize;
            for px in 0..SCREEN_WIDTH {
                self.frame_buf[line][px] = 0;
                self.bg_color_ids[px] = 0;
            }
        }

        // Sprites.
        if self.lcdc & LCDC_OBJ_ENABLE != 0 {
            self.render_sprites_scanline();
        }
    }

    // ---- Background ----

    fn render_bg_scanline(&mut self) {
        let line = self.ly as usize;

        // Which tile map?  LCDC.3: 0 → 0x9800, 1 → 0x9C00.
        let tilemap_base: u16 = if self.lcdc & LCDC_BG_TILEMAP != 0 {
            0x9C00
        } else {
            0x9800
        };

        // Tile data area: LCDC.4: 1 → 0x8000 (unsigned), 0 → 0x8800 (signed).
        let unsigned_addressing = self.lcdc & LCDC_BG_WIN_TILEDATA != 0;

        let y = self.scy.wrapping_add(self.ly);
        let tile_row = (y as u16 / 8) & 31; // which row of tiles (0-31)
        let pixel_y = y % 8;                 // row within the tile (0-7)

        for px in 0..SCREEN_WIDTH {
            let x = self.scx.wrapping_add(px as u8);
            let tile_col = (x as u16 / 8) & 31;
            let pixel_x = x % 8;

            // Fetch tile index from the tile map.
            let map_addr = tilemap_base + tile_row * 32 + tile_col;
            let tile_index = self.vram_read_internal(map_addr);

            // Compute address of the two bytes for this row of the tile.
            let tile_data_addr = tile_data_address(tile_index, unsigned_addressing, pixel_y);

            let lo = self.vram_read_internal(tile_data_addr);
            let hi = self.vram_read_internal(tile_data_addr + 1);

            let colour_id = pixel_colour_id(lo, hi, pixel_x);
            self.bg_color_ids[px] = colour_id;
            self.frame_buf[line][px] = apply_palette(self.bgp, colour_id);
        }
    }

    // ---- Window ----

    fn render_window_scanline(&mut self) {
        let line = self.ly as usize;

        let tilemap_base: u16 = if self.lcdc & LCDC_WIN_TILEMAP != 0 {
            0x9C00
        } else {
            0x9800
        };

        let unsigned_addressing = self.lcdc & LCDC_BG_WIN_TILEDATA != 0;

        // The window X on screen starts at (WX - 7). Values 0-6 are
        // effectively negative and clip the left side of the window.
        let wx_screen = self.wx as i16 - 7;

        let pixel_y = self.window_line % 8;
        let tile_row = (self.window_line as u16 / 8) & 31;

        let mut any_drawn = false;

        for px in 0..SCREEN_WIDTH {
            let screen_x = px as i16;
            if screen_x < wx_screen {
                continue;
            }

            any_drawn = true;
            let win_x = (screen_x - wx_screen) as u8;
            let tile_col = (win_x as u16 / 8) & 31;
            let pixel_x = win_x % 8;

            let map_addr = tilemap_base + tile_row * 32 + tile_col;
            let tile_index = self.vram_read_internal(map_addr);

            let tile_data_addr = tile_data_address(tile_index, unsigned_addressing, pixel_y);

            let lo = self.vram_read_internal(tile_data_addr);
            let hi = self.vram_read_internal(tile_data_addr + 1);

            let colour_id = pixel_colour_id(lo, hi, pixel_x);
            self.bg_color_ids[px] = colour_id;
            self.frame_buf[line][px] = apply_palette(self.bgp, colour_id);
        }

        // The window line counter only increments on scanlines where the
        // window was actually rendered.
        if any_drawn {
            self.window_line += 1;
            self.window_triggered = true;
        }
    }

    // ---- Sprites ----

    fn render_sprites_scanline(&mut self) {
        let line = self.ly as usize;
        let sprite_height: u8 = if self.lcdc & LCDC_OBJ_SIZE != 0 { 16 } else { 8 };

        // Collect sprites that intersect the current scanline (max 10).
        let mut sprites: alloc::vec::Vec<(usize, OamEntry)> = alloc::vec::Vec::new();

        for i in 0..40 {
            if sprites.len() >= 10 {
                break;
            }
            let base = i * 4;
            let entry = OamEntry::from_bytes(&self.oam[base..base + 4]);

            // Screen Y of the sprite's top pixel is (entry.y - 16).
            let sprite_top = entry.y.wrapping_sub(16) as i16;
            let ly_i = self.ly as i16;

            if ly_i >= sprite_top && ly_i < sprite_top + sprite_height as i16 {
                sprites.push((i, entry));
            }
        }

        // Sort by priority: lower X first, ties broken by lower OAM index
        // (which is already the natural order since we iterate 0..40).
        // We must use a *stable* sort so that equal-X entries keep their
        // OAM ordering.
        sprites.sort_by(|a, b| a.1.x.cmp(&b.1.x));

        // Render in *reverse* order so that higher-priority (lower index in
        // the sorted list) sprites overwrite lower-priority ones.
        for (oam_index, entry) in sprites.iter().rev() {
            let _ = oam_index; // used only for sort stability

            let sprite_screen_x = entry.x as i16 - 8;
            let sprite_screen_y = entry.y as i16 - 16;

            // Row within the sprite.
            let mut row = (self.ly as i16 - sprite_screen_y) as u8;
            if entry.y_flip() {
                row = sprite_height - 1 - row;
            }

            // In 8x16 mode the tile index has bit 0 forced to 0 for the
            // top tile and 1 for the bottom tile.
            let tile_index = if sprite_height == 16 {
                if row < 8 {
                    entry.tile & 0xFE
                } else {
                    entry.tile | 0x01
                }
            } else {
                entry.tile
            };

            let tile_row = row % 8;
            // Sprites always use unsigned addressing from 0x8000.
            let tile_addr = 0x8000u16 + tile_index as u16 * 16 + tile_row as u16 * 2;
            let lo = self.vram_read_internal(tile_addr);
            let hi = self.vram_read_internal(tile_addr + 1);

            let palette = if entry.palette() == 0 { self.obp0 } else { self.obp1 };

            for pixel_x in 0..8u8 {
                let screen_x = sprite_screen_x + pixel_x as i16;
                if screen_x < 0 || screen_x >= SCREEN_WIDTH as i16 {
                    continue;
                }

                let bit = if entry.x_flip() { pixel_x } else { 7 - pixel_x };
                let colour_id = ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1);

                // Colour 0 is transparent for sprites.
                if colour_id == 0 {
                    continue;
                }

                // BG-over-OBJ: if the priority bit is set, the sprite is
                // hidden behind BG colours 1-3.
                if entry.priority() {
                    // Check the raw BG colour ID (before palette mapping).
                    // If it is non-zero (colours 1-3), the sprite pixel is hidden.
                    if self.bg_color_ids[screen_x as usize] != 0 {
                        continue;
                    }
                }

                self.frame_buf[line][screen_x as usize] = apply_palette(palette, colour_id);
            }
        }
    }

    // -----------------------------------------------------------------------
    // VRAM / OAM access (respecting mode locks)
    // -----------------------------------------------------------------------

    /// Internal VRAM read – used by the PPU itself during rendering.
    /// Bypasses the mode lock since the PPU always has access to its own VRAM.
    fn vram_read_internal(&self, addr: u16) -> u8 {
        self.vram[(addr - 0x8000) as usize]
    }

    /// Read a byte from VRAM (0x8000-0x9FFF).
    /// Returns 0xFF when VRAM is inaccessible (during Mode 3).
    pub fn read_vram(&self, addr: u16) -> u8 {
        if self.lcdc & LCDC_ENABLE != 0 && self.mode == PpuMode::Drawing {
            return 0xFF;
        }
        self.vram[(addr - 0x8000) as usize]
    }

    /// Write a byte to VRAM (0x8000-0x9FFF).
    /// Writes are ignored when VRAM is inaccessible (during Mode 3).
    pub fn write_vram(&mut self, addr: u16, val: u8) {
        if self.lcdc & LCDC_ENABLE != 0 && self.mode == PpuMode::Drawing {
            return;
        }
        self.vram[(addr - 0x8000) as usize] = val;
    }

    /// Read a byte from OAM (0xFE00-0xFE9F).
    /// Returns 0xFF when OAM is inaccessible (during Mode 2 or 3).
    pub fn read_oam(&self, addr: u16) -> u8 {
        if self.lcdc & LCDC_ENABLE != 0
            && (self.mode == PpuMode::OamScan || self.mode == PpuMode::Drawing)
        {
            return 0xFF;
        }
        self.oam[(addr - 0xFE00) as usize]
    }

    /// Write a byte to OAM (0xFE00-0xFE9F).
    /// Writes are ignored when OAM is inaccessible (during Mode 2 or 3).
    pub fn write_oam(&mut self, addr: u16, val: u8) {
        if self.lcdc & LCDC_ENABLE != 0
            && (self.mode == PpuMode::OamScan || self.mode == PpuMode::Drawing)
        {
            return;
        }
        self.oam[(addr - 0xFE00) as usize] = val;
    }

    /// Direct OAM write used by DMA transfer – bypasses mode locks.
    pub fn dma_write_oam(&mut self, offset: u8, val: u8) {
        self.oam[offset as usize] = val;
    }

    // -----------------------------------------------------------------------
    // Register I/O (FF40 – FF4B)
    // -----------------------------------------------------------------------

    /// Read an LCD register.
    pub fn read_reg(&self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.lcdc,
            0xFF41 => {
                // Bits 6-3 are R/W, bits 2-0 are read-only (mode + LYC flag).
                // Bit 7 always reads as 1.
                0x80 | (self.stat & 0x7F)
            }
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            // FF46 is DMA – handled by bus, not PPU.
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            _ => 0xFF,
        }
    }

    /// Write an LCD register.
    pub fn write_reg(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF40 => {
                let was_enabled = self.lcdc & LCDC_ENABLE != 0;
                self.lcdc = val;
                let now_enabled = self.lcdc & LCDC_ENABLE != 0;

                if was_enabled && !now_enabled {
                    // LCD turned off: reset PPU state.
                    self.ly = 0;
                    self.dot = 0;
                    self.window_line = 0;
                    self.window_triggered = false;
                    self.mode = PpuMode::HBlank;
                    self.stat = (self.stat & 0xFC) | 0; // mode 0
                    self.prev_stat_line = false;
                }

                if !was_enabled && now_enabled {
                    // LCD turned on: start at mode 2 (OAM scan), line 0.
                    self.ly = 0;
                    self.dot = 0;
                    self.window_line = 0;
                    self.window_triggered = false;
                    self.mode = PpuMode::OamScan;
                    self.stat = (self.stat & 0xFC) | 2;
                    self.check_lyc();
                }
            }
            0xFF41 => {
                // Only bits 6-3 are writable. Bits 2-0 and bit 7 are read-only.
                self.stat = (self.stat & 0x87) | (val & 0x78);
                // Writing STAT can trigger a STAT interrupt re-evaluation.
                self.evaluate_stat_interrupt();
            }
            0xFF42 => self.scy = val,
            0xFF43 => self.scx = val,
            0xFF44 => {} // LY is read-only
            0xFF45 => {
                self.lyc = val;
                if self.lcdc & LCDC_ENABLE != 0 {
                    self.check_lyc();
                }
            }
            // FF46 is DMA – handled by bus.
            0xFF47 => self.bgp = val,
            0xFF48 => self.obp0 = val,
            0xFF49 => self.obp1 = val,
            0xFF4A => self.wy = val,
            0xFF4B => self.wx = val,
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions (pure, no state)
// ---------------------------------------------------------------------------

/// Compute the VRAM address of the first byte of a given tile row.
///
/// * `tile_index` – the raw index from the tile map (0x00-0xFF).
/// * `unsigned_addressing` – true when LCDC.4 = 1 (base 0x8000, unsigned).
///   When false the base is 0x9000 and the index is treated as signed.
/// * `row` – pixel row within the 8-pixel tall tile (0-7).
fn tile_data_address(tile_index: u8, unsigned_addressing: bool, row: u8) -> u16 {
    let base: u16 = if unsigned_addressing {
        0x8000 + tile_index as u16 * 16
    } else {
        // Signed: 0x9000 + (tile_index as i8) * 16
        let signed = tile_index as i8 as i16;
        (0x9000u16 as i16 + signed * 16) as u16
    };
    base + row as u16 * 2
}

/// Decode a single pixel's 2-bit colour ID from two tile-data bytes.
///
/// `pixel_x` is the column within the tile (0 = leftmost … 7 = rightmost).
/// The high bit comes from `hi`, the low bit from `lo`, with bit 7 being
/// the leftmost pixel.
#[inline]
fn pixel_colour_id(lo: u8, hi: u8, pixel_x: u8) -> u8 {
    let bit = 7 - pixel_x;
    let low_bit = (lo >> bit) & 1;
    let high_bit = (hi >> bit) & 1;
    (high_bit << 1) | low_bit
}

/// Map a 2-bit colour ID through a palette register.
///
/// Palette layout: bits 1:0 = colour 0, bits 3:2 = colour 1, etc.
#[inline]
fn apply_palette(palette: u8, colour_id: u8) -> u8 {
    (palette >> (colour_id * 2)) & 0x03
}
