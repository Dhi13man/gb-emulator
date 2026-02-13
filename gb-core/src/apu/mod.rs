/// APU (Audio Processing Unit): Generates the Game Boy's sound output.
///
/// The DMG APU contains four channels:
///   CH1: Square wave with frequency sweep
///   CH2: Square wave
///   CH3: Programmable wave
///   CH4: Noise (LFSR)
///
/// All channels are mixed into stereo output via NR50 (master volume)
/// and NR51 (panning) registers. NR52 controls the master power.
///
/// Audio I/O register map: FF10-FF3F
///   FF10-FF14: CH1 (square with sweep)
///   FF16-FF19: CH2 (square, no sweep; FF15 doesn't exist)
///   FF1A-FF1E: CH3 (wave)
///   FF20-FF23: CH4 (noise)
///   FF24: NR50 (master volume / Vin routing)
///   FF25: NR51 (channel panning)
///   FF26: NR52 (master enable / channel status)
///   FF30-FF3F: Wave RAM

extern crate alloc;
use alloc::vec::Vec;

pub mod square;
pub mod wave;
pub mod noise;
pub mod frame_seq;

use square::SquareChannel;
use wave::WaveChannel;
use noise::NoiseChannel;
use frame_seq::FrameSequencer;

const CPU_CLOCK: u32 = 4_194_304;
const SAMPLE_RATE: u32 = 44_100;
/// Number of T-cycles between output samples (CPU_CLOCK / SAMPLE_RATE ≈ 95)
const DOWNSAMPLE_PERIOD: u32 = CPU_CLOCK / SAMPLE_RATE;

pub struct Apu {
    ch1: SquareChannel,
    ch2: SquareChannel,
    ch3: WaveChannel,
    ch4: NoiseChannel,
    frame_seq: FrameSequencer,

    /// NR50 (FF24): Master volume & Vin routing
    ///   Bit 7:   Vin -> left  (not implemented, no cartridge audio)
    ///   Bit 6-4: Left volume  (0-7)
    ///   Bit 3:   Vin -> right (not implemented)
    ///   Bit 2-0: Right volume (0-7)
    nr50: u8,

    /// NR51 (FF25): Channel panning
    ///   Bit 7: CH4 left    Bit 3: CH4 right
    ///   Bit 6: CH3 left    Bit 2: CH3 right
    ///   Bit 5: CH2 left    Bit 1: CH2 right
    ///   Bit 4: CH1 left    Bit 0: CH1 right
    nr51: u8,

    /// NR52 (FF26): Master enable
    ///   Bit 7:   All sound on/off
    ///   Bit 3-0: CH4-CH1 status (read-only)
    nr52: u8,

    /// Stereo sample output buffer, drained by the frontend
    sample_buffer: Vec<(f32, f32)>,

    /// Counter for downsampling from CPU clock to audio sample rate
    sample_clock: u32,
}

impl Apu {
    pub fn new() -> Self {
        Self {
            ch1: SquareChannel::new(true),  // CH1 has sweep
            ch2: SquareChannel::new(false), // CH2 has no sweep
            ch3: WaveChannel::new(),
            ch4: NoiseChannel::new(),
            frame_seq: FrameSequencer::new(),
            nr50: 0x77,
            nr51: 0xF3,
            nr52: 0xF1,
            sample_buffer: Vec::new(),
            sample_clock: 0,
        }
    }

    /// Advance the APU by the given number of T-cycles.
    /// This ticks all channels, the frame sequencer, and collects output samples.
    pub fn step(&mut self, t_cycles: u32) {
        for _ in 0..t_cycles {
            self.tick_one();
        }
    }

    /// Advance the APU by exactly one T-cycle.
    fn tick_one(&mut self) {
        let power_on = self.nr52 & 0x80 != 0;

        if power_on {
            // Tick the frame sequencer
            if let Some(clocks) = self.frame_seq.tick() {
                if clocks.length {
                    self.ch1.tick_length();
                    self.ch2.tick_length();
                    self.ch3.tick_length();
                    self.ch4.tick_length();
                }
                if clocks.sweep {
                    self.ch1.tick_sweep();
                }
                if clocks.envelope {
                    self.ch1.tick_envelope();
                    self.ch2.tick_envelope();
                    self.ch4.tick_envelope();
                }
            }

            // Tick all channel frequency timers
            self.ch1.tick();
            self.ch2.tick();
            self.ch3.tick();
            self.ch4.tick();
        }

        // Downsample: collect a stereo sample at the target sample rate
        self.sample_clock += 1;
        if self.sample_clock >= DOWNSAMPLE_PERIOD {
            self.sample_clock -= DOWNSAMPLE_PERIOD;
            let (left, right) = self.mix();
            self.sample_buffer.push((left, right));
        }
    }

