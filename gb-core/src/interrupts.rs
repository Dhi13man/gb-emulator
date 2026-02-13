/// Interrupt bit positions.
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Interrupt {
    VBlank = 0,
    LcdStat = 1,
    Timer = 2,
    Serial = 3,
    Joypad = 4,
}

impl Interrupt {
    pub fn vector(self) -> u16 {
        match self {
            Interrupt::VBlank => 0x0040,
            Interrupt::LcdStat => 0x0048,
            Interrupt::Timer => 0x0050,
            Interrupt::Serial => 0x0058,
            Interrupt::Joypad => 0x0060,
        }
    }
}

pub struct InterruptController {
    /// Interrupt Enable (0xFFFF)
    pub ie: u8,
    /// Interrupt Flag (0xFF0F)
    pub if_reg: u8,
    /// Interrupt Master Enable (not memory-mapped)
    pub ime: bool,
    /// EI was just executed; enable IME after the next instruction
    pub ei_pending: bool,
}

impl InterruptController {
    pub fn new() -> Self {
        Self {
            ie: 0x00,
            if_reg: 0xE1, // Post-boot value
            ime: false,     // Disabled after boot
            ei_pending: false,
        }
    }

    /// Request an interrupt by setting the corresponding IF bit.
    pub fn request(&mut self, interrupt: Interrupt) {
        self.if_reg |= 1 << interrupt as u8;
    }

    /// Returns the bitmask of pending interrupts (IE & IF & 0x1F).
    pub fn pending(&self) -> u8 {
        self.ie & self.if_reg & 0x1F
    }

    /// Acknowledge (clear) the highest-priority pending interrupt.
    /// Returns the interrupt vector address if one was pending.
    pub fn acknowledge(&mut self) -> Option<u16> {
        if !self.ime {
            return None;
        }

        let pending = self.pending();
        if pending == 0 {
            return None;
        }

        // Lowest set bit = highest priority
        let bit = pending.trailing_zeros() as u8;
        self.if_reg &= !(1 << bit);
        self.ime = false;

        let interrupt = match bit {
            0 => Interrupt::VBlank,
            1 => Interrupt::LcdStat,
            2 => Interrupt::Timer,
            3 => Interrupt::Serial,
            4 => Interrupt::Joypad,
            _ => unreachable!(),
        };

        Some(interrupt.vector())
    }

    /// Check if any interrupt is pending (regardless of IME).
    /// Used for exiting HALT.
    pub fn any_pending(&self) -> bool {
        self.pending() != 0
    }

    pub fn read_ie(&self) -> u8 {
        self.ie
    }

    pub fn write_ie(&mut self, value: u8) {
        self.ie = value;
    }

    pub fn read_if(&self) -> u8 {
        self.if_reg | 0xE0 // Upper 3 bits always read as 1
    }

    pub fn write_if(&mut self, value: u8) {
        self.if_reg = value & 0x1F;
    }
}
