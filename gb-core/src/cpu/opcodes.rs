use crate::bus::Bus;
use super::Cpu;
use super::CpuState;

impl Cpu {
    /// Execute a base (non-CB-prefixed) opcode. Returns T-cycle cost.
    pub(super) fn execute(&mut self, bus: &mut Bus, opcode: u8) -> u32 {
        match opcode {
            // ===================================================================
            // 0x00 - 0x0F
            // ===================================================================

            // NOP
            0x00 => 4,

            // LD BC,d16
            0x01 => {
                let v = self.fetch_word(bus);
                self.regs.set_bc(v);
                12
            }

            // LD (BC),A
            0x02 => {
                bus.write_byte(self.regs.bc(), self.regs.a);
                8
            }

            // INC BC
            0x03 => {
                let v = self.regs.bc().wrapping_add(1);
                self.regs.set_bc(v);
                8
            }

            // INC B
            0x04 => {
                self.regs.b = self.alu_inc(self.regs.b);
                4
            }

            // DEC B
            0x05 => {
                self.regs.b = self.alu_dec(self.regs.b);
                4
            }

            // LD B,d8
            0x06 => {
                self.regs.b = self.fetch_byte(bus);
                8
            }

            // RLCA
            0x07 => {
                self.rlca();
                4
            }

            // LD (a16),SP
            0x08 => {
                let addr = self.fetch_word(bus);
                bus.write_byte(addr, self.regs.sp as u8);
                bus.write_byte(addr.wrapping_add(1), (self.regs.sp >> 8) as u8);
                20
            }

            // ADD HL,BC
            0x09 => {
                let v = self.regs.bc();
                self.alu_add_hl(v);
                8
            }

            // LD A,(BC)
            0x0A => {
                self.regs.a = bus.read_byte(self.regs.bc());
                8
            }

            // DEC BC
            0x0B => {
                let v = self.regs.bc().wrapping_sub(1);
                self.regs.set_bc(v);
                8
            }

            // INC C
            0x0C => {
                self.regs.c = self.alu_inc(self.regs.c);
                4
            }

            // DEC C
            0x0D => {
                self.regs.c = self.alu_dec(self.regs.c);
                4
            }

            // LD C,d8
            0x0E => {
                self.regs.c = self.fetch_byte(bus);
                8
            }

            // RRCA
            0x0F => {
                self.rrca();
                4
            }

            // ===================================================================
            // 0x10 - 0x1F
            // ===================================================================

            // STOP
            0x10 => {
                // Consume the next byte (STOP is a 2-byte instruction)
                let _ = self.fetch_byte(bus);
                4
            }

            // LD DE,d16
            0x11 => {
                let v = self.fetch_word(bus);
                self.regs.set_de(v);
                12
            }

            // LD (DE),A
            0x12 => {
                bus.write_byte(self.regs.de(), self.regs.a);
                8
            }

            // INC DE
            0x13 => {
                let v = self.regs.de().wrapping_add(1);
                self.regs.set_de(v);
                8
            }

            // INC D
            0x14 => {
                self.regs.d = self.alu_inc(self.regs.d);
                4
            }

            // DEC D
            0x15 => {
                self.regs.d = self.alu_dec(self.regs.d);
                4
            }

            // LD D,d8
            0x16 => {
                self.regs.d = self.fetch_byte(bus);
                8
            }

            // RLA
            0x17 => {
                self.rla();
                4
            }

            // JR r8
            0x18 => {
                let offset = self.fetch_byte(bus) as i8;
                self.regs.pc = self.regs.pc.wrapping_add(offset as u16);
                12
            }

            // ADD HL,DE
            0x19 => {
                let v = self.regs.de();
                self.alu_add_hl(v);
                8
            }

            // LD A,(DE)
            0x1A => {
                self.regs.a = bus.read_byte(self.regs.de());
                8
            }

            // DEC DE
            0x1B => {
                let v = self.regs.de().wrapping_sub(1);
                self.regs.set_de(v);
                8
            }

            // INC E
            0x1C => {
                self.regs.e = self.alu_inc(self.regs.e);
                4
            }

            // DEC E
            0x1D => {
                self.regs.e = self.alu_dec(self.regs.e);
                4
            }

            // LD E,d8
            0x1E => {
                self.regs.e = self.fetch_byte(bus);
                8
            }

            // RRA
            0x1F => {
                self.rra();
                4
            }

            // ===================================================================
            // 0x20 - 0x2F
            // ===================================================================

            // JR NZ,r8
            0x20 => {
                let offset = self.fetch_byte(bus) as i8;
                if !self.regs.f.zero {
                    self.regs.pc = self.regs.pc.wrapping_add(offset as u16);
                    12
                } else {
                    8
                }
            }

            // LD HL,d16
            0x21 => {
                let v = self.fetch_word(bus);
                self.regs.set_hl(v);
                12
            }

            // LD (HL+),A
            0x22 => {
                let hl = self.regs.hl();
                bus.write_byte(hl, self.regs.a);
                self.regs.set_hl(hl.wrapping_add(1));
                8
            }

            // INC HL
            0x23 => {
                let v = self.regs.hl().wrapping_add(1);
                self.regs.set_hl(v);
                8
            }

            // INC H
            0x24 => {
                self.regs.h = self.alu_inc(self.regs.h);
                4
            }

            // DEC H
            0x25 => {
                self.regs.h = self.alu_dec(self.regs.h);
                4
            }

            // LD H,d8
            0x26 => {
                self.regs.h = self.fetch_byte(bus);
                8
            }

            // DAA
            0x27 => {
                self.daa();
                4
            }

            // JR Z,r8
            0x28 => {
                let offset = self.fetch_byte(bus) as i8;
                if self.regs.f.zero {
                    self.regs.pc = self.regs.pc.wrapping_add(offset as u16);
                    12
                } else {
                    8
                }
            }

            // ADD HL,HL
            0x29 => {
                let v = self.regs.hl();
                self.alu_add_hl(v);
                8
            }

            // LD A,(HL+)
            0x2A => {
                let hl = self.regs.hl();
                self.regs.a = bus.read_byte(hl);
                self.regs.set_hl(hl.wrapping_add(1));
                8
            }

            // DEC HL
            0x2B => {
                let v = self.regs.hl().wrapping_sub(1);
                self.regs.set_hl(v);
                8
            }

            // INC L
            0x2C => {
                self.regs.l = self.alu_inc(self.regs.l);
                4
            }

            // DEC L
            0x2D => {
                self.regs.l = self.alu_dec(self.regs.l);
                4
            }

            // LD L,d8
            0x2E => {
                self.regs.l = self.fetch_byte(bus);
                8
            }

            // CPL
            0x2F => {
                self.regs.a = !self.regs.a;
                self.regs.f.subtract = true;
                self.regs.f.half_carry = true;
                4
            }

            // ===================================================================
            // 0x30 - 0x3F
            // ===================================================================

            // JR NC,r8
            0x30 => {
                let offset = self.fetch_byte(bus) as i8;
                if !self.regs.f.carry {
                    self.regs.pc = self.regs.pc.wrapping_add(offset as u16);
                    12
                } else {
                    8
                }
            }

            // LD SP,d16
            0x31 => {
                self.regs.sp = self.fetch_word(bus);
                12
            }

            // LD (HL-),A
            0x32 => {
                let hl = self.regs.hl();
                bus.write_byte(hl, self.regs.a);
                self.regs.set_hl(hl.wrapping_sub(1));
                8
            }

            // INC SP
            0x33 => {
                self.regs.sp = self.regs.sp.wrapping_add(1);
                8
            }

            // INC (HL)
            0x34 => {
                let hl = self.regs.hl();
                let v = bus.read_byte(hl);
                let result = self.alu_inc(v);
                bus.write_byte(hl, result);
                12
            }

            // DEC (HL)
            0x35 => {
                let hl = self.regs.hl();
                let v = bus.read_byte(hl);
                let result = self.alu_dec(v);
                bus.write_byte(hl, result);
                12
            }

            // LD (HL),d8
            0x36 => {
                let v = self.fetch_byte(bus);
                bus.write_byte(self.regs.hl(), v);
                12
            }

            // SCF
            0x37 => {
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;
                self.regs.f.carry = true;
                4
            }

            // JR C,r8
            0x38 => {
                let offset = self.fetch_byte(bus) as i8;
                if self.regs.f.carry {
                    self.regs.pc = self.regs.pc.wrapping_add(offset as u16);
                    12
                } else {
                    8
                }
            }

            // ADD HL,SP
            0x39 => {
                let v = self.regs.sp;
                self.alu_add_hl(v);
                8
            }

            // LD A,(HL-)
            0x3A => {
                let hl = self.regs.hl();
                self.regs.a = bus.read_byte(hl);
                self.regs.set_hl(hl.wrapping_sub(1));
                8
            }

            // DEC SP
            0x3B => {
                self.regs.sp = self.regs.sp.wrapping_sub(1);
                8
            }

            // INC A
            0x3C => {
                self.regs.a = self.alu_inc(self.regs.a);
                4
            }

            // DEC A
            0x3D => {
                self.regs.a = self.alu_dec(self.regs.a);
                4
            }

            // LD A,d8
            0x3E => {
                self.regs.a = self.fetch_byte(bus);
                8
            }

            // CCF
            0x3F => {
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;
                self.regs.f.carry = !self.regs.f.carry;
                4
            }

            // ===================================================================
            // 0x40 - 0x4F: LD B/C, r
            // ===================================================================

            // LD B,B
            0x40 => 4,

            // LD B,C
            0x41 => {
                self.regs.b = self.regs.c;
                4
            }

            // LD B,D
            0x42 => {
                self.regs.b = self.regs.d;
                4
            }

            // LD B,E
            0x43 => {
                self.regs.b = self.regs.e;
                4
            }

            // LD B,H
            0x44 => {
                self.regs.b = self.regs.h;
                4
            }

            // LD B,L
            0x45 => {
                self.regs.b = self.regs.l;
                4
            }

            // LD B,(HL)
            0x46 => {
                self.regs.b = bus.read_byte(self.regs.hl());
                8
            }

            // LD B,A
            0x47 => {
                self.regs.b = self.regs.a;
                4
            }

            // LD C,B
            0x48 => {
                self.regs.c = self.regs.b;
                4
            }

            // LD C,C
            0x49 => 4,

            // LD C,D
            0x4A => {
                self.regs.c = self.regs.d;
                4
            }

            // LD C,E
            0x4B => {
                self.regs.c = self.regs.e;
                4
            }

            // LD C,H
            0x4C => {
                self.regs.c = self.regs.h;
                4
            }

            // LD C,L
            0x4D => {
                self.regs.c = self.regs.l;
                4
            }

            // LD C,(HL)
            0x4E => {
                self.regs.c = bus.read_byte(self.regs.hl());
                8
            }

            // LD C,A
            0x4F => {
                self.regs.c = self.regs.a;
                4
            }

            // ===================================================================
            // 0x50 - 0x5F: LD D/E, r
            // ===================================================================

            // LD D,B
            0x50 => {
                self.regs.d = self.regs.b;
                4
            }

            // LD D,C
            0x51 => {
                self.regs.d = self.regs.c;
                4
            }

            // LD D,D
            0x52 => 4,

            // LD D,E
            0x53 => {
                self.regs.d = self.regs.e;
                4
            }

            // LD D,H
            0x54 => {
                self.regs.d = self.regs.h;
                4
            }

            // LD D,L
            0x55 => {
                self.regs.d = self.regs.l;
                4
            }

            // LD D,(HL)
            0x56 => {
                self.regs.d = bus.read_byte(self.regs.hl());
                8
            }

            // LD D,A
            0x57 => {
                self.regs.d = self.regs.a;
                4
            }

            // LD E,B
            0x58 => {
                self.regs.e = self.regs.b;
                4
            }

            // LD E,C
            0x59 => {
                self.regs.e = self.regs.c;
                4
            }

            // LD E,D
            0x5A => {
                self.regs.e = self.regs.d;
                4
            }

            // LD E,E
            0x5B => 4,

            // LD E,H
            0x5C => {
                self.regs.e = self.regs.h;
                4
            }

            // LD E,L
            0x5D => {
                self.regs.e = self.regs.l;
                4
            }

            // LD E,(HL)
            0x5E => {
                self.regs.e = bus.read_byte(self.regs.hl());
                8
            }

            // LD E,A
            0x5F => {
                self.regs.e = self.regs.a;
                4
            }

            // ===================================================================
            // 0x60 - 0x6F: LD H/L, r
            // ===================================================================

            // LD H,B
            0x60 => {
                self.regs.h = self.regs.b;
                4
            }

            // LD H,C
            0x61 => {
                self.regs.h = self.regs.c;
                4
            }

            // LD H,D
            0x62 => {
                self.regs.h = self.regs.d;
                4
            }

            // LD H,E
            0x63 => {
                self.regs.h = self.regs.e;
                4
            }

            // LD H,H
            0x64 => 4,

            // LD H,L
            0x65 => {
                self.regs.h = self.regs.l;
                4
            }

            // LD H,(HL)
            0x66 => {
                self.regs.h = bus.read_byte(self.regs.hl());
                8
            }

            // LD H,A
            0x67 => {
                self.regs.h = self.regs.a;
                4
            }

            // LD L,B
            0x68 => {
                self.regs.l = self.regs.b;
                4
            }

            // LD L,C
            0x69 => {
                self.regs.l = self.regs.c;
                4
            }

            // LD L,D
            0x6A => {
                self.regs.l = self.regs.d;
                4
            }

            // LD L,E
            0x6B => {
                self.regs.l = self.regs.e;
                4
            }

            // LD L,H
            0x6C => {
                self.regs.l = self.regs.h;
                4
            }

            // LD L,L
            0x6D => 4,

            // LD L,(HL)
            0x6E => {
                self.regs.l = bus.read_byte(self.regs.hl());
                8
            }

            // LD L,A
            0x6F => {
                self.regs.l = self.regs.a;
                4
            }

            // ===================================================================
            // 0x70 - 0x7F: LD (HL)/A, r
            // ===================================================================

            // LD (HL),B
            0x70 => {
                bus.write_byte(self.regs.hl(), self.regs.b);
                8
            }

            // LD (HL),C
            0x71 => {
                bus.write_byte(self.regs.hl(), self.regs.c);
                8
            }

            // LD (HL),D
            0x72 => {
                bus.write_byte(self.regs.hl(), self.regs.d);
                8
            }

            // LD (HL),E
            0x73 => {
                bus.write_byte(self.regs.hl(), self.regs.e);
                8
            }

            // LD (HL),H
            0x74 => {
                bus.write_byte(self.regs.hl(), self.regs.h);
                8
            }

            // LD (HL),L
            0x75 => {
                bus.write_byte(self.regs.hl(), self.regs.l);
                8
            }

            // HALT
            0x76 => {
                // Check for HALT bug: IME=0 and (IE & IF) != 0
                if !bus.interrupts.ime && bus.interrupts.pending() != 0 {
                    self.halt_bug = true;
                } else {
                    self.state = CpuState::Halted;
                }
                4
            }

            // LD (HL),A
            0x77 => {
                bus.write_byte(self.regs.hl(), self.regs.a);
                8
            }

            // LD A,B
            0x78 => {
                self.regs.a = self.regs.b;
                4
            }

            // LD A,C
            0x79 => {
                self.regs.a = self.regs.c;
                4
            }

            // LD A,D
            0x7A => {
                self.regs.a = self.regs.d;
                4
            }

            // LD A,E
            0x7B => {
                self.regs.a = self.regs.e;
                4
            }

            // LD A,H
            0x7C => {
                self.regs.a = self.regs.h;
                4
            }

            // LD A,L
            0x7D => {
                self.regs.a = self.regs.l;
                4
            }

            // LD A,(HL)
            0x7E => {
                self.regs.a = bus.read_byte(self.regs.hl());
                8
            }

            // LD A,A
            0x7F => 4,

            // ===================================================================
            // 0x80 - 0x8F: ADD A,r / ADC A,r
            // ===================================================================

            // ADD A,B
            0x80 => {
                self.alu_add(self.regs.b, false);
                4
            }

            // ADD A,C
            0x81 => {
                self.alu_add(self.regs.c, false);
                4
            }

            // ADD A,D
            0x82 => {
                self.alu_add(self.regs.d, false);
                4
            }

            // ADD A,E
            0x83 => {
                self.alu_add(self.regs.e, false);
                4
            }

            // ADD A,H
            0x84 => {
                self.alu_add(self.regs.h, false);
                4
            }

            // ADD A,L
            0x85 => {
                self.alu_add(self.regs.l, false);
                4
            }

            // ADD A,(HL)
            0x86 => {
                let v = bus.read_byte(self.regs.hl());
                self.alu_add(v, false);
                8
            }

            // ADD A,A
            0x87 => {
                self.alu_add(self.regs.a, false);
                4
            }

            // ADC A,B
            0x88 => {
                self.alu_add(self.regs.b, true);
                4
            }

            // ADC A,C
            0x89 => {
                self.alu_add(self.regs.c, true);
                4
            }

            // ADC A,D
            0x8A => {
                self.alu_add(self.regs.d, true);
                4
            }

            // ADC A,E
            0x8B => {
                self.alu_add(self.regs.e, true);
                4
            }

            // ADC A,H
            0x8C => {
                self.alu_add(self.regs.h, true);
                4
            }

            // ADC A,L
            0x8D => {
                self.alu_add(self.regs.l, true);
                4
            }

            // ADC A,(HL)
            0x8E => {
                let v = bus.read_byte(self.regs.hl());
                self.alu_add(v, true);
                8
            }

            // ADC A,A
            0x8F => {
                self.alu_add(self.regs.a, true);
                4
            }

            // ===================================================================
            // 0x90 - 0x9F: SUB r / SBC A,r
            // ===================================================================

            // SUB B
            0x90 => {
                self.alu_sub(self.regs.b, false);
                4
            }

            // SUB C
            0x91 => {
                self.alu_sub(self.regs.c, false);
                4
            }

            // SUB D
            0x92 => {
                self.alu_sub(self.regs.d, false);
                4
            }

            // SUB E
            0x93 => {
                self.alu_sub(self.regs.e, false);
                4
            }

            // SUB H
            0x94 => {
                self.alu_sub(self.regs.h, false);
                4
            }

            // SUB L
            0x95 => {
                self.alu_sub(self.regs.l, false);
                4
            }

            // SUB (HL)
            0x96 => {
                let v = bus.read_byte(self.regs.hl());
                self.alu_sub(v, false);
                8
            }

            // SUB A
            0x97 => {
                self.alu_sub(self.regs.a, false);
                4
            }

            // SBC A,B
            0x98 => {
                self.alu_sub(self.regs.b, true);
                4
            }

            // SBC A,C
            0x99 => {
                self.alu_sub(self.regs.c, true);
                4
            }

            // SBC A,D
            0x9A => {
                self.alu_sub(self.regs.d, true);
                4
            }

            // SBC A,E
            0x9B => {
                self.alu_sub(self.regs.e, true);
                4
            }

            // SBC A,H
            0x9C => {
                self.alu_sub(self.regs.h, true);
                4
            }

            // SBC A,L
            0x9D => {
                self.alu_sub(self.regs.l, true);
                4
            }

            // SBC A,(HL)
            0x9E => {
                let v = bus.read_byte(self.regs.hl());
                self.alu_sub(v, true);
                8
            }

            // SBC A,A
            0x9F => {
                self.alu_sub(self.regs.a, true);
                4
            }

            // ===================================================================
            // 0xA0 - 0xAF: AND r / XOR r
            // ===================================================================

            // AND B
            0xA0 => {
                self.alu_and(self.regs.b);
                4
            }

            // AND C
            0xA1 => {
                self.alu_and(self.regs.c);
                4
            }

            // AND D
            0xA2 => {
                self.alu_and(self.regs.d);
                4
            }

            // AND E
            0xA3 => {
                self.alu_and(self.regs.e);
                4
            }

            // AND H
            0xA4 => {
                self.alu_and(self.regs.h);
                4
            }

            // AND L
            0xA5 => {
                self.alu_and(self.regs.l);
                4
            }

            // AND (HL)
            0xA6 => {
                let v = bus.read_byte(self.regs.hl());
                self.alu_and(v);
                8
            }

            // AND A
            0xA7 => {
                self.alu_and(self.regs.a);
                4
            }

            // XOR B
            0xA8 => {
                self.alu_xor(self.regs.b);
                4
            }

            // XOR C
            0xA9 => {
                self.alu_xor(self.regs.c);
                4
            }

            // XOR D
            0xAA => {
                self.alu_xor(self.regs.d);
                4
            }

            // XOR E
            0xAB => {
                self.alu_xor(self.regs.e);
                4
            }

            // XOR H
            0xAC => {
                self.alu_xor(self.regs.h);
                4
            }

            // XOR L
            0xAD => {
                self.alu_xor(self.regs.l);
                4
            }

            // XOR (HL)
            0xAE => {
                let v = bus.read_byte(self.regs.hl());
                self.alu_xor(v);
                8
            }

            // XOR A
            0xAF => {
                self.alu_xor(self.regs.a);
                4
            }

            // ===================================================================
            // 0xB0 - 0xBF: OR r / CP r
            // ===================================================================

            // OR B
            0xB0 => {
                self.alu_or(self.regs.b);
                4
            }

            // OR C
            0xB1 => {
                self.alu_or(self.regs.c);
                4
            }

            // OR D
            0xB2 => {
                self.alu_or(self.regs.d);
                4
            }

            // OR E
            0xB3 => {
                self.alu_or(self.regs.e);
                4
            }

            // OR H
            0xB4 => {
                self.alu_or(self.regs.h);
                4
            }

            // OR L
            0xB5 => {
                self.alu_or(self.regs.l);
                4
            }

            // OR (HL)
            0xB6 => {
                let v = bus.read_byte(self.regs.hl());
                self.alu_or(v);
                8
            }

            // OR A
            0xB7 => {
                self.alu_or(self.regs.a);
                4
            }

            // CP B
            0xB8 => {
                self.alu_cp(self.regs.b);
                4
            }

            // CP C
            0xB9 => {
                self.alu_cp(self.regs.c);
                4
            }

            // CP D
            0xBA => {
                self.alu_cp(self.regs.d);
                4
            }

            // CP E
            0xBB => {
                self.alu_cp(self.regs.e);
                4
            }

            // CP H
            0xBC => {
                self.alu_cp(self.regs.h);
                4
            }

            // CP L
            0xBD => {
                self.alu_cp(self.regs.l);
                4
            }

            // CP (HL)
            0xBE => {
                let v = bus.read_byte(self.regs.hl());
                self.alu_cp(v);
                8
            }

            // CP A
            0xBF => {
                self.alu_cp(self.regs.a);
                4
            }

            // ===================================================================
            // 0xC0 - 0xCF
            // ===================================================================

            // RET NZ
            0xC0 => {
                if !self.regs.f.zero {
                    self.regs.pc = self.pop_word(bus);
                    20
                } else {
                    8
                }
            }

            // POP BC
            0xC1 => {
                let v = self.pop_word(bus);
                self.regs.set_bc(v);
                12
            }

            // JP NZ,a16
            0xC2 => {
                let addr = self.fetch_word(bus);
                if !self.regs.f.zero {
                    self.regs.pc = addr;
                    16
                } else {
                    12
                }
            }

            // JP a16
            0xC3 => {
                self.regs.pc = self.fetch_word(bus);
                16
            }

            // CALL NZ,a16
            0xC4 => {
                let addr = self.fetch_word(bus);
                if !self.regs.f.zero {
                    self.push_word(bus, self.regs.pc);
                    self.regs.pc = addr;
                    24
                } else {
                    12
                }
            }

            // PUSH BC
            0xC5 => {
                let v = self.regs.bc();
                self.push_word(bus, v);
                16
            }

            // ADD A,d8
            0xC6 => {
                let v = self.fetch_byte(bus);
                self.alu_add(v, false);
                8
            }

            // RST 00H
            0xC7 => {
                self.push_word(bus, self.regs.pc);
                self.regs.pc = 0x0000;
                16
            }

            // RET Z
            0xC8 => {
                if self.regs.f.zero {
                    self.regs.pc = self.pop_word(bus);
                    20
                } else {
                    8
                }
            }

            // RET
            0xC9 => {
                self.regs.pc = self.pop_word(bus);
                16
            }

            // JP Z,a16
            0xCA => {
                let addr = self.fetch_word(bus);
                if self.regs.f.zero {
                    self.regs.pc = addr;
                    16
                } else {
                    12
                }
            }

            // PREFIX CB - handled in step(), should not reach here
            0xCB => unreachable!("CB prefix should be handled in step()"),

            // CALL Z,a16
            0xCC => {
                let addr = self.fetch_word(bus);
                if self.regs.f.zero {
                    self.push_word(bus, self.regs.pc);
                    self.regs.pc = addr;
                    24
                } else {
                    12
                }
            }

            // CALL a16
            0xCD => {
                let addr = self.fetch_word(bus);
                self.push_word(bus, self.regs.pc);
                self.regs.pc = addr;
                24
            }

            // ADC A,d8
            0xCE => {
                let v = self.fetch_byte(bus);
                self.alu_add(v, true);
                8
            }

            // RST 08H
            0xCF => {
                self.push_word(bus, self.regs.pc);
                self.regs.pc = 0x0008;
                16
            }

            // ===================================================================
            // 0xD0 - 0xDF
            // ===================================================================

            // RET NC
            0xD0 => {
                if !self.regs.f.carry {
                    self.regs.pc = self.pop_word(bus);
                    20
                } else {
                    8
                }
            }

            // POP DE
            0xD1 => {
                let v = self.pop_word(bus);
                self.regs.set_de(v);
                12
            }

            // JP NC,a16
            0xD2 => {
                let addr = self.fetch_word(bus);
                if !self.regs.f.carry {
                    self.regs.pc = addr;
                    16
                } else {
                    12
                }
            }

            // [D3] - Undefined
            0xD3 => panic!("Undefined opcode: 0xD3"),

            // CALL NC,a16
            0xD4 => {
                let addr = self.fetch_word(bus);
                if !self.regs.f.carry {
                    self.push_word(bus, self.regs.pc);
                    self.regs.pc = addr;
                    24
                } else {
                    12
                }
            }

            // PUSH DE
            0xD5 => {
                let v = self.regs.de();
                self.push_word(bus, v);
                16
            }

            // SUB d8
            0xD6 => {
                let v = self.fetch_byte(bus);
                self.alu_sub(v, false);
                8
            }

            // RST 10H
            0xD7 => {
                self.push_word(bus, self.regs.pc);
                self.regs.pc = 0x0010;
                16
            }

            // RET C
            0xD8 => {
                if self.regs.f.carry {
                    self.regs.pc = self.pop_word(bus);
                    20
                } else {
                    8
                }
            }

            // RETI
            0xD9 => {
                self.regs.pc = self.pop_word(bus);
                bus.interrupts.ime = true;
                16
            }

            // JP C,a16
            0xDA => {
                let addr = self.fetch_word(bus);
                if self.regs.f.carry {
                    self.regs.pc = addr;
                    16
                } else {
                    12
                }
            }

            // [DB] - Undefined
            0xDB => panic!("Undefined opcode: 0xDB"),

            // CALL C,a16
            0xDC => {
                let addr = self.fetch_word(bus);
                if self.regs.f.carry {
                    self.push_word(bus, self.regs.pc);
                    self.regs.pc = addr;
                    24
                } else {
                    12
                }
            }

            // [DD] - Undefined
            0xDD => panic!("Undefined opcode: 0xDD"),

            // SBC A,d8
            0xDE => {
                let v = self.fetch_byte(bus);
                self.alu_sub(v, true);
                8
            }

            // RST 18H
            0xDF => {
                self.push_word(bus, self.regs.pc);
                self.regs.pc = 0x0018;
                16
            }

            // ===================================================================
            // 0xE0 - 0xEF
            // ===================================================================

            // LDH (a8),A  -  LD (0xFF00+a8),A
            0xE0 => {
                let offset = self.fetch_byte(bus) as u16;
                bus.write_byte(0xFF00 | offset, self.regs.a);
                12
            }

            // POP HL
            0xE1 => {
                let v = self.pop_word(bus);
                self.regs.set_hl(v);
                12
            }

            // LD (C),A  -  LD (0xFF00+C),A
            0xE2 => {
                bus.write_byte(0xFF00 | self.regs.c as u16, self.regs.a);
                8
            }

            // [E3] - Undefined
            0xE3 => panic!("Undefined opcode: 0xE3"),

            // [E4] - Undefined
            0xE4 => panic!("Undefined opcode: 0xE4"),

            // PUSH HL
            0xE5 => {
                let v = self.regs.hl();
                self.push_word(bus, v);
                16
            }

            // AND d8
            0xE6 => {
                let v = self.fetch_byte(bus);
                self.alu_and(v);
                8
            }

            // RST 20H
            0xE7 => {
                self.push_word(bus, self.regs.pc);
                self.regs.pc = 0x0020;
                16
            }

            // ADD SP,r8
            0xE8 => {
                let result = self.alu_add_sp_signed(bus);
                self.regs.sp = result;
                16
            }

            // JP (HL)  -  actually JP HL (no dereference)
            0xE9 => {
                self.regs.pc = self.regs.hl();
                4
            }

            // LD (a16),A
            0xEA => {
                let addr = self.fetch_word(bus);
                bus.write_byte(addr, self.regs.a);
                16
            }

            // [EB] - Undefined
            0xEB => panic!("Undefined opcode: 0xEB"),

            // [EC] - Undefined
            0xEC => panic!("Undefined opcode: 0xEC"),

            // [ED] - Undefined
            0xED => panic!("Undefined opcode: 0xED"),

            // XOR d8
            0xEE => {
                let v = self.fetch_byte(bus);
                self.alu_xor(v);
                8
            }

            // RST 28H
            0xEF => {
                self.push_word(bus, self.regs.pc);
                self.regs.pc = 0x0028;
                16
            }

            // ===================================================================
            // 0xF0 - 0xFF
            // ===================================================================

            // LDH A,(a8)  -  LD A,(0xFF00+a8)
            0xF0 => {
                let offset = self.fetch_byte(bus) as u16;
                self.regs.a = bus.read_byte(0xFF00 | offset);
                12
            }

            // POP AF
            0xF1 => {
                let v = self.pop_word(bus);
                self.regs.set_af(v); // set_af masks lower 4 bits of F
                12
            }

            // LD A,(C)  -  LD A,(0xFF00+C)
            0xF2 => {
                self.regs.a = bus.read_byte(0xFF00 | self.regs.c as u16);
                8
            }

            // DI
            0xF3 => {
                bus.interrupts.ime = false;
                4
            }

            // [F4] - Undefined
            0xF4 => panic!("Undefined opcode: 0xF4"),

            // PUSH AF
            0xF5 => {
                let v = self.regs.af();
                self.push_word(bus, v);
                16
            }

            // OR d8
            0xF6 => {
                let v = self.fetch_byte(bus);
                self.alu_or(v);
                8
            }

            // RST 30H
            0xF7 => {
                self.push_word(bus, self.regs.pc);
                self.regs.pc = 0x0030;
                16
            }

            // LD HL,SP+r8
            0xF8 => {
                let result = self.alu_add_sp_signed(bus);
                self.regs.set_hl(result);
                12
            }

            // LD SP,HL
            0xF9 => {
                self.regs.sp = self.regs.hl();
                8
            }

            // LD A,(a16)
            0xFA => {
                let addr = self.fetch_word(bus);
                self.regs.a = bus.read_byte(addr);
                16
            }

            // EI
            0xFB => {
                bus.interrupts.ei_pending = true;
                4
            }

            // [FC] - Undefined (not in the standard list, but FC is valid on some docs)
            // Actually 0xFC is undefined on DMG
            0xFC => panic!("Undefined opcode: 0xFC"),

            // [FD] - Undefined
            0xFD => panic!("Undefined opcode: 0xFD"),

            // CP d8
            0xFE => {
                let v = self.fetch_byte(bus);
                self.alu_cp(v);
                8
            }

            // RST 38H
            0xFF => {
                self.push_word(bus, self.regs.pc);
                self.regs.pc = 0x0038;
                16
            }
        }
    }
}
