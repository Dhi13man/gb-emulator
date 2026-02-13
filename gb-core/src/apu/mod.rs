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
const DEFAULT_SAMPLE_RATE: u32 = 44_100;

/// Simple first-order high-pass filter that emulates the Game Boy's
/// capacitor coupling, removing DC offset from the audio signal.
struct HighPassFilter {
    capacitor: f32,
    factor: f32,
}

impl HighPassFilter {
    fn new(sample_rate: u32) -> Self {
        // Cutoff frequency ~20 Hz, matching Game Boy hardware coupling capacitor.
        // factor = 1 - (2*pi*cutoff / sample_rate), clamped for stability.
        let cutoff = 20.0_f32;
        let factor = 1.0 - (2.0 * core::f32::consts::PI * cutoff / sample_rate as f32);
        Self {
            capacitor: 0.0,
            factor: factor.clamp(0.9, 0.9999),
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = input - self.capacitor;
        self.capacitor = input - output * self.factor;
        output
    }
}

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

    /// T-cycles between output samples (CPU_CLOCK / sample_rate)
    downsample_period: u32,

    /// High-pass filters for left and right channels (emulates hardware capacitor coupling)
    hpf_left: HighPassFilter,
    hpf_right: HighPassFilter,
}

impl Apu {
    pub fn new() -> Self {
        Self::with_sample_rate(DEFAULT_SAMPLE_RATE)
    }

    pub fn with_sample_rate(sample_rate: u32) -> Self {
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
            downsample_period: CPU_CLOCK / sample_rate,
            hpf_left: HighPassFilter::new(sample_rate),
            hpf_right: HighPassFilter::new(sample_rate),
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
        if self.sample_clock >= self.downsample_period {
            self.sample_clock -= self.downsample_period;
            let (left, right) = self.mix();
            self.sample_buffer.push((left, right));
        }
    }

    /// Mix all four channels into a stereo (left, right) sample pair.
    /// Applies DAC centering, NR51 panning, NR50 master volume, and high-pass filtering.
    /// Returns normalized f32 values in approximately [-1.0, 1.0].
    fn mix(&mut self) -> (f32, f32) {
        if self.nr52 & 0x80 == 0 {
            return (0.0, 0.0);
        }

        // Convert each channel's digital output (0-15) to centered DAC output.
        // When DAC is enabled: maps 0→-1.0, 7.5→0.0, 15→+1.0
        // When DAC is disabled: output is 0.0 (disconnected from mixer)
        #[inline]
        fn dac_output(sample: u8, dac_enabled: bool) -> f32 {
            if !dac_enabled {
                return 0.0;
            }
            (sample as f32 / 7.5) - 1.0
        }

        let ch1 = dac_output(self.ch1.sample(), self.ch1.dac_enabled);
        let ch2 = dac_output(self.ch2.sample(), self.ch2.dac_enabled);
        let ch3 = dac_output(self.ch3.sample(), self.ch3.dac_enabled);
        let ch4 = dac_output(self.ch4.sample(), self.ch4.dac_enabled);

        // Apply NR51 panning
        let mut left: f32 = 0.0;
        let mut right: f32 = 0.0;

        if self.nr51 & 0x10 != 0 { left += ch1; }
        if self.nr51 & 0x20 != 0 { left += ch2; }
        if self.nr51 & 0x40 != 0 { left += ch3; }
        if self.nr51 & 0x80 != 0 { left += ch4; }

        if self.nr51 & 0x01 != 0 { right += ch1; }
        if self.nr51 & 0x02 != 0 { right += ch2; }
        if self.nr51 & 0x04 != 0 { right += ch3; }
        if self.nr51 & 0x08 != 0 { right += ch4; }

        // Apply NR50 master volume (0-7, hardware adds 1 → range 1-8)
        let left_vol = ((self.nr50 >> 4) & 0x07) as f32 + 1.0;
        let right_vol = (self.nr50 & 0x07) as f32 + 1.0;

        left *= left_vol;
        right *= right_vol;

        // Normalize: max = 1.0 (centered DAC) * 4 (channels) * 8 (volume) = 32
        left /= 32.0;
        right /= 32.0;

        // High-pass filter removes DC offset (emulates hardware capacitor coupling)
        left = self.hpf_left.process(left);
        right = self.hpf_right.process(right);

        (left.clamp(-1.0, 1.0), right.clamp(-1.0, 1.0))
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
