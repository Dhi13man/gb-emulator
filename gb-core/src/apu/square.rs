/// Square wave channel, used for both CH1 (with sweep) and CH2 (without sweep).
///
/// Registers (relative to channel base):
///   NRx0: Sweep (CH1 only)
///   NRx1: Duty + Length
///   NRx2: Volume Envelope
///   NRx3: Frequency low
///   NRx4: Trigger + Length enable + Frequency high

/// Duty waveforms: 8 steps each
const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1], // 12.5%
    [1, 0, 0, 0, 0, 0, 0, 1], // 25%
    [1, 0, 0, 0, 0, 1, 1, 1], // 50%
    [0, 1, 1, 1, 1, 1, 1, 0], // 75%
];

pub struct SquareChannel {
    pub enabled: bool,
    has_sweep: bool,

    // Sweep (CH1 only)
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_timer: u8,
    sweep_enabled: bool,
    shadow_freq: u16,
    sweep_negate_used: bool,

    // Duty
    duty: u8,
    duty_pos: u8,

    // Length
    length_timer: u8,
    length_enabled: bool,

    // Envelope
    envelope_volume: u8,
    envelope_direction: bool, // true = increase
    envelope_period: u8,
    envelope_timer: u8,
    volume: u8,

    // Frequency
    frequency: u16,
    timer: u16,

    // DAC
    dac_enabled: bool,
}

impl SquareChannel {
    pub fn new(has_sweep: bool) -> Self {
        Self {
            enabled: false,
            has_sweep,

            sweep_period: 0,
            sweep_negate: false,
            sweep_shift: 0,
            sweep_timer: 0,
            sweep_enabled: false,
            shadow_freq: 0,
            sweep_negate_used: false,

            duty: 0,
            duty_pos: 0,

            length_timer: 0,
            length_enabled: false,

            envelope_volume: 0,
            envelope_direction: false,
            envelope_period: 0,
            envelope_timer: 0,
            volume: 0,

            frequency: 0,
            timer: 0,

            dac_enabled: false,
        }
    }

    /// Advance the channel's frequency timer by one T-cycle.
    /// When the timer expires, advance the duty position.
    pub fn tick(&mut self) {
        if self.timer == 0 {
            // Reload timer: period = (2048 - frequency) * 4
            self.timer = (2048 - self.frequency).wrapping_mul(4);
            // Advance duty position
            self.duty_pos = (self.duty_pos + 1) & 7;
        }
        self.timer = self.timer.wrapping_sub(1);
    }

