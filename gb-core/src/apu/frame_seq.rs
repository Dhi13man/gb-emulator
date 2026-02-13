/// Frame sequencer: clocked at 512 Hz (every 8192 T-cycles).
///
/// Steps through an 8-step pattern that determines which APU components
/// (length counters, envelope, sweep) should be clocked each step.

const FRAME_SEQUENCER_PERIOD: u32 = 8192;

/// Which components should be clocked on a given frame sequencer step.
pub struct FrameClocks {
    pub length: bool,
    pub envelope: bool,
    pub sweep: bool,
}

pub struct FrameSequencer {
    /// Current step in the 0-7 pattern
    step: u8,
    /// T-cycle counter until next step
    timer: u32,
}

impl FrameSequencer {
    pub fn new() -> Self {
        Self {
            step: 0,
            timer: FRAME_SEQUENCER_PERIOD,
        }
    }

    /// Advance the frame sequencer by one T-cycle.
    /// Returns `Some(FrameClocks)` when a step boundary is reached, indicating
    /// which components should be clocked.
    pub fn tick(&mut self) -> Option<FrameClocks> {
        self.timer = self.timer.wrapping_sub(1);
        if self.timer == 0 {
            self.timer = FRAME_SEQUENCER_PERIOD;
            let clocks = self.clocks_for_step(self.step);
            self.step = (self.step + 1) & 7;
            Some(clocks)
        } else {
            None
        }
    }

    /// Determine which components to clock based on the current step.
    ///
    /// Step mapping:
    /// - 0: length
    /// - 1: nothing
    /// - 2: length + sweep
    /// - 3: nothing
    /// - 4: length
    /// - 5: nothing
    /// - 6: length + sweep
    /// - 7: envelope
    fn clocks_for_step(&self, step: u8) -> FrameClocks {
        match step {
            0 => FrameClocks { length: true,  envelope: false, sweep: false },
            1 => FrameClocks { length: false, envelope: false, sweep: false },
            2 => FrameClocks { length: true,  envelope: false, sweep: true  },
            3 => FrameClocks { length: false, envelope: false, sweep: false },
            4 => FrameClocks { length: true,  envelope: false, sweep: false },
            5 => FrameClocks { length: false, envelope: false, sweep: false },
            6 => FrameClocks { length: true,  envelope: false, sweep: true  },
            7 => FrameClocks { length: false, envelope: true,  sweep: false },
            _ => unreachable!(),
        }
    }

    /// Reset the frame sequencer to its initial state.
    pub fn reset(&mut self) {
        self.step = 0;
        self.timer = FRAME_SEQUENCER_PERIOD;
    }
}
