extern crate alloc;

/// Serial transfer stub (FF01-FF02).
/// Minimal implementation to prevent games from hanging.
pub struct Serial {
    /// Serial transfer data (SB, 0xFF01)
    pub data: u8,
    /// Serial transfer control (SC, 0xFF02)
    pub control: u8,
    /// Transfer cycles remaining
    transfer_cycles: u32,
    pub interrupt_pending: bool,
    /// Captured serial output bytes (for test ROM debugging).
    output: alloc::vec::Vec<u8>,
}

impl Serial {
    pub fn new() -> Self {
        Self {
            data: 0x00,
            control: 0x7E,
            transfer_cycles: 0,
            interrupt_pending: false,
            output: alloc::vec::Vec::new(),
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF01 => self.data,
            0xFF02 => self.control | 0x7E, // Bits 1-6 always 1 on DMG
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF01 => self.data = value,
            0xFF02 => {
                self.control = value;
                // If transfer start and internal clock selected
                if value & 0x81 == 0x81 {
                    // Start transfer: 8 bits at 8192 Hz = 512 T-cycles per bit = 4096 total
                    self.transfer_cycles = 4096;
                }
            }
            _ => {}
        }
    }

    /// Drain captured serial output bytes.
    pub fn drain_output(&mut self) -> alloc::vec::Vec<u8> {
        core::mem::take(&mut self.output)
    }

    pub fn tick(&mut self, cycles: u32) {
        if self.transfer_cycles == 0 {
            return;
        }

        self.transfer_cycles = self.transfer_cycles.saturating_sub(cycles);

        if self.transfer_cycles == 0 {
            // Transfer complete: capture the sent byte before replacing
            self.output.push(self.data);
            // No cable connected, received 0xFF
            self.data = 0xFF;
            self.control &= !0x80; // Clear transfer start flag
            self.interrupt_pending = true;
        }
    }
}
