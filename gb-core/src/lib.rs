#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod apu;
pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod dma;
pub mod interrupts;
pub mod joypad;
pub mod ppu;
pub mod serial;
pub mod timer;

use bus::Bus;
use cpu::Cpu;

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;
pub const CYCLES_PER_FRAME: u32 = 70224;

pub struct GameBoy {
    cpu: Cpu,
    bus: Bus,
}

impl GameBoy {
    pub fn new(rom: alloc::vec::Vec<u8>) -> Self {
        let cartridge = cartridge::Cartridge::new(rom);
        let mut bus = Bus::new(cartridge);
        let cpu = Cpu::new();

        // Initialize post-boot register/IO state
        bus.init_post_boot();

        Self { cpu, bus }
    }

    /// Run the emulator for one full frame (70224 T-cycles).
    /// Returns the number of T-cycles actually executed.
    pub fn run_frame(&mut self) -> u32 {
        let mut frame_cycles: u32 = 0;

        while frame_cycles < CYCLES_PER_FRAME {
            let cycles = self.step();
            frame_cycles += cycles;
        }

        frame_cycles
    }

    /// Execute a single CPU step and tick all subsystems.
    /// Returns T-cycles consumed.
    pub fn step(&mut self) -> u32 {
        let cycles = self.cpu.step(&mut self.bus);

        self.bus.tick(cycles);

        cycles
    }

    /// Get a reference to the PPU's frame buffer (160x144, 2-bit color indices).
    pub fn frame_buffer(&self) -> &[[u8; SCREEN_WIDTH]; SCREEN_HEIGHT] {
        self.bus.ppu.frame_buffer()
    }

    /// Get pending audio samples and drain the buffer.
    pub fn audio_buffer(&mut self) -> alloc::vec::Vec<(f32, f32)> {
        self.bus.apu.drain_samples()
    }

    /// Update joypad button state.
    pub fn set_button(&mut self, button: joypad::Button, pressed: bool) {
        self.bus.joypad.set_button(button, pressed);
    }
}
