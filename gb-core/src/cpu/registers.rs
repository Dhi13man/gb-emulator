/// CPU Flags (stored in the F register, upper 4 bits only).
#[derive(Clone, Copy, Default)]
pub struct Flags {
    pub zero: bool,
    pub subtract: bool,
    pub half_carry: bool,
    pub carry: bool,
}

impl From<Flags> for u8 {
    fn from(f: Flags) -> u8 {
        (if f.zero { 0x80 } else { 0 })
            | (if f.subtract { 0x40 } else { 0 })
            | (if f.half_carry { 0x20 } else { 0 })
            | (if f.carry { 0x10 } else { 0 })
    }
}

impl From<u8> for Flags {
    fn from(byte: u8) -> Flags {
        Flags {
            zero: byte & 0x80 != 0,
            subtract: byte & 0x40 != 0,
            half_carry: byte & 0x20 != 0,
            carry: byte & 0x10 != 0,
        }
    }
}

/// SM83 CPU register file.
pub struct Registers {
    pub a: u8,
    pub f: Flags,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

impl Registers {
    /// Post-boot register state (DMG).
    pub fn new() -> Self {
        Self {
            a: 0x01,
            f: Flags::from(0xB0),
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
        }
    }

    // --- 16-bit register pair accessors ---

    pub fn af(&self) -> u16 {
        (self.a as u16) << 8 | u8::from(self.f) as u16
    }

    pub fn set_af(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        self.f = Flags::from((value & 0xF0) as u8); // Lower 4 bits always 0
    }

    pub fn bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }

    pub fn set_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = value as u8;
    }

    pub fn de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }

    pub fn set_de(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = value as u8;
    }

    pub fn hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }

    pub fn set_hl(&mut self, value: u16) {
        self.h = (value >> 8) as u8;
        self.l = value as u8;
    }
}
