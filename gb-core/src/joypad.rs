#[derive(Clone, Copy)]
pub enum Button {
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start,
}

pub struct Joypad {
    /// Bit 4: select direction keys, Bit 5: select action buttons
    select: u8,
    /// Direction button state (0 = pressed): Right, Left, Up, Down
    directions: u8,
    /// Action button state (0 = pressed): A, B, Select, Start
    actions: u8,
    /// Whether a joypad interrupt should be requested
    pub interrupt_pending: bool,
}

impl Joypad {
    pub fn new() -> Self {
        Self {
            select: 0x30,
            directions: 0x0F,
            actions: 0x0F,
            interrupt_pending: false,
        }
    }

    pub fn read(&self) -> u8 {
        let mut lower = 0x0F; // all buttons released
        if self.select & 0x10 == 0 {
            lower &= self.directions;
        }
        if self.select & 0x20 == 0 {
            lower &= self.actions;
        }
        self.select | 0xC0 | lower
    }

    pub fn write(&mut self, value: u8) {
        self.select = value & 0x30;
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        let (reg, bit) = match button {
            Button::Right => (&mut self.directions, 0),
            Button::Left => (&mut self.directions, 1),
            Button::Up => (&mut self.directions, 2),
            Button::Down => (&mut self.directions, 3),
            Button::A => (&mut self.actions, 0),
            Button::B => (&mut self.actions, 1),
            Button::Select => (&mut self.actions, 2),
            Button::Start => (&mut self.actions, 3),
        };

        let old = *reg;
        if pressed {
            *reg &= !(1 << bit); // 0 = pressed
        } else {
            *reg |= 1 << bit; // 1 = released
        }

        // Interrupt on high-to-low transition (button press)
        if old & (1 << bit) != 0 && pressed {
            self.interrupt_pending = true;
        }
    }
}
