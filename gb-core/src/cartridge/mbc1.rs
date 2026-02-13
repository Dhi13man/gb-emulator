extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

/// MBC1 — up to 2 MB ROM, 32 KB RAM.
///
/// Banking modes:
///   Mode 0 (ROM banking): the 2-bit register is combined with the 5-bit bank
///     for ROM bank selection; RAM is always bank 0.
///   Mode 1 (RAM banking / advanced ROM banking): the 2-bit register selects
///     the RAM bank OR upper ROM bits for the 0x0000-0x3FFF window.
pub struct Mbc1 {
    pub(super) rom: Vec<u8>,
    ram: Vec<u8>,
    /// Lower 5-bit ROM bank register (written via 0x2000-0x3FFF).
    rom_bank: u8,
    /// Upper 2-bit register (written via 0x4000-0x5FFF).
    ram_bank: u8,
    ram_enabled: bool,
    /// false = mode 0 (ROM mode), true = mode 1 (RAM mode).
    banking_mode: bool,
    /// Number of ROM banks (derived from ROM size).
    rom_bank_count: usize,
    /// Number of 8 KiB RAM banks (derived from RAM size).
    ram_bank_count: usize,
}

impl Mbc1 {
    pub fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        let rom_bank_count = rom.len() / 0x4000;
        let ram_bank_count = if ram_size > 0 { ram_size / 0x2000 } else { 0 };

        Self {
            rom,
            ram: vec![0u8; ram_size],
            rom_bank: 1,
            ram_bank: 0,
            ram_enabled: false,
            banking_mode: false,
            rom_bank_count: if rom_bank_count == 0 { 1 } else { rom_bank_count },
            ram_bank_count,
        }
    }

    /// Resolve the effective ROM bank for the 0x0000-0x3FFF window.
    fn bank0(&self) -> usize {
        if self.banking_mode {
            // Mode 1: upper bits affect the low bank window.
            let bank = (self.ram_bank as usize) << 5;
            bank % self.rom_bank_count
        } else {
            // Mode 0: low bank is always bank 0.
            0
        }
    }

    /// Resolve the effective ROM bank for the 0x4000-0x7FFF window.
    fn bank_high(&self) -> usize {
        let bank = ((self.ram_bank as usize) << 5) | (self.rom_bank as usize);
        bank % self.rom_bank_count
    }

    /// Resolve the effective RAM bank.
    fn effective_ram_bank(&self) -> usize {
        if self.ram_bank_count == 0 {
            return 0;
        }
        if self.banking_mode {
            (self.ram_bank as usize) % self.ram_bank_count
        } else {
            0
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // Low ROM bank (0x0000-0x3FFF)
            0x0000..=0x3FFF => {
                let bank = self.bank0();
                let offset = bank * 0x4000 + addr as usize;
                if offset < self.rom.len() {
                    self.rom[offset]
                } else {
                    0xFF
                }
            }
            // High ROM bank (0x4000-0x7FFF)
            0x4000..=0x7FFF => {
                let bank = self.bank_high();
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
            // ROM bank number — lower 5 bits (0x2000-0x3FFF)
            0x2000..=0x3FFF => {
                let mut bank = value & 0x1F;
                // Bank 0 is not directly addressable in the high window;
                // hardware maps 0 → 1.
                if bank == 0 {
                    bank = 1;
                }
                self.rom_bank = bank;
            }
            // RAM bank / upper ROM bits (0x4000-0x5FFF)
            0x4000..=0x5FFF => {
                self.ram_bank = value & 0x03;
            }
            // Banking mode select (0x6000-0x7FFF)
            0x6000..=0x7FFF => {
                self.banking_mode = (value & 0x01) != 0;
            }
            _ => {}
        }
    }

    pub fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled || self.ram.is_empty() {
            return 0xFF;
        }
        let bank = self.effective_ram_bank();
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
        let bank = self.effective_ram_bank();
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
