use crate::bus::Bus;
use super::Cpu;

impl Cpu {
    // --- Register index helpers for CB opcode decoding ---
    //
    // The 3-bit register index (bits 2-0 of the opcode) maps to:
    //   0: B, 1: C, 2: D, 3: E, 4: H, 5: L, 6: (HL), 7: A
    //
    // Index 6 is special: it addresses the byte in memory at address HL
    // rather than a CPU register, incurring extra cycle cost.

    /// Read an 8-bit value by CB register index.
    /// Index 6 reads from memory at address HL.
    fn read_r8(&self, bus: &Bus, index: u8) -> u8 {
        match index {
            0 => self.regs.b,
            1 => self.regs.c,
            2 => self.regs.d,
            3 => self.regs.e,
            4 => self.regs.h,
            5 => self.regs.l,
            6 => bus.read_byte(self.regs.hl()),
            7 => self.regs.a,
            _ => unreachable!(),
        }
    }

    /// Write an 8-bit value by CB register index.
    /// Index 6 writes to memory at address HL.
    fn write_r8(&mut self, bus: &mut Bus, index: u8, value: u8) {
        match index {
            0 => self.regs.b = value,
            1 => self.regs.c = value,
            2 => self.regs.d = value,
            3 => self.regs.e = value,
            4 => self.regs.h = value,
            5 => self.regs.l = value,
            6 => bus.write_byte(self.regs.hl(), value),
            7 => self.regs.a = value,
            _ => unreachable!(),
        }
    }

    /// Execute a CB-prefixed opcode. Returns T-cycle cost.
    ///
    /// CB opcodes follow a perfectly regular encoding:
    ///   Bits 7-6: operation group
    ///     00 = rotate/shift
    ///     01 = BIT (test bit)
    ///     10 = RES (reset bit)
    ///     11 = SET (set bit)
    ///   Bits 5-3: bit index (BIT/RES/SET) or shift variant (rotate/shift group)
    ///   Bits 2-0: register operand (B/C/D/E/H/L/(HL)/A)
    pub(super) fn execute_cb(&mut self, bus: &mut Bus, opcode: u8) -> u32 {
        let op = (opcode >> 6) & 0x03;    // bits 7-6: operation group
        let bit = (opcode >> 3) & 0x07;   // bits 5-3: bit index / shift variant
        let reg = opcode & 0x07;           // bits 2-0: register index
        let is_hl = reg == 6;

        match op {
            // --- Rotate / Shift group ---
            0b00 => {
                let value = self.read_r8(bus, reg);
                let result = match bit {
                    0 => self.cb_rlc(value),
                    1 => self.cb_rrc(value),
                    2 => self.cb_rl(value),
                    3 => self.cb_rr(value),
                    4 => self.cb_sla(value),
                    5 => self.cb_sra(value),
                    6 => self.cb_swap(value),
                    7 => self.cb_srl(value),
                    _ => unreachable!(),
                };
                self.write_r8(bus, reg, result);

                if is_hl { 16 } else { 8 }
            }

            // --- BIT: test bit ---
            0b01 => {
                let value = self.read_r8(bus, reg);
                self.regs.f.zero = (value >> bit) & 1 == 0;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = true;
                // Carry flag is unchanged.

                // BIT on (HL) reads memory but does not write back.
                if is_hl { 12 } else { 8 }
            }

            // --- RES: reset (clear) bit ---
            0b10 => {
                let value = self.read_r8(bus, reg);
                let result = value & !(1 << bit);
                self.write_r8(bus, reg, result);
                // No flag changes.

                if is_hl { 16 } else { 8 }
            }

            // --- SET: set bit ---
            0b11 => {
                let value = self.read_r8(bus, reg);
                let result = value | (1 << bit);
                self.write_r8(bus, reg, result);
                // No flag changes.

                if is_hl { 16 } else { 8 }
            }

            _ => unreachable!(),
        }
    }

    // --- Rotate / Shift helpers ---
    //
    // Each returns the result byte and sets flags accordingly:
    //   Z = result == 0
    //   N = 0
    //   H = 0
    //   C = the bit that was rotated/shifted out

    /// RLC: Rotate left. Old bit 7 goes to both carry and bit 0.
    fn cb_rlc(&mut self, value: u8) -> u8 {
        let carry_bit = value >> 7;
        let result = (value << 1) | carry_bit;

        self.regs.f.zero = result == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = carry_bit != 0;

        result
    }

    /// RRC: Rotate right. Old bit 0 goes to both carry and bit 7.
    fn cb_rrc(&mut self, value: u8) -> u8 {
        let carry_bit = value & 1;
        let result = (value >> 1) | (carry_bit << 7);

        self.regs.f.zero = result == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = carry_bit != 0;

        result
    }

    /// RL: Rotate left through carry. Old bit 7 goes to carry; old carry goes to bit 0.
    fn cb_rl(&mut self, value: u8) -> u8 {
        let old_carry = if self.regs.f.carry { 1u8 } else { 0 };
        let new_carry = value >> 7;
        let result = (value << 1) | old_carry;

        self.regs.f.zero = result == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = new_carry != 0;

        result
    }

    /// RR: Rotate right through carry. Old bit 0 goes to carry; old carry goes to bit 7.
    fn cb_rr(&mut self, value: u8) -> u8 {
        let old_carry = if self.regs.f.carry { 1u8 } else { 0 };
        let new_carry = value & 1;
        let result = (value >> 1) | (old_carry << 7);

        self.regs.f.zero = result == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = new_carry != 0;

        result
    }

    /// SLA: Shift left arithmetic. Bit 7 goes to carry; bit 0 becomes 0.
    fn cb_sla(&mut self, value: u8) -> u8 {
        let carry_bit = value >> 7;
        let result = value << 1;

        self.regs.f.zero = result == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = carry_bit != 0;

        result
    }

    /// SRA: Shift right arithmetic. Bit 0 goes to carry; bit 7 is preserved (sign extension).
    fn cb_sra(&mut self, value: u8) -> u8 {
        let carry_bit = value & 1;
        let result = (value >> 1) | (value & 0x80); // Preserve bit 7

        self.regs.f.zero = result == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = carry_bit != 0;

        result
    }

    /// SWAP: Swap upper and lower nibbles. Carry is always cleared.
    fn cb_swap(&mut self, value: u8) -> u8 {
        let result = (value >> 4) | (value << 4);

        self.regs.f.zero = result == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = false;

        result
    }

    /// SRL: Shift right logical. Bit 0 goes to carry; bit 7 becomes 0.
    fn cb_srl(&mut self, value: u8) -> u8 {
        let carry_bit = value & 1;
        let result = value >> 1;

        self.regs.f.zero = result == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = carry_bit != 0;

        result
    }
}
