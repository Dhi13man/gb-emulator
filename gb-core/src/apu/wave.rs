/// Wave channel (CH3): Plays samples from a 32-entry wave table.
///
/// Registers:
///   FF1A (NR30): DAC enable
///   FF1B (NR31): Length
///   FF1C (NR32): Volume code
///   FF1D (NR33): Frequency low
///   FF1E (NR34): Trigger + Length enable + Frequency high
///   FF30-FF3F: Wave RAM (16 bytes = 32 4-bit samples)

pub struct WaveChannel {
    pub enabled: bool,
    dac_enabled: bool,
    length_timer: u16,
    length_enabled: bool,
    volume_code: u8,
    frequency: u16,
    timer: u16,
    position: u8,
    wave_ram: [u8; 16],
    sample_buffer: u8,
}

impl WaveChannel {
    pub fn new() -> Self {
        Self {
            enabled: false,
            dac_enabled: false,
            length_timer: 0,
            length_enabled: false,
            volume_code: 0,
            frequency: 0,
            timer: 0,
            position: 0,
            wave_ram: [0; 16],
            sample_buffer: 0,
        }
    }

    /// Advance the channel's frequency timer by one T-cycle.
    /// When the timer expires, advance the sample position in wave RAM.
    pub fn tick(&mut self) {
        if self.timer == 0 {
            // Reload timer: period = (2048 - frequency) * 2
            self.timer = (2048 - self.frequency).wrapping_mul(2);
            // Advance position through the 32 samples
            self.position = (self.position + 1) & 31;
            // Read the current sample from wave RAM
            let byte_index = (self.position / 2) as usize;
            self.sample_buffer = if self.position & 1 == 0 {
                // High nibble
                (self.wave_ram[byte_index] >> 4) & 0x0F
            } else {
                // Low nibble
                self.wave_ram[byte_index] & 0x0F
            };
        }
        self.timer = self.timer.wrapping_sub(1);
    }

    /// Return the current digital sample (0-15) with volume applied.
    pub fn sample(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }

        // Volume shift: 0=mute, 1=100% (>>0), 2=50% (>>1), 3=25% (>>2)
        match self.volume_code {
            0 => 0,
            1 => self.sample_buffer,
            2 => self.sample_buffer >> 1,
            3 => self.sample_buffer >> 2,
            _ => 0,
        }
    }

    /// Trigger the channel (NR34 bit 7 written).
    pub fn trigger(&mut self) {
        self.enabled = true;

        if self.length_timer == 0 {
            self.length_timer = 256;
        }

        // Reload frequency timer
        self.timer = (2048 - self.frequency).wrapping_mul(2);

        // Reset wave position
        self.position = 0;

        // If DAC is off, channel remains disabled
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    /// Clock the length counter. Called at 256 Hz.
    pub fn tick_length(&mut self) {
        if self.length_enabled && self.length_timer > 0 {
            self.length_timer -= 1;
            if self.length_timer == 0 {
                self.enabled = false;
            }
        }
    }

    /// Read a channel register by index (0-4 relative to channel base).
    ///
    /// Index 0 = NR30 (FF1A), 1 = NR31 (FF1B), etc.
    pub fn read_reg(&self, reg_index: u8) -> u8 {
        match reg_index {
            // NR30: DAC enable
            0 => 0x7F | if self.dac_enabled { 0x80 } else { 0 },
            // NR31: Length (write-only)
            1 => 0xFF,
            // NR32: Volume code
            2 => 0x9F | (self.volume_code << 5),
            // NR33: Frequency low (write-only)
            3 => 0xFF,
            // NR34: Trigger (write-only) + Length enable + Frequency high (write-only)
            4 => 0xBF | if self.length_enabled { 0x40 } else { 0 },
            _ => 0xFF,
        }
    }

    /// Write a channel register by index (0-4 relative to channel base).
    pub fn write_reg(&mut self, reg_index: u8, value: u8) {
        match reg_index {
            // NR30: DAC enable
            0 => {
                self.dac_enabled = value & 0x80 != 0;
                if !self.dac_enabled {
                    self.enabled = false;
                }
            }
            // NR31: Length
            1 => {
                self.length_timer = 256 - value as u16;
            }
            // NR32: Volume code
            2 => {
                self.volume_code = (value >> 5) & 0x03;
            }
            // NR33: Frequency low bits
            3 => {
                self.frequency = (self.frequency & 0x700) | value as u16;
            }
            // NR34: Trigger + Length enable + Frequency high bits
            4 => {
                self.frequency = (self.frequency & 0x0FF) | (((value & 0x07) as u16) << 8);
                self.length_enabled = value & 0x40 != 0;

                if value & 0x80 != 0 {
                    self.trigger();
                }
            }
            _ => {}
        }
    }

    /// Read a byte from wave RAM (address FF30-FF3F).
    pub fn read_wave_ram(&self, addr: u16) -> u8 {
        let offset = (addr - 0xFF30) as usize;
        if offset < 16 {
            self.wave_ram[offset]
        } else {
            0xFF
        }
    }

    /// Write a byte to wave RAM (address FF30-FF3F).
    pub fn write_wave_ram(&mut self, addr: u16, val: u8) {
        let offset = (addr - 0xFF30) as usize;
        if offset < 16 {
            self.wave_ram[offset] = val;
        }
    }

    /// Reset all channel state (called when NR52 bit 7 is cleared).
    pub fn reset(&mut self) {
        // Wave RAM is preserved across resets
        let wave_ram = self.wave_ram;
        *self = Self::new();
        self.wave_ram = wave_ram;
    }
}
