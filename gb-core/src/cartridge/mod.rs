extern crate alloc;
use alloc::vec::Vec;
use core::str;

mod no_mbc;
mod mbc1;
mod mbc2;
mod mbc3;
mod mbc5;

use no_mbc::NoMbc;
use mbc1::Mbc1;
use mbc2::Mbc2;
use mbc3::Mbc3;
use mbc5::Mbc5;

/// Cartridge with enum-based MBC dispatch (zero-cost via compiler optimization).
pub enum Cartridge {
    NoMbc(NoMbc),
    Mbc1(Mbc1),
    Mbc2(Mbc2),
    Mbc3(Mbc3),
    Mbc5(Mbc5),
}

impl Cartridge {
    /// Parse the ROM header, detect the MBC type, allocate RAM, and construct
    /// the appropriate cartridge variant.
    ///
    /// # Panics
    /// Panics if the ROM is too small to contain a valid header (< 0x0150 bytes)
    /// or if the cartridge type byte is unsupported.
    pub fn new(rom: Vec<u8>) -> Self {
        assert!(
            rom.len() >= 0x0150,
            "ROM too small: {} bytes (minimum 336 bytes for a valid header)",
            rom.len()
        );

        let cartridge_type = rom[0x0147];
        let ram_size = Self::parse_ram_size(rom[0x0149]);

        match cartridge_type {
            // ROM Only
            0x00 => Cartridge::NoMbc(NoMbc::new(rom, ram_size)),

            // MBC1 / MBC1+RAM / MBC1+RAM+BATTERY
            0x01 | 0x02 | 0x03 => Cartridge::Mbc1(Mbc1::new(rom, ram_size)),

            // MBC2 / MBC2+BATTERY
            0x05 | 0x06 => Cartridge::Mbc2(Mbc2::new(rom, ram_size)),

            // MBC3+TIMER+BATTERY / MBC3+TIMER+RAM+BATTERY / MBC3 / MBC3+RAM /
            // MBC3+RAM+BATTERY
            0x0F | 0x10 | 0x11 | 0x12 | 0x13 => Cartridge::Mbc3(Mbc3::new(rom, ram_size)),

            // MBC5 / MBC5+RAM / MBC5+RAM+BATTERY / MBC5+RUMBLE /
            // MBC5+RUMBLE+RAM / MBC5+RUMBLE+RAM+BATTERY
            0x19 | 0x1A | 0x1B | 0x1C | 0x1D | 0x1E => Cartridge::Mbc5(Mbc5::new(rom, ram_size)),

            _ => panic!(
                "Unsupported cartridge type: 0x{:02X}",
                cartridge_type
            ),
        }
    }

    /// Decode the RAM size byte (header offset 0x0149) into a byte count.
    fn parse_ram_size(code: u8) -> usize {
        match code {
            0x00 => 0,          // No RAM
            0x01 => 0,          // Listed in header but unused
            0x02 => 8 * 1024,   // 8 KiB  (1 bank)
            0x03 => 32 * 1024,  // 32 KiB (4 banks)
            0x04 => 128 * 1024, // 128 KiB (16 banks)
            0x05 => 64 * 1024,  // 64 KiB (8 banks)
            _ => 0,
        }
    }

    /// Read a byte from the ROM area (0x0000-0x7FFF).
    pub fn read(&self, addr: u16) -> u8 {
        match self {
            Cartridge::NoMbc(c) => c.read(addr),
            Cartridge::Mbc1(c) => c.read(addr),
            Cartridge::Mbc2(c) => c.read(addr),
            Cartridge::Mbc3(c) => c.read(addr),
            Cartridge::Mbc5(c) => c.read(addr),
        }
    }

    /// Write to the ROM area (0x0000-0x7FFF) for bank switching / register writes.
    pub fn write(&mut self, addr: u16, value: u8) {
        match self {
            Cartridge::NoMbc(c) => c.write(addr, value),
            Cartridge::Mbc1(c) => c.write(addr, value),
            Cartridge::Mbc2(c) => c.write(addr, value),
            Cartridge::Mbc3(c) => c.write(addr, value),
            Cartridge::Mbc5(c) => c.write(addr, value),
        }
    }

    /// Read a byte from external RAM (0xA000-0xBFFF).
    pub fn read_ram(&self, addr: u16) -> u8 {
        match self {
            Cartridge::NoMbc(c) => c.read_ram(addr),
            Cartridge::Mbc1(c) => c.read_ram(addr),
            Cartridge::Mbc2(c) => c.read_ram(addr),
            Cartridge::Mbc3(c) => c.read_ram(addr),
            Cartridge::Mbc5(c) => c.read_ram(addr),
        }
    }

    /// Write a byte to external RAM (0xA000-0xBFFF).
    pub fn write_ram(&mut self, addr: u16, value: u8) {
        match self {
            Cartridge::NoMbc(c) => c.write_ram(addr, value),
            Cartridge::Mbc1(c) => c.write_ram(addr, value),
            Cartridge::Mbc2(c) => c.write_ram(addr, value),
            Cartridge::Mbc3(c) => c.write_ram(addr, value),
            Cartridge::Mbc5(c) => c.write_ram(addr, value),
        }
    }

    /// Extract the game title from the ROM header (0x0134-0x0143).
    ///
    /// On CGB cartridges, the title may be shorter (0x0134-0x013E) with the
    /// manufacturer code and CGB flag occupying the remaining bytes. This
    /// implementation reads the full 16-byte range and trims NUL bytes.
    pub fn title(&self) -> &str {
        let rom = self.rom_data();
        let title_bytes = &rom[0x0134..=0x0143];

        // Find the first NUL or non-ASCII byte to determine string length.
        let len = title_bytes
            .iter()
            .position(|&b| b == 0 || !b.is_ascii())
            .unwrap_or(title_bytes.len());

        // Safety: we've verified all bytes up to `len` are ASCII.
        str::from_utf8(&title_bytes[..len]).unwrap_or("UNKNOWN")
    }

    /// Get a reference to the cartridge RAM contents (for save-file serialization).
    pub fn ram_data(&self) -> &[u8] {
        match self {
            Cartridge::NoMbc(c) => c.ram_data(),
            Cartridge::Mbc1(c) => c.ram_data(),
            Cartridge::Mbc2(c) => c.ram_data(),
            Cartridge::Mbc3(c) => c.ram_data(),
            Cartridge::Mbc5(c) => c.ram_data(),
        }
    }

    /// Load external RAM from a save file.
    pub fn load_ram(&mut self, data: &[u8]) {
        match self {
            Cartridge::NoMbc(c) => c.load_ram(data),
            Cartridge::Mbc1(c) => c.load_ram(data),
            Cartridge::Mbc2(c) => c.load_ram(data),
            Cartridge::Mbc3(c) => c.load_ram(data),
            Cartridge::Mbc5(c) => c.load_ram(data),
        }
    }

    /// Internal helper to get the ROM data for header parsing (used by `title()`).
    fn rom_data(&self) -> &[u8] {
        match self {
            Cartridge::NoMbc(c) => &c.rom,
            Cartridge::Mbc1(c) => &c.rom,
            Cartridge::Mbc2(c) => &c.rom,
            Cartridge::Mbc3(c) => &c.rom,
            Cartridge::Mbc5(c) => &c.rom,
        }
    }
}