    /// Mix all four channels into a stereo (left, right) sample pair.
    /// Applies NR51 panning and NR50 master volume.
    /// Returns normalized f32 values in approximately [-1.0, 1.0].
    fn mix(&self) -> (f32, f32) {
        if self.nr52 & 0x80 == 0 {
            return (0.0, 0.0);
        }

        let ch1_sample = self.ch1.sample() as f32;
        let ch2_sample = self.ch2.sample() as f32;
        let ch3_sample = self.ch3.sample() as f32;
        let ch4_sample = self.ch4.sample() as f32;

        // Apply NR51 panning
        let mut left: f32 = 0.0;
        let mut right: f32 = 0.0;

        if self.nr51 & 0x10 != 0 { left += ch1_sample; }
        if self.nr51 & 0x20 != 0 { left += ch2_sample; }
        if self.nr51 & 0x40 != 0 { left += ch3_sample; }
        if self.nr51 & 0x80 != 0 { left += ch4_sample; }

        if self.nr51 & 0x01 != 0 { right += ch1_sample; }
        if self.nr51 & 0x02 != 0 { right += ch2_sample; }
        if self.nr51 & 0x04 != 0 { right += ch3_sample; }
        if self.nr51 & 0x08 != 0 { right += ch4_sample; }

        // Apply NR50 master volume (0-7, +1 for hardware scaling)
        let left_vol = ((self.nr50 >> 4) & 0x07) as f32 + 1.0;
        let right_vol = (self.nr50 & 0x07) as f32 + 1.0;

        left *= left_vol;
        right *= right_vol;

        // Normalize: max possible is 15 (sample) * 4 (channels) * 8 (volume) = 480
        // Scale to [-1.0, 1.0] range, centered around 0
        // DAC output: digital 0-15 maps to analog -1.0 to +1.0
        // So we first convert 0-15 to -7.5..+7.5 conceptually, but since we already
        // have the raw 0-15 values, we normalize by the maximum and center:
        // max = 15 * 4 * 8 = 480
        // We divide by 480 to get 0..1, then remap to -1..1
        let norm = 1.0 / 480.0;
        left = left * norm * 2.0 - 1.0;
        right = right * norm * 2.0 - 1.0;

        // Clamp
        left = if left > 1.0 { 1.0 } else if left < -1.0 { -1.0 } else { left };
        right = if right > 1.0 { 1.0 } else if right < -1.0 { -1.0 } else { right };

        (left, right)
    }

    /// Drain and return all accumulated audio samples.
    /// The frontend should call this once per frame.
    pub fn drain_samples(&mut self) -> Vec<(f32, f32)> {
        let mut samples = Vec::new();
        core::mem::swap(&mut samples, &mut self.sample_buffer);
        samples
    }

    /// Read an APU register (FF10-FF3F).
    pub fn read(&self, addr: u16) -> u8 {
        // If APU is powered off, only NR52 and wave RAM are readable
        if self.nr52 & 0x80 == 0 {
            return match addr {
                0xFF26 => self.read_nr52(),
                0xFF30..=0xFF3F => self.ch3.read_wave_ram(addr),
                _ => 0xFF,
            };
        }

        match addr {
            // CH1: Square with sweep (FF10-FF14)
            0xFF10 => self.ch1.read_reg(0),
            0xFF11 => self.ch1.read_reg(1),
            0xFF12 => self.ch1.read_reg(2),
            0xFF13 => self.ch1.read_reg(3),
            0xFF14 => self.ch1.read_reg(4),

            // FF15 doesn't exist
            0xFF15 => 0xFF,

            // CH2: Square without sweep (FF16-FF19)
            0xFF16 => self.ch2.read_reg(1),
            0xFF17 => self.ch2.read_reg(2),
            0xFF18 => self.ch2.read_reg(3),
            0xFF19 => self.ch2.read_reg(4),

            // CH3: Wave (FF1A-FF1E)
            0xFF1A => self.ch3.read_reg(0),
            0xFF1B => self.ch3.read_reg(1),
            0xFF1C => self.ch3.read_reg(2),
            0xFF1D => self.ch3.read_reg(3),
            0xFF1E => self.ch3.read_reg(4),

            // FF1F doesn't exist
            0xFF1F => 0xFF,

            // CH4: Noise (FF20-FF23)
            0xFF20 => self.ch4.read_reg(0),
            0xFF21 => self.ch4.read_reg(1),
            0xFF22 => self.ch4.read_reg(2),
            0xFF23 => self.ch4.read_reg(3),

            // Master control registers
            0xFF24 => self.nr50,
            0xFF25 => self.nr51,
            0xFF26 => self.read_nr52(),

            // FF27-FF2F: unused
            0xFF27..=0xFF2F => 0xFF,

            // Wave RAM (FF30-FF3F)
            0xFF30..=0xFF3F => self.ch3.read_wave_ram(addr),

            _ => 0xFF,
        }
    }

