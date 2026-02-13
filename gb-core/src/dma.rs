/// OAM DMA transfer controller.
/// Writing to 0xFF46 initiates a 160-byte copy from source to OAM.
pub struct Dma {
    /// Whether a DMA transfer is active
    pub active: bool,
    /// Source address high byte (written to FF46)
    source: u8,
    /// Current byte index being transferred (0-159)
    byte_index: u8,
    /// Cycles remaining in transfer
    cycles_remaining: u16,
}

impl Dma {
    pub fn new() -> Self {
        Self {
            active: false,
            source: 0,
            byte_index: 0,
            cycles_remaining: 0,
        }
    }

    pub fn read(&self) -> u8 {
        self.source
    }

    /// Start a DMA transfer. Value is the high byte of the source address.
    pub fn write(&mut self, value: u8) {
        self.source = value;
        self.active = true;
        self.byte_index = 0;
        // DMA takes 160 M-cycles = 640 T-cycles
        self.cycles_remaining = 640;
    }

    /// Returns the source address for the current DMA byte, or None if DMA is inactive.
    pub fn source_addr(&self) -> Option<u16> {
        if !self.active {
            return None;
        }
        Some((self.source as u16) << 8 | self.byte_index as u16)
    }

    /// Advance DMA by the given number of T-cycles.
    /// Returns (source_addr, oam_offset) pairs for bytes to transfer this tick.
    pub fn tick(&mut self, cycles: u32) -> alloc::vec::Vec<(u16, u8)> {
        let mut transfers = alloc::vec::Vec::new();

        if !self.active {
            return transfers;
        }

        // Each byte takes 4 T-cycles (1 M-cycle)
        for _ in 0..cycles {
            if !self.active {
                break;
            }

            self.cycles_remaining = self.cycles_remaining.saturating_sub(1);

            // Transfer one byte every 4 T-cycles
            if self.cycles_remaining % 4 == 0 && self.byte_index < 160 {
                let src = (self.source as u16) << 8 | self.byte_index as u16;
                transfers.push((src, self.byte_index));
                self.byte_index += 1;
            }

            if self.byte_index >= 160 {
                self.active = false;
                break;
            }
        }

        transfers
    }
}
