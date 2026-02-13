extern crate alloc;
use alloc::vec::Vec;

/// MBC2 — up to 256 KB ROM, 512 x 4-bit internal RAM.
///
/// The RAM is built into the MBC2 chip itself (not a separate SRAM chip).
/// Each byte only uses the lower 4 bits; the upper 4 bits read as 0xF.
pub struct Mbc2 {
    pub(super) rom: Vec<u8>,
    /// 512 bytes of 4-bit RAM (only lower nibble is valid).
    ram: [u8; 512],
    rom_bank: u8,
    ram_enabled: bool,
    /// Number of ROM banks (derived from ROM size).
    rom_bank_count: usize,
}

impl Mbc2 {
    pub fn new(rom: Vec<u8>, _ram_size: usize) -> Self {
        // MBC2 always has exactly 512 x 4-bit internal RAM regardless of header.
        let rom_bank_count = rom.len() / 0x4000;
        Self {
            rom,
            ram: [0u8; 512],
            rom_bank: 1,
            ram_enabled: false,
            rom_bank_count: if rom_bank_count == 0 { 1 } else { rom_bank_count },
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // Low ROM bank — always bank 0.
            0x0000..=0x3FFF => {
                let index = addr as usize;
                if index < self.rom.len() {
                    self.rom[index]
                } else {
                    0xFF
                }
            }
            // High ROM bank.
            0x4000..=0x7FFF => {
                let bank = (self.rom_bank as usize) % self.rom_bank_count;
                let offset = bank * 0x4000 + (addr as usize - 0x4000);
                if offset < self.rom.len() {
                    self.rom[offset]
                } else {
                    0xFF
                }
            }
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x3FFF => {
                // Bit 8 of the address determines the operation:
                //   bit 8 == 0 → RAM enable/disable
                //   bit 8 == 1 → ROM bank select
                if addr & 0x0100 == 0 {
                    // RAM enable: any write with low nibble == 0x0A enables.
                    self.ram_enabled = (value & 0x0F) == 0x0A;
                } else {
                    // ROM bank: only lower 4 bits. Bank 0 maps to 1.
                    let mut bank = value & 0x0F;
                    if bank == 0 {
                        bank = 1;
                    }
                    self.rom_bank = bank;
                }
            }
            _ => {}
        }
    }

    pub fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        // MBC2 RAM is 512 bytes, addressed as 0xA000-0xA1FF, mirrored
        // throughout the 0xA000-0xBFFF range.
        let index = (addr as usize - 0xA000) & 0x01FF;
        // Upper nibble reads as 0xF on real hardware.
        self.ram[index] | 0xF0
    }

    pub fn write_ram(&mut self, addr: u16, value: u8) {
        if !self.ram_enabled {
            return;
        }
        let index = (addr as usize - 0xA000) & 0x01FF;
        // Only the lower 4 bits are stored.
        self.ram[index] = value & 0x0F;
    }

    pub fn ram_data(&self) -> &[u8] {
        &self.ram
    }

    pub fn load_ram(&mut self, data: &[u8]) {
        let len = data.len().min(self.ram.len());
        self.ram[..len].copy_from_slice(&data[..len]);
    }
}
