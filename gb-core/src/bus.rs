extern crate alloc;

use crate::apu::Apu;
use crate::cartridge::Cartridge;
use crate::dma::Dma;
use crate::interrupts::{Interrupt, InterruptController};
use crate::joypad::Joypad;
use crate::ppu::Ppu;
use crate::serial::Serial;
use crate::timer::Timer;

/// Memory bus: routes reads and writes to the appropriate subsystem.
pub struct Bus {
    pub cartridge: Cartridge,
    pub ppu: Ppu,
    pub apu: Apu,
    pub timer: Timer,
    pub interrupts: InterruptController,
    pub joypad: Joypad,
    pub serial: Serial,
    pub dma: Dma,
    wram: [u8; 0x2000],
    hram: [u8; 0x7F],
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            cartridge,
            ppu: Ppu::new(),
            apu: Apu::new(),
            timer: Timer::new(),
            interrupts: InterruptController::new(),
            joypad: Joypad::new(),
            serial: Serial::new(),
            dma: Dma::new(),
            wram: [0; 0x2000],
            hram: [0; 0x7F],
        }
    }

    /// Initialize I/O registers to post-boot ROM state.
    pub fn init_post_boot(&mut self) {
        // Timer
        self.timer.write(0xFF05, 0x00); // TIMA
        self.timer.write(0xFF06, 0x00); // TMA
        self.timer.write(0xFF07, 0x00); // TAC

        // Audio - set key registers to post-boot values
        self.apu.write(0xFF10, 0x80);
        self.apu.write(0xFF11, 0xBF);
        self.apu.write(0xFF12, 0xF3);
        self.apu.write(0xFF14, 0xBF);
        self.apu.write(0xFF16, 0x3F);
        self.apu.write(0xFF17, 0x00);
        self.apu.write(0xFF19, 0xBF);
        self.apu.write(0xFF1A, 0x7F);
        self.apu.write(0xFF1B, 0xFF);
        self.apu.write(0xFF1C, 0x9F);
        self.apu.write(0xFF1E, 0xBF);
        self.apu.write(0xFF20, 0xFF);
        self.apu.write(0xFF21, 0x00);
        self.apu.write(0xFF22, 0x00);
        self.apu.write(0xFF23, 0xBF);
        self.apu.write(0xFF24, 0x77);
        self.apu.write(0xFF25, 0xF3);
        self.apu.write(0xFF26, 0xF1);

        // PPU
        self.ppu.write_reg(0xFF40, 0x91); // LCDC
        self.ppu.write_reg(0xFF42, 0x00); // SCY
        self.ppu.write_reg(0xFF43, 0x00); // SCX
        self.ppu.write_reg(0xFF45, 0x00); // LYC
        self.ppu.write_reg(0xFF47, 0xFC); // BGP
        self.ppu.write_reg(0xFF48, 0xFF); // OBP0
        self.ppu.write_reg(0xFF49, 0xFF); // OBP1
        self.ppu.write_reg(0xFF4A, 0x00); // WY
        self.ppu.write_reg(0xFF4B, 0x00); // WX

        // Interrupts
        self.interrupts.write_if(0xE1);
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            // ROM bank 0 + switchable bank
            0x0000..=0x7FFF => self.cartridge.read(addr),

            // VRAM
            0x8000..=0x9FFF => self.ppu.read_vram(addr),

            // External RAM
            0xA000..=0xBFFF => self.cartridge.read_ram(addr),

            // Work RAM
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],

            // Echo RAM (mirror of C000-DDFF)
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize],

            // OAM
            0xFE00..=0xFE9F => self.ppu.read_oam(addr),

            // Unusable
            0xFEA0..=0xFEFF => 0xFF,

            // I/O Registers
            0xFF00 => self.joypad.read(),
            0xFF01..=0xFF02 => self.serial.read(addr),
            0xFF03 => 0xFF,
            0xFF04..=0xFF07 => self.timer.read(addr),
            0xFF08..=0xFF0E => 0xFF,
            0xFF0F => self.interrupts.read_if(),
            0xFF10..=0xFF3F => self.apu.read(addr),
            0xFF40..=0xFF45 => self.ppu.read_reg(addr),
            0xFF46 => self.dma.read(),
            0xFF47..=0xFF4B => self.ppu.read_reg(addr),
            0xFF4C..=0xFF7F => 0xFF, // Unused I/O

            // HRAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],

            // Interrupt Enable
            0xFFFF => self.interrupts.read_ie(),
        }
    }

    pub fn write_byte(&mut self, addr: u16, value: u8) {
        match addr {
            // ROM area (writes go to MBC for bank switching)
            0x0000..=0x7FFF => self.cartridge.write(addr, value),

            // VRAM
            0x8000..=0x9FFF => self.ppu.write_vram(addr, value),

            // External RAM
            0xA000..=0xBFFF => self.cartridge.write_ram(addr, value),

            // Work RAM
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = value,

            // Echo RAM
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = value,

            // OAM
            0xFE00..=0xFE9F => self.ppu.write_oam(addr, value),

            // Unusable
            0xFEA0..=0xFEFF => {}

            // I/O Registers
            0xFF00 => self.joypad.write(value),
            0xFF01..=0xFF02 => self.serial.write(addr, value),
            0xFF03 => {}
            0xFF04..=0xFF07 => self.timer.write(addr, value),
            0xFF08..=0xFF0E => {}
            0xFF0F => self.interrupts.write_if(value),
            0xFF10..=0xFF3F => self.apu.write(addr, value),
            0xFF40..=0xFF45 => self.ppu.write_reg(addr, value),
            0xFF46 => {
                self.dma.write(value);
            }
            0xFF47..=0xFF4B => self.ppu.write_reg(addr, value),
            0xFF4C..=0xFF7F => {} // Unused I/O

            // HRAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = value,

            // Interrupt Enable
            0xFFFF => self.interrupts.write_ie(value),
        }
    }

    /// Tick all subsystems by the given number of T-cycles.
    pub fn tick(&mut self, cycles: u32) {
        // Timer
        self.timer.tick(cycles);
        if self.timer.interrupt_pending {
            self.timer.interrupt_pending = false;
            self.interrupts.request(Interrupt::Timer);
        }

        // PPU
        self.ppu.step(cycles);
        if self.ppu.vblank_interrupt {
            self.ppu.vblank_interrupt = false;
            self.interrupts.request(Interrupt::VBlank);
        }
        if self.ppu.stat_interrupt {
            self.ppu.stat_interrupt = false;
            self.interrupts.request(Interrupt::LcdStat);
        }

        // APU
        self.apu.step(cycles);

        // Serial
        self.serial.tick(cycles);
        if self.serial.interrupt_pending {
            self.serial.interrupt_pending = false;
            self.interrupts.request(Interrupt::Serial);
        }

        // Joypad
        if self.joypad.interrupt_pending {
            self.joypad.interrupt_pending = false;
            self.interrupts.request(Interrupt::Joypad);
        }

        // OAM DMA
        let transfers = self.dma.tick(cycles);
        for (src_addr, oam_offset) in transfers {
            let value = self.dma_read(src_addr);
            self.ppu.write_oam(0xFE00 + oam_offset as u16, value);
        }
    }

    /// Read for DMA (can access most memory, but not OAM or I/O during DMA).
    fn dma_read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.cartridge.read(addr),
            0x8000..=0x9FFF => self.ppu.read_vram(addr),
            0xA000..=0xBFFF => self.cartridge.read_ram(addr),
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize],
            _ => 0xFF,
        }
    }
}
