pub mod registers;
mod opcodes;
mod cb_opcodes;

use registers::Registers;
use crate::bus::Bus;

/// CPU state.
#[derive(Clone, Copy, PartialEq)]
enum CpuState {
    Running,
    Halted,
}

pub struct Cpu {
    pub regs: Registers,
    state: CpuState,
    /// HALT bug: if HALT executed with IME=0 and (IE & IF) != 0,
    /// the next instruction's first byte is read twice (PC not incremented).
    halt_bug: bool,
    /// Debug: count consecutive halted steps to detect hangs.
    halt_cycles: u32,
    /// Debug: total steps executed.
    step_count: u64,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            regs: Registers::new(),
            state: CpuState::Running,
            halt_bug: false,
            halt_cycles: 0,
            step_count: 0,
        }
    }

    /// Execute one instruction and return the T-cycle cost.
    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        self.step_count += 1;

        // Handle EI delay: save and clear the pending flag so we can detect
        // if DI cancels it during the instruction.
        let was_ei_pending = bus.interrupts.ei_pending;
        if was_ei_pending {
            bus.interrupts.ei_pending = false;
        }

        // Check for pending interrupts
        let interrupt_cycles = self.handle_interrupts(bus);
        if interrupt_cycles > 0 {
            self.halt_cycles = 0;
            return interrupt_cycles;
        }

        // If halted, consume 4 T-cycles (1 M-cycle) doing nothing
        if self.state == CpuState::Halted {
            self.halt_cycles += 4;
            return 4;
        }
        self.halt_cycles = 0;

        // Fetch opcode
        let opcode = self.fetch_byte(bus);

        // HALT bug: PC was not incremented, so the byte was read but PC stays
        if self.halt_bug {
            self.halt_bug = false;
            self.regs.pc = self.regs.pc.wrapping_sub(1);
        }

        // Execute instruction
        let cycles = if opcode == 0xCB {
            let cb_opcode = self.fetch_byte(bus);
            self.execute_cb(bus, cb_opcode)
        } else {
            self.execute(bus, opcode)
        };

        // Apply EI delay: enable IME after the instruction following EI,
        // but only if DI didn't cancel it (DI clears ei_pending which we
        // already saved, but also re-clears it to signal cancellation).
        // If the executed instruction was DI, ei_pending stays false (DI
        // explicitly clears it). If it was anything else, ei_pending is
        // false because we cleared it above. We detect DI cancellation by
        // checking if IME was explicitly set to false during this step.
        if was_ei_pending && !bus.interrupts.ei_pending {
            // ei_pending is false either because we cleared it (normal) or
            // because DI cleared it (cancel). Distinguish by checking: DI
            // sets IME=false. If IME is already false and the instruction
            // was DI (opcode 0xF3), we should NOT re-enable.
            if opcode != 0xF3 {
                bus.interrupts.ime = true;
            }
        }

        cycles
    }

    fn handle_interrupts(&mut self, bus: &mut Bus) -> u32 {
        let pending = bus.interrupts.pending();

        if pending != 0 {
            // Any pending interrupt wakes CPU from HALT
            self.state = CpuState::Running;
        }

        // Try to dispatch interrupt
        if let Some(vector) = bus.interrupts.acknowledge() {
            // Clear halt_bug if set — interrupt dispatch takes priority
            // (e.g. EI + HALT with pending interrupt: halt_bug was set but
            // the interrupt dispatches before the next fetch can consume it)
            self.halt_bug = false;

            // Push PC onto stack (2 M-cycles)
            self.regs.sp = self.regs.sp.wrapping_sub(1);
            bus.write_byte(self.regs.sp, (self.regs.pc >> 8) as u8);
            self.regs.sp = self.regs.sp.wrapping_sub(1);
            bus.write_byte(self.regs.sp, self.regs.pc as u8);

            // Jump to interrupt vector
            self.regs.pc = vector;

            // Interrupt dispatch takes 20 T-cycles (5 M-cycles)
            return 20;
        }

        0
    }

    fn fetch_byte(&mut self, bus: &Bus) -> u8 {
        let byte = bus.read_byte(self.regs.pc);
        self.regs.pc = self.regs.pc.wrapping_add(1);
        byte
    }

    fn fetch_word(&mut self, bus: &Bus) -> u16 {
        let lo = self.fetch_byte(bus) as u16;
        let hi = self.fetch_byte(bus) as u16;
        hi << 8 | lo
    }

    // --- Stack operations ---

    fn push_word(&mut self, bus: &mut Bus, value: u16) {
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        bus.write_byte(self.regs.sp, (value >> 8) as u8);
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        bus.write_byte(self.regs.sp, value as u8);
    }

    fn pop_word(&mut self, bus: &Bus) -> u16 {
        let lo = bus.read_byte(self.regs.sp) as u16;
        self.regs.sp = self.regs.sp.wrapping_add(1);
        let hi = bus.read_byte(self.regs.sp) as u16;
        self.regs.sp = self.regs.sp.wrapping_add(1);
        hi << 8 | lo
    }

    // --- ALU helpers ---

    fn alu_add(&mut self, value: u8, carry: bool) {
        let c = if carry && self.regs.f.carry { 1u8 } else { 0 };
        let a = self.regs.a;
        let result = a as u16 + value as u16 + c as u16;

        self.regs.f.zero = (result as u8) == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = (a & 0x0F) + (value & 0x0F) + c > 0x0F;
        self.regs.f.carry = result > 0xFF;
        self.regs.a = result as u8;
    }

    fn alu_sub(&mut self, value: u8, carry: bool) {
        let c = if carry && self.regs.f.carry { 1u8 } else { 0 };
        let a = self.regs.a;
        let result = (a as i16) - (value as i16) - (c as i16);

        self.regs.f.zero = (result as u8) == 0;
        self.regs.f.subtract = true;
        self.regs.f.half_carry = (a & 0x0F) < (value & 0x0F) + c;
        self.regs.f.carry = result < 0;
        self.regs.a = result as u8;
    }

    fn alu_and(&mut self, value: u8) {
        self.regs.a &= value;
        self.regs.f.zero = self.regs.a == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = true;
        self.regs.f.carry = false;
    }

    fn alu_xor(&mut self, value: u8) {
        self.regs.a ^= value;
        self.regs.f.zero = self.regs.a == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = false;
    }

    fn alu_or(&mut self, value: u8) {
        self.regs.a |= value;
        self.regs.f.zero = self.regs.a == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = false;
    }

    fn alu_cp(&mut self, value: u8) {
        let a = self.regs.a;
        self.alu_sub(value, false);
        self.regs.a = a; // CP doesn't store the result
    }

    fn alu_inc(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        self.regs.f.zero = result == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = (value & 0x0F) + 1 > 0x0F;
        // Carry not affected
        result
    }

    fn alu_dec(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        self.regs.f.zero = result == 0;
        self.regs.f.subtract = true;
        self.regs.f.half_carry = (value & 0x0F) == 0;
        // Carry not affected
        result
    }

    fn alu_add_hl(&mut self, value: u16) {
        let hl = self.regs.hl();
        let result = hl as u32 + value as u32;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = (hl & 0x0FFF) + (value & 0x0FFF) > 0x0FFF;
        self.regs.f.carry = result > 0xFFFF;
        self.regs.set_hl(result as u16);
    }

    fn alu_add_sp_signed(&mut self, bus: &Bus) -> u16 {
        let offset = self.fetch_byte(bus) as i8 as i16 as u16;
        let sp = self.regs.sp;
        let result = sp.wrapping_add(offset);

        self.regs.f.zero = false;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = (sp & 0x000F) + (offset & 0x000F) > 0x000F;
        self.regs.f.carry = (sp & 0x00FF) + (offset & 0x00FF) > 0x00FF;

        result
    }

    // --- Rotate/shift helpers (for non-CB instructions) ---

    fn rlca(&mut self) {
        let carry = self.regs.a >> 7;
        self.regs.a = (self.regs.a << 1) | carry;
        self.regs.f.zero = false;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = carry != 0;
    }

    fn rrca(&mut self) {
        let carry = self.regs.a & 1;
        self.regs.a = (self.regs.a >> 1) | (carry << 7);
        self.regs.f.zero = false;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = carry != 0;
    }

    fn rla(&mut self) {
        let old_carry = if self.regs.f.carry { 1u8 } else { 0 };
        let new_carry = self.regs.a >> 7;
        self.regs.a = (self.regs.a << 1) | old_carry;
        self.regs.f.zero = false;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = new_carry != 0;
    }

    fn rra(&mut self) {
        let old_carry = if self.regs.f.carry { 1u8 } else { 0 };
        let new_carry = self.regs.a & 1;
        self.regs.a = (self.regs.a >> 1) | (old_carry << 7);
        self.regs.f.zero = false;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = new_carry != 0;
    }

    fn daa(&mut self) {
        let mut adjust = 0u8;
        let mut carry = false;

        if self.regs.f.subtract {
            if self.regs.f.carry {
                adjust |= 0x60;
                carry = true;
            }
            if self.regs.f.half_carry {
                adjust |= 0x06;
            }
            self.regs.a = self.regs.a.wrapping_sub(adjust);
        } else {
            if self.regs.f.carry || self.regs.a > 0x99 {
                adjust |= 0x60;
                carry = true;
            }
            if self.regs.f.half_carry || (self.regs.a & 0x0F) > 0x09 {
                adjust |= 0x06;
            }
            self.regs.a = self.regs.a.wrapping_add(adjust);
        }

        self.regs.f.zero = self.regs.a == 0;
        self.regs.f.half_carry = false;
        self.regs.f.carry = carry;
    }
}
