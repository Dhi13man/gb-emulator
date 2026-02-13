/// Noise channel (CH4): Generates pseudo-random noise via a linear feedback
/// shift register (LFSR).
///
/// Registers:
///   FF20 (NR41): Length
///   FF21 (NR42): Volume Envelope
///   FF22 (NR43): Polynomial counter (clock shift, width mode, divisor code)
///   FF23 (NR44): Trigger + Length enable

/// Divisor table: divisor_code 0 uses 8, not 0.
const DIVISOR_TABLE: [u32; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

pub struct NoiseChannel {
    pub enabled: bool,
    pub(crate) dac_enabled: bool,

    // Length
    length_timer: u8,
    length_enabled: bool,

    // Envelope
    envelope_volume: u8,
    envelope_direction: bool,
    envelope_period: u8,
    envelope_timer: u8,
    volume: u8,

    // Polynomial counter
    clock_shift: u8,
    width_mode: bool, // false = 15-bit LFSR, true = 7-bit LFSR
    divisor_code: u8,

    // Timer
    timer: u32,

    // LFSR (15-bit linear feedback shift register)
    lfsr: u16,
}

impl NoiseChannel {
    pub fn new() -> Self {
        Self {
            enabled: false,
            dac_enabled: false,

            length_timer: 0,
            length_enabled: false,

            envelope_volume: 0,
            envelope_direction: false,
            envelope_period: 0,
            envelope_timer: 0,
            volume: 0,

            clock_shift: 0,
            width_mode: false,
            divisor_code: 0,

            timer: 0,
            lfsr: 0x7FFF, // All bits set
        }
    }

    /// Advance the channel's timer by one T-cycle.
    /// When the timer expires, clock the LFSR.
    pub fn tick(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period();
            self.clock_lfsr();
        }
        self.timer = self.timer.wrapping_sub(1);
    }

    /// Clock the LFSR: XOR bits 0 and 1, shift right, put result in bit 14.
    /// If width_mode is set, also put result in bit 6.
    fn clock_lfsr(&mut self) {
        let xor_result = (self.lfsr & 0x01) ^ ((self.lfsr >> 1) & 0x01);

        // Shift right by 1
        self.lfsr >>= 1;

        // Put XOR result into bit 14
        self.lfsr |= xor_result << 14;

        // If 7-bit width mode, also put result into bit 6
        if self.width_mode {
            self.lfsr &= !(1 << 6); // Clear bit 6
            self.lfsr |= xor_result << 6;
        }
    }

    /// Calculate the timer period from divisor code and clock shift.
    fn timer_period(&self) -> u32 {
        DIVISOR_TABLE[self.divisor_code as usize] << self.clock_shift
    }

    /// Return the current digital sample (0-15).
    /// The output is the inverted bit 0 of the LFSR multiplied by volume.
    pub fn sample(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }

        // Bit 0 inverted: when bit 0 is 0, output is high (volume); when 1, output is 0
        if self.lfsr & 0x01 == 0 {
            self.volume
        } else {
            0
        }
    }

    /// Trigger the channel (NR44 bit 7 written).
    pub fn trigger(&mut self) {
        self.enabled = true;

        if self.length_timer == 0 {
            self.length_timer = 64;
        }

        // Reload timer
        self.timer = self.timer_period();

        // Reload envelope
        self.volume = self.envelope_volume;
        self.envelope_timer = if self.envelope_period == 0 { 8 } else { self.envelope_period };

        // Reset LFSR: all 15 bits set
        self.lfsr = 0x7FFF;

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

    /// Clock the volume envelope. Called at 64 Hz.
    pub fn tick_envelope(&mut self) {
        if self.envelope_period == 0 {
            return;
        }

        if self.envelope_timer > 0 {
            self.envelope_timer -= 1;
        }

        if self.envelope_timer == 0 {
            self.envelope_timer = if self.envelope_period == 0 { 8 } else { self.envelope_period };

            if self.envelope_direction && self.volume < 15 {
                self.volume += 1;
            } else if !self.envelope_direction && self.volume > 0 {
                self.volume -= 1;
            }
        }
    }

    /// Read a channel register by index (0-3 relative to channel base).
    ///
    /// Index 0 = NR41 (FF20), 1 = NR42 (FF21), 2 = NR43 (FF22), 3 = NR44 (FF23).
    pub fn read_reg(&self, reg_index: u8) -> u8 {
        match reg_index {
            // NR41: Length (write-only)
            0 => 0xFF,
            // NR42: Volume Envelope
            1 => {
                (self.envelope_volume << 4)
                    | if self.envelope_direction { 0x08 } else { 0 }
                    | self.envelope_period
            }
            // NR43: Polynomial counter
            2 => {
                (self.clock_shift << 4)
                    | if self.width_mode { 0x08 } else { 0 }
                    | self.divisor_code
            }
            // NR44: Trigger (write-only) + Length enable
            3 => 0xBF | if self.length_enabled { 0x40 } else { 0 },
            _ => 0xFF,
        }
    }

    /// Write a channel register by index (0-3 relative to channel base).
    pub fn write_reg(&mut self, reg_index: u8, value: u8) {
        match reg_index {
            // NR41: Length
            0 => {
                self.length_timer = 64 - (value & 0x3F);
            }
            // NR42: Volume Envelope
            1 => {
                self.envelope_volume = (value >> 4) & 0x0F;
                self.envelope_direction = value & 0x08 != 0;
                self.envelope_period = value & 0x07;

                // DAC enabled when bits 3-7 are not all zero
                self.dac_enabled = value & 0xF8 != 0;
                if !self.dac_enabled {
                    self.enabled = false;
                }
            }
            // NR43: Polynomial counter
            2 => {
                self.clock_shift = (value >> 4) & 0x0F;
                self.width_mode = value & 0x08 != 0;
                self.divisor_code = value & 0x07;
            }
            // NR44: Trigger + Length enable
            3 => {
                self.length_enabled = value & 0x40 != 0;

                if value & 0x80 != 0 {
                    self.trigger();
                }
            }
            _ => {}
        }
    }

    /// Reset all channel state (called when NR52 bit 7 is cleared).
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}