    /// Return the current digital sample (0-15) considering duty, volume, DAC, and enabled state.
    pub fn sample(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }
        let duty_output = DUTY_TABLE[self.duty as usize][self.duty_pos as usize];
        if duty_output != 0 {
            self.volume
        } else {
            0
        }
    }

    /// Trigger the channel (NRx4 bit 7 written).
    pub fn trigger(&mut self) {
        self.enabled = true;

        // Reload length timer if it was zero
        if self.length_timer == 0 {
            self.length_timer = 64;
        }

        // Reload frequency timer
        self.timer = (2048 - self.frequency).wrapping_mul(4);

        // Reload envelope
        self.volume = self.envelope_volume;
        self.envelope_timer = if self.envelope_period == 0 { 8 } else { self.envelope_period };

        // Sweep initialization (CH1 only)
        if self.has_sweep {
            self.shadow_freq = self.frequency;
            self.sweep_timer = if self.sweep_period == 0 { 8 } else { self.sweep_period };
            self.sweep_enabled = self.sweep_period != 0 || self.sweep_shift != 0;
            self.sweep_negate_used = false;

            // If shift is non-zero, perform overflow check immediately
            if self.sweep_shift != 0 {
                let new_freq = self.calculate_sweep_freq();
                if new_freq > 2047 {
                    self.enabled = false;
                }
            }
        }

        // If DAC is off, channel is immediately disabled
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    /// Clock the length counter. Called at 256 Hz (frame sequencer steps 0, 2, 4, 6).
    pub fn tick_length(&mut self) {
        if self.length_enabled && self.length_timer > 0 {
            self.length_timer -= 1;
            if self.length_timer == 0 {
                self.enabled = false;
            }
        }
    }

    /// Clock the volume envelope. Called at 64 Hz (frame sequencer step 7).
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

    /// Clock the frequency sweep (CH1 only). Called at 128 Hz (frame sequencer steps 2, 6).
    pub fn tick_sweep(&mut self) {
        if !self.has_sweep {
            return;
        }

        if self.sweep_timer > 0 {
            self.sweep_timer -= 1;
        }

        if self.sweep_timer == 0 {
            self.sweep_timer = if self.sweep_period == 0 { 8 } else { self.sweep_period };

            if self.sweep_enabled && self.sweep_period != 0 {
                let new_freq = self.calculate_sweep_freq();
                if new_freq <= 2047 && self.sweep_shift != 0 {
                    self.frequency = new_freq;
                    self.shadow_freq = new_freq;

                    // Overflow check again with new frequency
                    let next_freq = self.calculate_sweep_freq();
                    if next_freq > 2047 {
                        self.enabled = false;
                    }
                }
                if new_freq > 2047 {
                    self.enabled = false;
                }
            }
        }
    }

    /// Calculate the next sweep frequency. Also performs the overflow check.
    fn calculate_sweep_freq(&mut self) -> u16 {
        let shifted = self.shadow_freq >> self.sweep_shift;
        if self.sweep_negate {
            self.sweep_negate_used = true;
            self.shadow_freq.wrapping_sub(shifted)
        } else {
            self.shadow_freq.wrapping_add(shifted)
        }
    }

    /// Read a channel register by index (0-4 relative to channel base).
    ///
    /// For CH1: index 0 = NR10 (FF10), 1 = NR11 (FF11), etc.
    /// For CH2: index 0 = unused, 1 = NR21 (FF16), etc.
    pub fn read_reg(&self, reg_index: u8) -> u8 {
        match reg_index {
            // NRx0: Sweep (CH1) or unused (CH2)
            0 => {
                if self.has_sweep {
                    0x80 // bit 7 unused
                        | (self.sweep_period << 4)
                        | if self.sweep_negate { 0x08 } else { 0 }
                        | self.sweep_shift
                } else {
                    0xFF
                }
            }
            // NRx1: Duty + Length (write-only length bits, so OR with 0x3F)
            1 => (self.duty << 6) | 0x3F,
            // NRx2: Volume Envelope
            2 => {
                (self.envelope_volume << 4)
                    | if self.envelope_direction { 0x08 } else { 0 }
                    | self.envelope_period
            }
            // NRx3: Frequency low (write-only)
            3 => 0xFF,
            // NRx4: Trigger (write-only) + Length enable + Frequency high (write-only)
            4 => 0xBF | if self.length_enabled { 0x40 } else { 0 },
            _ => 0xFF,
        }
    }

    /// Write a channel register by index (0-4 relative to channel base).
    pub fn write_reg(&mut self, reg_index: u8, value: u8) {
        match reg_index {
            // NRx0: Sweep
            0 => {
                if self.has_sweep {
                    self.sweep_period = (value >> 4) & 0x07;
                    let new_negate = value & 0x08 != 0;
                    // Obscure behavior: switching from negate to non-negate mode
                    // after negate was used disables the channel
                    if self.sweep_negate_used && self.sweep_negate && !new_negate {
                        self.enabled = false;
                    }
                    self.sweep_negate = new_negate;
                    self.sweep_shift = value & 0x07;
                }
            }
            // NRx1: Duty + Length
            1 => {
                self.duty = (value >> 6) & 0x03;
                self.length_timer = 64 - (value & 0x3F);
            }
            // NRx2: Volume Envelope
            2 => {
                self.envelope_volume = (value >> 4) & 0x0F;
                self.envelope_direction = value & 0x08 != 0;
                self.envelope_period = value & 0x07;

                // DAC is enabled if bits 3-7 are not all zero
                self.dac_enabled = value & 0xF8 != 0;
                if !self.dac_enabled {
                    self.enabled = false;
                }
            }
            // NRx3: Frequency low bits
            3 => {
                self.frequency = (self.frequency & 0x700) | value as u16;
            }
            // NRx4: Trigger + Length enable + Frequency high bits
            4 => {
                self.frequency = (self.frequency & 0x0FF) | (((value & 0x07) as u16) << 8);
                self.length_enabled = value & 0x40 != 0;

                // Trigger
                if value & 0x80 != 0 {
                    self.trigger();
                }
            }
            _ => {}
        }
    }

    /// Reset all channel state (called when NR52 bit 7 is cleared).
    pub fn reset(&mut self) {
        let has_sweep = self.has_sweep;
        *self = Self::new(has_sweep);
    }
}
