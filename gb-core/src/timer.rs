/// Timer subsystem: DIV (FF04), TIMA (FF05), TMA (FF06), TAC (FF07).
///
/// The timer is driven by a 16-bit internal counter (div_counter) that
/// increments every T-cycle. DIV exposes the upper 8 bits. TIMA increments
/// on a falling edge of (selected_counter_bit AND timer_enable).
pub struct Timer {
    /// Internal 16-bit counter; DIV = bits 8-15
    div_counter: u16,
    /// Timer counter (TIMA)
    tima: u8,
    /// Timer modulo (TMA) - value loaded into TIMA on overflow
    tma: u8,
    /// Timer control (TAC)
    tac: u8,
    /// Previous AND result for falling edge detection
    prev_and_result: bool,
    /// Countdown for delayed TIMA reload after overflow (in T-cycles)
    /// -1 = inactive, 4..1 = counting down, 0 = reload this cycle
    overflow_countdown: i8,
    pub interrupt_pending: bool,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            div_counter: 0xABCC, // Post-boot value
            tima: 0x00,
            tma: 0x00,
            tac: 0xF8,
            prev_and_result: false,
            overflow_countdown: -1,
            interrupt_pending: false,
        }
    }

    /// Advance the timer by the given number of T-cycles.
    pub fn tick(&mut self, cycles: u32) {
        for _ in 0..cycles {
            self.tick_one();
        }
    }

    fn tick_one(&mut self) {
        self.div_counter = self.div_counter.wrapping_add(1);

        // Handle delayed TIMA reload
        if self.overflow_countdown >= 0 {
            self.overflow_countdown -= 1;
            if self.overflow_countdown < 0 {
                self.tima = self.tma;
                self.interrupt_pending = true;
            }
        }

        self.update_falling_edge();
    }

    fn update_falling_edge(&mut self) {
        let bit_pos = match self.tac & 0x03 {
            0 => 9,
            1 => 3,
            2 => 5,
            3 => 7,
            _ => unreachable!(),
        };
        let timer_enabled = (self.tac & 0x04) != 0;
        let bit = (self.div_counter >> bit_pos) & 1 != 0;
        let and_result = bit && timer_enabled;

        // Falling edge: was 1, now 0
        if self.prev_and_result && !and_result {
            let (new_tima, overflow) = self.tima.overflowing_add(1);
            self.tima = new_tima;
            if overflow {
                // TIMA overflowed: reload delayed by 4 T-cycles (1 M-cycle)
                self.tima = 0x00; // Reads as 0 during the delay
                self.overflow_countdown = 3; // Will reload after 4 more ticks
            }
        }

        self.prev_and_result = and_result;
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (self.div_counter >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac | 0xF8, // Upper 5 bits read as 1
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF04 => {
                // Writing ANY value resets the entire 16-bit counter to 0.
                // This can trigger a TIMA increment via falling edge!
                self.div_counter = 0;
                self.update_falling_edge();
            }
            0xFF05 => {
                // Writing to TIMA during overflow countdown cancels the reload
                if self.overflow_countdown >= 0 {
                    self.overflow_countdown = -1;
                }
                self.tima = value;
            }
            0xFF06 => {
                self.tma = value;
                // If TMA is written on the same cycle as reload, new value goes to TIMA
                if self.overflow_countdown == -1 && self.tima == self.tma {
                    // Normal case, no special handling needed
                }
            }
            0xFF07 => {
                let old_tac = self.tac;
                self.tac = value;
                // Changing TAC can trigger falling edge
                if old_tac != value {
                    self.update_falling_edge();
                }
            }
            _ => {}
        }
    }
}
