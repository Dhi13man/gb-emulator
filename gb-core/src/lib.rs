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

    /// Set the APU output sample rate to match the audio device.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.bus.apu = apu::Apu::with_sample_rate(sample_rate);
    }

    /// Run the emulator until the PPU enters VBlank (frame buffer complete).
    /// Falls back to a cycle limit to prevent infinite loops if LCD is off.
    /// Returns the number of T-cycles actually executed.
    pub fn run_frame(&mut self) -> u32 {
        let mut frame_cycles: u32 = 0;

        // Clear any previous frame_complete flag
        self.bus.ppu.frame_complete = false;

        // Run until VBlank starts (frame buffer is complete)
        // Use a generous cycle limit (2 frames) as safety valve
        let limit = CYCLES_PER_FRAME * 2;
        while frame_cycles < limit {
            let cycles = self.step();
            frame_cycles += cycles;

            if self.bus.ppu.frame_complete {
                self.bus.ppu.frame_complete = false;
                break;
            }
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

    /// Drain serial output bytes (for test ROM debugging).
    pub fn serial_output(&mut self) -> alloc::vec::Vec<u8> {
        self.bus.serial.drain_output()
    }

    /// Enable/disable PPU debug logging.
    pub fn set_ppu_debug(&mut self, enabled: bool) {
        self.bus.ppu.debug_log = enabled;
        if enabled && self.bus.ppu.debug_buffer.is_none() {
            self.bus.ppu.debug_buffer = Some(alloc::string::String::new());
        }
    }

    /// Drain PPU debug log.
    pub fn drain_ppu_debug(&mut self) -> Option<alloc::string::String> {
        self.bus.ppu.debug_buffer.take()
    }

    /// Get current PPU frame number.
    pub fn ppu_frame_number(&self) -> u32 {
        self.bus.ppu.frame_number
    }
}