    /// Write an APU register (FF10-FF3F).
    pub fn write(&mut self, addr: u16, val: u8) {
        // Wave RAM is always writable
        if (0xFF30..=0xFF3F).contains(&addr) {
            self.ch3.write_wave_ram(addr, val);
            return;
        }

        // NR52 master control is always writable
        if addr == 0xFF26 {
            self.write_nr52(val);
            return;
        }

        // If APU is powered off, ignore all other writes
        if self.nr52 & 0x80 == 0 {
            // Exception: length counters (NRx1) can still be written on DMG
            match addr {
                0xFF11 => self.ch1.write_reg(1, val & 0x3F), // Length only, not duty
                0xFF16 => self.ch2.write_reg(1, val & 0x3F),
                0xFF1B => self.ch3.write_reg(1, val),
                0xFF20 => self.ch4.write_reg(0, val & 0x3F),
                _ => {}
            }
            return;
        }

        match addr {
            // CH1: Square with sweep
            0xFF10 => self.ch1.write_reg(0, val),
            0xFF11 => self.ch1.write_reg(1, val),
            0xFF12 => self.ch1.write_reg(2, val),
            0xFF13 => self.ch1.write_reg(3, val),
            0xFF14 => self.ch1.write_reg(4, val),

            // CH2: Square without sweep
            0xFF16 => self.ch2.write_reg(1, val),
            0xFF17 => self.ch2.write_reg(2, val),
            0xFF18 => self.ch2.write_reg(3, val),
            0xFF19 => self.ch2.write_reg(4, val),

            // CH3: Wave
            0xFF1A => self.ch3.write_reg(0, val),
            0xFF1B => self.ch3.write_reg(1, val),
            0xFF1C => self.ch3.write_reg(2, val),
            0xFF1D => self.ch3.write_reg(3, val),
            0xFF1E => self.ch3.write_reg(4, val),

            // CH4: Noise
            0xFF20 => self.ch4.write_reg(0, val),
            0xFF21 => self.ch4.write_reg(1, val),
            0xFF22 => self.ch4.write_reg(2, val),
            0xFF23 => self.ch4.write_reg(3, val),

            // Master control
            0xFF24 => self.nr50 = val,
            0xFF25 => self.nr51 = val,
            // NR52 handled above

            _ => {}
        }
    }

    /// Build the NR52 read value: bit 7 = master enable, bits 0-3 = channel status.
    fn read_nr52(&self) -> u8 {
        let mut status = 0x70; // Bits 4-6 always read as 1
        if self.nr52 & 0x80 != 0 { status |= 0x80; }
        if self.ch1.enabled { status |= 0x01; }
        if self.ch2.enabled { status |= 0x02; }
        if self.ch3.enabled { status |= 0x04; }
        if self.ch4.enabled { status |= 0x08; }
        status
    }

    /// Handle writes to NR52 (FF26). Only bit 7 is writable.
    fn write_nr52(&mut self, val: u8) {
        let was_on = self.nr52 & 0x80 != 0;
        let now_on = val & 0x80 != 0;

        if was_on && !now_on {
            // Powering off: reset all registers and channels
            self.ch1.reset();
            self.ch2.reset();
            self.ch3.reset();
            self.ch4.reset();
            self.frame_seq.reset();
            self.nr50 = 0;
            self.nr51 = 0;
        }

        // Only bit 7 is writable; lower bits are read-only status
        self.nr52 = val & 0x80;
    }
}
