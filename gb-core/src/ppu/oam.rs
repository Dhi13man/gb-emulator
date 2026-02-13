/// A single OAM (Object Attribute Memory) entry describing one sprite.
///
/// Each entry is 4 bytes in OAM at addresses FE00-FE9F (40 entries total).
///
/// Byte 0: Y position (screen Y = y - 16)
/// Byte 1: X position (screen X = x - 8)
/// Byte 2: Tile index into VRAM tile data at 0x8000
/// Byte 3: Flags/attributes
///   Bit 7: BG-over-OBJ priority (0 = sprite above BG, 1 = BG colors 1-3 over sprite)
///   Bit 6: Y flip
///   Bit 5: X flip
///   Bit 4: Palette (0 = OBP0, 1 = OBP1)
///   Bits 3-0: Unused (CGB only)
pub struct OamEntry {
    pub y: u8,
    pub x: u8,
    pub tile: u8,
    pub flags: u8,
}

impl OamEntry {
    /// Parse an OAM entry from a 4-byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            y: bytes[0],
            x: bytes[1],
            tile: bytes[2],
            flags: bytes[3],
        }
    }

    /// BG-over-OBJ priority flag (bit 7).
    /// When true, background colors 1-3 are drawn over the sprite.
    #[inline]
    pub fn priority(&self) -> bool {
        self.flags & 0x80 != 0
    }

    /// Y flip flag (bit 6).
    #[inline]
    pub fn y_flip(&self) -> bool {
        self.flags & 0x40 != 0
    }

    /// X flip flag (bit 5).
    #[inline]
    pub fn x_flip(&self) -> bool {
        self.flags & 0x20 != 0
    }

    /// Palette selection (bit 4): 0 = OBP0, 1 = OBP1.
    #[inline]
    pub fn palette(&self) -> u8 {
        (self.flags >> 4) & 1
    }
}
