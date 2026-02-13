extern crate alloc;
use alloc::vec::Vec;

/// ROM-only cartridge (32 KiB, no banking, no RAM).
pub struct NoMbc {
    pub(super) rom: Vec<u8>,
}

impl NoMbc {
    pub fn new(rom: Vec<u8>, _ram_size: usize) -> Self {
        Self { rom }
    }

    /// Direct read from ROM (0x0000-0x7FFF).
    pub fn read(&self, addr: u16) -> u8 {
        let index = addr as usize;
        if index < self.rom.len() {
            self.rom[index]
        } else {
            0xFF
        }
    }

    /// Writes to ROM area are ignored on ROM-only cartridges.
    pub fn write(&mut self, _addr: u16, _value: u8) {
        // No-op: ROM-only cartridge has no bank switching.
    }

    /// No external RAM on ROM-only cartridges.
    pub fn read_ram(&self, _addr: u16) -> u8 {
        0xFF
    }

    /// No external RAM on ROM-only cartridges.
    pub fn write_ram(&mut self, _addr: u16, _value: u8) {
        // No-op
    }

    pub fn ram_data(&self) -> &[u8] {
        &[]
    }

    pub fn load_ram(&mut self, _data: &[u8]) {
        // No-op: no RAM to load into.
    }
}
