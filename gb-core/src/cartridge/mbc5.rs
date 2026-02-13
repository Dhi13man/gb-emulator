extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

/// MBC5 — up to 8 MB ROM, 128 KB RAM.
///
/// Unlike MBC1, bank 0 is valid in the high ROM window (no quirky remapping).
/// The ROM bank number is 9 bits wide (0-511).
pub struct Mbc5 {
    pub(super) rom: Vec<u8>,
    ram: Vec<u8>,
    /// 9-bit ROM bank number (0-511).
    rom_bank: u16,
    /// 4-bit RAM bank number (0-15).
    ram_bank: u8,
    ram_enabled: bool,
    /// Number of ROM banks.
    rom_bank_count: usize,
    /// Number of 8 KiB RAM banks.
    ram_bank_count: usize,
}

impl Mbc5 {
    pub fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        let rom_bank_count = rom.len() / 0x4000;
        let ram_bank_count = if ram_size > 0 { ram_size / 0x2000 } else { 0 };

        Self {
            rom,
            ram: vec![0u8; ram_size],
            rom_bank: 1,
            ram_bank: 0,
            ram_enabled: false,
            rom_bank_count: if rom_bank_count == 0 { 1 } else { rom_bank_count },
            ram_bank_count,
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
            // RAM enable (0x0000-0x1FFF)
            0x0000..=0x1FFF => {
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            // ROM bank — low 8 bits (0x2000-0x2FFF)
            0x2000..=0x2FFF => {
                self.rom_bank = (self.rom_bank & 0x100) | (value as u16);
            }
            // ROM bank — bit 8 (0x3000-0x3FFF)
            0x3000..=0x3FFF => {
                self.rom_bank = (self.rom_bank & 0x0FF) | (((value & 0x01) as u16) << 8);
            }
            // RAM bank (0x4000-0x5FFF) — 4 bits
            0x4000..=0x5FFF => {
                self.ram_bank = value & 0x0F;
            }
            _ => {}
        }
    }

    pub fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled || self.ram.is_empty() {
            return 0xFF;
        }
        let bank = if self.ram_bank_count > 0 {
            (self.ram_bank as usize) % self.ram_bank_count
        } else {
            0
        };
        let offset = bank * 0x2000 + (addr as usize - 0xA000);
        if offset < self.ram.len() {
            self.ram[offset]
        } else {
            0xFF
        }
    }

    pub fn write_ram(&mut self, addr: u16, value: u8) {
        if !self.ram_enabled || self.ram.is_empty() {
            return;
        }
        let bank = if self.ram_bank_count > 0 {
            (self.ram_bank as usize) % self.ram_bank_count
        } else {
            0
        };
        let offset = bank * 0x2000 + (addr as usize - 0xA000);
        if offset < self.ram.len() {
            self.ram[offset] = value;
        }
    }

    pub fn ram_data(&self) -> &[u8] {
        &self.ram
    }

    pub fn load_ram(&mut self, data: &[u8]) {
        let len = data.len().min(self.ram.len());
        self.ram[..len].copy_from_slice(&data[..len]);
    }
}
