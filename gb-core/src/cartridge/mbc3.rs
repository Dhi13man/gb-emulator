extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

/// MBC3 — up to 2 MB ROM, 32 KB RAM, optional Real-Time Clock.
///
/// The RTC registers are mapped into the RAM bank space when ram_bank is
/// set to 0x08-0x0C.
pub struct Mbc3 {
    pub(super) rom: Vec<u8>,
    ram: Vec<u8>,
    /// ROM bank (7 bits, 1-127). Bank 0 maps to 1.
    rom_bank: u8,
    /// RAM bank (0-3 for SRAM) or RTC register select (0x08-0x0C).
    ram_bank: u8,
    ram_enabled: bool,

    // --- RTC registers ---
    rtc_seconds: u8,  // 0x08: 0-59
    rtc_minutes: u8,  // 0x09: 0-59
    rtc_hours: u8,    // 0x0A: 0-23
    rtc_days_low: u8, // 0x0B: lower 8 bits of day counter
    rtc_days_high: u8, // 0x0C: bit 0 = day counter MSB, bit 6 = halt, bit 7 = day carry

    // --- RTC latched copies ---
    rtc_latched_s: u8,
    rtc_latched_m: u8,
    rtc_latched_h: u8,
    rtc_latched_dl: u8,
    rtc_latched_dh: u8,

    /// Whether the RTC values have been latched (reading returns latched copy).
    rtc_latched: bool,
    /// Latch preparation flag: becomes true on write of 0x00, latch triggers
    /// on subsequent write of 0x01.
    rtc_latch_prep: bool,

    /// Number of ROM banks.
    rom_bank_count: usize,
    /// Number of 8 KiB RAM banks.
    ram_bank_count: usize,
}

impl Mbc3 {
    pub fn new(rom: Vec<u8>, ram_size: usize) -> Self {
        let rom_bank_count = rom.len() / 0x4000;
        let ram_bank_count = if ram_size > 0 { ram_size / 0x2000 } else { 0 };

        Self {
            rom,
            ram: vec![0u8; ram_size],
            rom_bank: 1,
            ram_bank: 0,
            ram_enabled: false,

            rtc_seconds: 0,
            rtc_minutes: 0,
            rtc_hours: 0,
            rtc_days_low: 0,
            rtc_days_high: 0,

            rtc_latched_s: 0,
            rtc_latched_m: 0,
            rtc_latched_h: 0,
            rtc_latched_dl: 0,
            rtc_latched_dh: 0,

            rtc_latched: false,
            rtc_latch_prep: false,

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
            // RAM / RTC enable (0x0000-0x1FFF)
            0x0000..=0x1FFF => {
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            // ROM bank number (0x2000-0x3FFF) — 7 bits
            0x2000..=0x3FFF => {
                let mut bank = value & 0x7F;
                if bank == 0 {
                    bank = 1;
                }
                self.rom_bank = bank;
            }
            // RAM bank / RTC register select (0x4000-0x5FFF)
            0x4000..=0x5FFF => {
                self.ram_bank = value;
            }
            // RTC latch clock data (0x6000-0x7FFF)
            0x6000..=0x7FFF => {
                if value == 0x00 {
                    self.rtc_latch_prep = true;
                } else if value == 0x01 && self.rtc_latch_prep {
                    // Latch current RTC values.
                    self.rtc_latched_s = self.rtc_seconds;
                    self.rtc_latched_m = self.rtc_minutes;
                    self.rtc_latched_h = self.rtc_hours;
                    self.rtc_latched_dl = self.rtc_days_low;
                    self.rtc_latched_dh = self.rtc_days_high;
                    self.rtc_latched = true;
                    self.rtc_latch_prep = false;
                } else {
                    self.rtc_latch_prep = false;
                }
            }
            _ => {}
        }
    }

    pub fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }

        match self.ram_bank {
            // Normal SRAM banks 0-3.
            0x00..=0x03 => {
                if self.ram.is_empty() {
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
            // RTC registers (0x08-0x0C). Return latched values if latched.
            0x08 => {
                if self.rtc_latched { self.rtc_latched_s } else { self.rtc_seconds }
            }
            0x09 => {
                if self.rtc_latched { self.rtc_latched_m } else { self.rtc_minutes }
            }
            0x0A => {
                if self.rtc_latched { self.rtc_latched_h } else { self.rtc_hours }
            }
            0x0B => {
                if self.rtc_latched { self.rtc_latched_dl } else { self.rtc_days_low }
            }
            0x0C => {
                if self.rtc_latched { self.rtc_latched_dh } else { self.rtc_days_high }
            }
            _ => 0xFF,
        }
    }

    pub fn write_ram(&mut self, addr: u16, value: u8) {
        if !self.ram_enabled {
            return;
        }

        match self.ram_bank {
            // Normal SRAM banks 0-3.
            0x00..=0x03 => {
                if self.ram.is_empty() {
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
            // RTC register writes.
            0x08 => self.rtc_seconds = value,
            0x09 => self.rtc_minutes = value,
            0x0A => self.rtc_hours = value,
            0x0B => self.rtc_days_low = value,
            0x0C => self.rtc_days_high = value,
            _ => {}
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
