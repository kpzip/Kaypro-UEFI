use crate::memory::{Z80IO, Z80Memory};
use alloc::boxed::Box;

#[derive(Debug, Copy, Clone)]
enum Prefix {
    CB,
    ED,
}

#[derive(Debug, Clone)]
enum Phase {
    /// Base opcode fetch (M1) approximated as 4 T-states (t = 0..=3).
    Fetch {
        t: u8,
    },

    /// Fetch the opcode byte after a prefix (CB/ED). Also modeled as 4 T-states.
    PrefixFetch {
        prefix: Prefix,
        t: u8,
    },

    Exec(ExecState),
    Halted,
}

impl Default for Phase {
    fn default() -> Self {
        Phase::Fetch { t: 0 }
    }
}

#[derive(Debug, Copy, Clone)]
enum Reg8 {
    B,
    C,
    D,
    E,
    H,
    L,
    A,
}

#[derive(Debug, Copy, Clone)]
enum Reg16 {
    BC,
    DE,
    HL,
    SP,
}

#[derive(Debug, Copy, Clone)]
enum Cond {
    NZ,
    Z,
    NC,
    C,
    PO,
    PE,
    P,
    M,
}

#[derive(Debug, Clone)]
enum Next {
    End,
    Exec(Box<ExecState>),
}

impl Next {
    #[inline]
    fn exec(e: ExecState) -> Self {
        Next::Exec(Box::new(e))
    }
}

#[derive(Debug, Copy, Clone)]
enum MemDest8 {
    Reg(Reg8),
    Tmp,
}

#[derive(Debug, Copy, Clone)]
enum ImmDest8 {
    Reg(Reg8),
    Tmp,
    PortLow,
    RelOff,
}

#[derive(Debug, Copy, Clone)]
enum ImmDest16 {
    Addr,       // tmp16
    Reg(Reg16), // set rr
}

#[derive(Debug, Clone)]
enum ExecState {
    None,

    /// Do the effect once, then burn `remaining` T-states.
    Simple {
        remaining: u8,
        performed: bool,
        op: SimpleOp,
    },

    /// 3T memory read (read on t==1).
    MemRead8 {
        t: u8,
        addr: u16,
        dest: MemDest8,
        next: Next,
    },

    /// 3T memory write (write on t==1).
    MemWrite8 {
        t: u8,
        addr: u16,
        value: u8, // may be patched from tmp8 at write time
        next: Next,
    },

    /// 3T immediate read from PC (read on t==1).
    ImmRead8 {
        t: u8,
        dest: ImmDest8,
        next: Next,
    },

    /// 16-bit immediate read: lo then hi, each 3T.
    ImmRead16 {
        t: u8,
        lo: u8,
        dest: ImmDest16,
        next: Next,
    },

    /// 4T I/O read (sample on t==1).
    IoRead {
        t: u8,
        port: u16,
        dest: Reg8,
        next: Next,
    },

    /// 4T I/O write (write on t==1).
    IoWrite {
        t: u8,
        port: u16,
        value: u8,
        next: Next,
    },

    /// CB-prefixed (HL) RMW: read (3T) + internal (1T) + write (3T) => 7T after fetches
    CbMemRmw {
        t: u8, // 0..=6
        op: u8,
        addr: u16,
    },

    /// CB-prefixed BIT b,(HL): read (3T) + internal (1T) => 4T after fetches
    CbMemBit {
        t: u8, // 0..=3
        op: u8,
        addr: u16,
    },
}

#[derive(Debug, Copy, Clone)]
enum SimpleOp {
    // Basic misc
    Halt,
    Nop,

    // Loads
    LdRR { dst: Reg8, src: Reg8 },
    LdSpHl,

    // INC/DEC
    IncR { r: Reg8 },
    DecR { r: Reg8 },
    IncRR { r: Reg16 },
    DecRR { r: Reg16 },

    // 16-bit add
    AddHL { r: Reg16 },

    // ALU A, value (value usually in tmp8 for (HL)/imm paths)
    AddA { v: u8, carry: bool },
    SubA { v: u8, carry: bool },
    AndA { v: u8 },
    OrA { v: u8 },
    XorA { v: u8 },
    CpA { v: u8 },

    // Control flow
    Jp { addr: u16 },
    JpHl,
    Jr { off: i8 },
    Djnz { off: i8 },
    Call { addr: u16 },
    Ret,
    Rst { vec: u16 },

    // Stack
    Push { r: Reg16, af: bool },
    Pop { r: Reg16, af: bool },

    // Exchanges / flags / interrupts (subset)
    ExDeHl,
    ExAfAf,
    Exx,
    Di,
    Ei,
    Scf,
    Ccf,
    Cpl,

    // CB register ops
    CbOpReg { op: u8 },

    // ED: documented family
    EdAdcHl { r: Reg16 },
    EdSbcHl { r: Reg16 },
    EdNeg,
    EdReti,
    EdRetn,
    EdIm { mode: u8 }, // 0/1/2
    EdLdIFromA,
    EdLdRFromA,
    EdLdAFromI,
    EdLdAFromR,

    EdLdMemNNFromRR { r: Reg16 }, // LD (nn),rr
    EdLdRRFromMemNN { r: Reg16 }, // LD rr,(nn)

    EdOutC0,
    EdInFC,

    EdRrd,
    EdRld,

    EdLdi { repeat: bool, decrement: bool }, // LDI/LDIR/LDD/LDDR
    EdCpi { repeat: bool, decrement: bool }, // CPI/CPIR/CPD/CPDR
    EdIni { repeat: bool, decrement: bool }, // INI/INIR/IND/INDR
    EdOuti { repeat: bool, decrement: bool }, // OUTI/OTIR/OUTD/OTDR
}

#[derive(Debug, Clone)]
pub struct Z80 {
    // Registers
    program_counter: u16,
    stack_pointer: u16,

    a: u8,
    flags: u8,

    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,

    a_alt: u8,
    f_alt: u8,
    b_alt: u8,
    c_alt: u8,
    d_alt: u8,
    e_alt: u8,
    h_alt: u8,
    l_alt: u8,

    index_x: u16,
    index_y: u16,

    interrupt_flag_1: bool,
    interrupt_flag_2: bool,

    /// Used as the Z80 I register.
    interrupt_vector_base: u8,

    /// Used as the Z80 R register.
    refresh_register: u8,

    // --- extra internal state for single-T stepping ---
    phase: Phase,
    current_opcode: u8,
    mem_bank: u8,

    // prefix second byte latches
    cb_opcode: u8,
    ed_opcode: u8,

    // scratch
    tmp8: u8,
    tmp16: u16,
}

impl Default for Z80 {
    fn default() -> Self {
        Self {
            program_counter: 0x0000,
            stack_pointer: 0xFFFF,

            a: 0,
            flags: 0,

            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,

            a_alt: 0,
            f_alt: 0,
            b_alt: 0,
            c_alt: 0,
            d_alt: 0,
            e_alt: 0,
            h_alt: 0,
            l_alt: 0,

            index_x: 0,
            index_y: 0,

            interrupt_flag_1: false,
            interrupt_flag_2: false,
            interrupt_vector_base: 0,
            refresh_register: 0,

            phase: Phase::Fetch { t: 0 },
            current_opcode: 0,
            mem_bank: 1,

            cb_opcode: 0,
            ed_opcode: 0,

            tmp8: 0,
            tmp16: 0,
        }
    }
}

impl Z80 {
    pub fn new() -> Self {
        Default::default()
    }

    // Flag bits
    const F_S: u8 = 0x80;
    const F_Z: u8 = 0x40;
    const F_H: u8 = 0x10;
    const F_PV: u8 = 0x04;
    const F_N: u8 = 0x02;
    const F_C: u8 = 0x01;

    fn set_flag(&mut self, mask: u8, v: bool) {
        if v {
            self.flags |= mask;
        } else {
            self.flags &= !mask;
        }
    }
    fn get_flag(&self, mask: u8) -> bool {
        (self.flags & mask) != 0
    }

    fn parity(v: u8) -> bool {
        v.count_ones() % 2 == 0
    }

    fn set_szp(&mut self, v: u8) {
        self.set_flag(Self::F_S, (v & 0x80) != 0);
        self.set_flag(Self::F_Z, v == 0);
        self.set_flag(Self::F_PV, Self::parity(v));
    }

    fn get8(&self, r: Reg8) -> u8 {
        match r {
            Reg8::A => self.a,
            Reg8::B => self.b,
            Reg8::C => self.c,
            Reg8::D => self.d,
            Reg8::E => self.e,
            Reg8::H => self.h,
            Reg8::L => self.l,
        }
    }

    fn set8(&mut self, r: Reg8, v: u8) {
        match r {
            Reg8::A => self.a = v,
            Reg8::B => self.b = v,
            Reg8::C => self.c = v,
            Reg8::D => self.d = v,
            Reg8::E => self.e = v,
            Reg8::H => self.h = v,
            Reg8::L => self.l = v,
        }
    }

    fn get16(&self, r: Reg16) -> u16 {
        match r {
            Reg16::BC => ((self.b as u16) << 8) | (self.c as u16),
            Reg16::DE => ((self.d as u16) << 8) | (self.e as u16),
            Reg16::HL => ((self.h as u16) << 8) | (self.l as u16),
            Reg16::SP => self.stack_pointer,
        }
    }

    fn set16(&mut self, r: Reg16, v: u16) {
        match r {
            Reg16::BC => {
                self.b = (v >> 8) as u8;
                self.c = (v & 0xFF) as u8;
            }
            Reg16::DE => {
                self.d = (v >> 8) as u8;
                self.e = (v & 0xFF) as u8;
            }
            Reg16::HL => {
                self.h = (v >> 8) as u8;
                self.l = (v & 0xFF) as u8;
            }
            Reg16::SP => self.stack_pointer = v,
        }
    }

    fn hl(&self) -> u16 {
        self.get16(Reg16::HL)
    }

    fn push8(&mut self, memory: &mut impl Z80Memory, v: u8) {
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
        memory.write_byte(self.stack_pointer, self.mem_bank, v);
    }

    fn pop8(&mut self, memory: &mut impl Z80Memory) -> u8 {
        let v = memory.read_byte(self.stack_pointer, self.mem_bank);
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        v
    }

    fn update_bank_from_kaypro_crt_control(&mut self, value: u8) {
        // Kaypro: bank select is bit 7 of IO port 0x1C.
        self.mem_bank = if (value & 0x80) != 0 { 1 } else { 0 };
    }

    fn decode_reg8(bits: u8) -> Option<Reg8> {
        match bits & 0x07 {
            0 => Some(Reg8::B),
            1 => Some(Reg8::C),
            2 => Some(Reg8::D),
            3 => Some(Reg8::E),
            4 => Some(Reg8::H),
            5 => Some(Reg8::L),
            6 => None, // (HL)
            _ => Some(Reg8::A),
        }
    }

    fn cb_bit_index(op: u8) -> u8 {
        (op >> 3) & 0x07
    }

    fn cb_apply_rot_shift(&mut self, op: u8, v: u8) -> u8 {
        let group = (op >> 3) & 0x07;
        let old_c = if self.get_flag(Self::F_C) { 1u8 } else { 0u8 };
        let (res, carry_out) = match group {
            0 => {
                // RLC
                let c = (v >> 7) & 1;
                ((v << 1) | c, c != 0)
            }
            1 => {
                // RRC
                let c = v & 1;
                ((v >> 1) | (c << 7), c != 0)
            }
            2 => {
                // RL
                let c = (v >> 7) & 1;
                ((v << 1) | old_c, c != 0)
            }
            3 => {
                // RR
                let c = v & 1;
                ((v >> 1) | (old_c << 7), c != 0)
            }
            4 => {
                // SLA
                let c = (v >> 7) & 1;
                (v << 1, c != 0)
            }
            5 => {
                // SRA
                let c = v & 1;
                ((v >> 1) | (v & 0x80), c != 0)
            }
            6 => {
                // SLL (undocumented): (v<<1)|1
                let c = (v >> 7) & 1;
                ((v << 1) | 1, c != 0)
            }
            _ => {
                // SRL
                let c = v & 1;
                (v >> 1, c != 0)
            }
        };

        self.set_szp(res);
        self.set_flag(Self::F_H, false);
        self.set_flag(Self::F_N, false);
        self.set_flag(Self::F_C, carry_out);

        res
    }

    fn cb_apply_bit(&mut self, bit: u8, v: u8) {
        let mask = 1u8 << (bit & 7);
        let z = (v & mask) == 0;
        self.set_flag(Self::F_Z, z);
        self.set_flag(Self::F_H, true);
        self.set_flag(Self::F_N, false);
        self.set_flag(Self::F_S, (bit == 7) && ((v & 0x80) != 0));
        self.set_flag(Self::F_PV, z);
    }

    fn ed_adc_hl(&mut self, rr: Reg16) {
        let hl = self.get16(Reg16::HL);
        let v = self.get16(rr);
        let c = if self.get_flag(Self::F_C) { 1u16 } else { 0u16 };
        let sum32 = hl as u32 + v as u32 + c as u32;
        let res = sum32 as u16;

        self.set_flag(Self::F_H, ((hl & 0x0FFF) + (v & 0x0FFF) + c) > 0x0FFF);
        self.set_flag(Self::F_N, false);
        self.set_flag(Self::F_C, sum32 > 0xFFFF);

        self.set_flag(Self::F_S, (res & 0x8000) != 0);
        self.set_flag(Self::F_Z, res == 0);

        let overflow = (((hl ^ res) & (v ^ res)) & 0x8000) != 0;
        self.set_flag(Self::F_PV, overflow);

        self.set16(Reg16::HL, res);
    }

    fn ed_sbc_hl(&mut self, rr: Reg16) {
        let hl = self.get16(Reg16::HL);
        let v = self.get16(rr);
        let c = if self.get_flag(Self::F_C) { 1u16 } else { 0u16 };
        let diff32 = (hl as i32) - (v as i32) - (c as i32);
        let res = diff32 as u16;

        self.set_flag(
            Self::F_H,
            ((hl & 0x0FFF) as i32 - (v & 0x0FFF) as i32 - c as i32) < 0,
        );
        self.set_flag(Self::F_N, true);
        self.set_flag(Self::F_C, diff32 < 0);

        self.set_flag(Self::F_S, (res & 0x8000) != 0);
        self.set_flag(Self::F_Z, res == 0);

        let overflow = (((hl ^ v) & (hl ^ res)) & 0x8000) != 0;
        self.set_flag(Self::F_PV, overflow);

        self.set16(Reg16::HL, res);
    }

    fn simple(remaining: u8, op: SimpleOp) -> ExecState {
        ExecState::Simple {
            remaining,
            performed: false,
            op,
        }
    }

    fn next_phase(next: Next) -> Phase {
        match next {
            Next::End => Phase::Fetch { t: 0 },
            Next::Exec(e) => Phase::Exec(*e),
        }
    }

    fn decode_base_exec(&mut self, opcode: u8) -> ExecState {
        match opcode {
            0xCB => {
                self.phase = Phase::PrefixFetch {
                    prefix: Prefix::CB,
                    t: 0,
                };
                ExecState::None
            }
            0xED => {
                self.phase = Phase::PrefixFetch {
                    prefix: Prefix::ED,
                    t: 0,
                };
                ExecState::None
            }
            _ => self.decode_unprefixed_exec(opcode),
        }
    }

    fn decode_unprefixed_exec(&mut self, opcode: u8) -> ExecState {
        // Minimal-but-useful unprefixed core (you can extend further later).
        // LD r,r block (including (HL))
        if (0x40..=0x7F).contains(&opcode) && opcode != 0x76 {
            let dst_bits = (opcode >> 3) & 0x07;
            let src_bits = opcode & 0x07;

            let dst = Self::decode_reg8(dst_bits);
            let src = Self::decode_reg8(src_bits);

            match (dst, src) {
                (Some(d), Some(s)) => Self::simple(4, SimpleOp::LdRR { dst: d, src: s }),
                (Some(d), None) => ExecState::MemRead8 {
                    t: 0,
                    addr: self.hl(),
                    dest: MemDest8::Reg(d),
                    next: Next::End,
                },
                (None, Some(s)) => ExecState::MemWrite8 {
                    t: 0,
                    addr: self.hl(),
                    value: self.get8(s),
                    next: Next::End,
                },
                (None, None) => ExecState::None,
            }
        } else {
            match opcode {
                0x00 => ExecState::None, // NOP

                0x76 => Self::simple(4, SimpleOp::Halt),

                // LD r,n
                0x06 => ExecState::ImmRead8 {
                    t: 0,
                    dest: ImmDest8::Reg(Reg8::B),
                    next: Next::End,
                },
                0x0E => ExecState::ImmRead8 {
                    t: 0,
                    dest: ImmDest8::Reg(Reg8::C),
                    next: Next::End,
                },
                0x16 => ExecState::ImmRead8 {
                    t: 0,
                    dest: ImmDest8::Reg(Reg8::D),
                    next: Next::End,
                },
                0x1E => ExecState::ImmRead8 {
                    t: 0,
                    dest: ImmDest8::Reg(Reg8::E),
                    next: Next::End,
                },
                0x26 => ExecState::ImmRead8 {
                    t: 0,
                    dest: ImmDest8::Reg(Reg8::H),
                    next: Next::End,
                },
                0x2E => ExecState::ImmRead8 {
                    t: 0,
                    dest: ImmDest8::Reg(Reg8::L),
                    next: Next::End,
                },
                0x3E => ExecState::ImmRead8 {
                    t: 0,
                    dest: ImmDest8::Reg(Reg8::A),
                    next: Next::End,
                },

                // LD rr,nn
                0x01 => ExecState::ImmRead16 {
                    t: 0,
                    lo: 0,
                    dest: ImmDest16::Reg(Reg16::BC),
                    next: Next::End,
                },
                0x11 => ExecState::ImmRead16 {
                    t: 0,
                    lo: 0,
                    dest: ImmDest16::Reg(Reg16::DE),
                    next: Next::End,
                },
                0x21 => ExecState::ImmRead16 {
                    t: 0,
                    lo: 0,
                    dest: ImmDest16::Reg(Reg16::HL),
                    next: Next::End,
                },
                0x31 => ExecState::ImmRead16 {
                    t: 0,
                    lo: 0,
                    dest: ImmDest16::Reg(Reg16::SP),
                    next: Next::End,
                },

                // JP nn
                0xC3 => ExecState::ImmRead16 {
                    t: 0,
                    lo: 0,
                    dest: ImmDest16::Addr,
                    next: Next::exec(Self::simple(4, SimpleOp::Jp { addr: 0 })),
                },

                // JP (HL)
                0xE9 => Self::simple(4, SimpleOp::JpHl),

                // JR e
                0x18 => ExecState::ImmRead8 {
                    t: 0,
                    dest: ImmDest8::RelOff,
                    next: Next::exec(Self::simple(9, SimpleOp::Jr { off: 0 })),
                },

                // DJNZ e
                0x10 => ExecState::ImmRead8 {
                    t: 0,
                    dest: ImmDest8::RelOff,
                    next: Next::exec(Self::simple(10, SimpleOp::Djnz { off: 0 })),
                },

                // CALL nn
                0xCD => ExecState::ImmRead16 {
                    t: 0,
                    lo: 0,
                    dest: ImmDest16::Addr,
                    next: Next::exec(Self::simple(7, SimpleOp::Call { addr: 0 })),
                },

                // RET
                0xC9 => Self::simple(10, SimpleOp::Ret),

                // RST
                0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                    let vec = match opcode {
                        0xC7 => 0x00,
                        0xCF => 0x08,
                        0xD7 => 0x10,
                        0xDF => 0x18,
                        0xE7 => 0x20,
                        0xEF => 0x28,
                        0xF7 => 0x30,
                        _ => 0x38,
                    };
                    Self::simple(11, SimpleOp::Rst { vec })
                }

                // PUSH/POP (subset)
                0xC5 => Self::simple(
                    11,
                    SimpleOp::Push {
                        r: Reg16::BC,
                        af: false,
                    },
                ),
                0xD5 => Self::simple(
                    11,
                    SimpleOp::Push {
                        r: Reg16::DE,
                        af: false,
                    },
                ),
                0xE5 => Self::simple(
                    11,
                    SimpleOp::Push {
                        r: Reg16::HL,
                        af: false,
                    },
                ),
                0xF5 => Self::simple(
                    11,
                    SimpleOp::Push {
                        r: Reg16::HL,
                        af: true,
                    },
                ),
                0xC1 => Self::simple(
                    10,
                    SimpleOp::Pop {
                        r: Reg16::BC,
                        af: false,
                    },
                ),
                0xD1 => Self::simple(
                    10,
                    SimpleOp::Pop {
                        r: Reg16::DE,
                        af: false,
                    },
                ),
                0xE1 => Self::simple(
                    10,
                    SimpleOp::Pop {
                        r: Reg16::HL,
                        af: false,
                    },
                ),
                0xF1 => Self::simple(
                    10,
                    SimpleOp::Pop {
                        r: Reg16::HL,
                        af: true,
                    },
                ),

                // OUT (n),A
                0xD3 => ExecState::ImmRead8 {
                    t: 0,
                    dest: ImmDest8::PortLow,
                    next: Next::exec(ExecState::IoWrite {
                        t: 0,
                        port: 0,  // patched from A/tmp8
                        value: 0, // patched from A
                        next: Next::End,
                    }),
                },

                // IN A,(n)
                0xDB => ExecState::ImmRead8 {
                    t: 0,
                    dest: ImmDest8::PortLow,
                    next: Next::exec(ExecState::IoRead {
                        t: 0,
                        port: 0,
                        dest: Reg8::A,
                        next: Next::End,
                    }),
                },

                _ => ExecState::None,
            }
        }
    }

    fn decode_cb_exec(&mut self, cb_op: u8) -> ExecState {
        let r = cb_op & 0x07;
        let is_mem = r == 6;
        let group = cb_op & 0xC0;

        match group {
            0x00 => {
                // rot/shift
                if is_mem {
                    ExecState::CbMemRmw {
                        t: 0,
                        op: cb_op,
                        addr: self.hl(),
                    }
                } else {
                    Self::simple(0, SimpleOp::CbOpReg { op: cb_op })
                }
            }
            0x40 => {
                // BIT
                if is_mem {
                    ExecState::CbMemBit {
                        t: 0,
                        op: cb_op,
                        addr: self.hl(),
                    }
                } else {
                    Self::simple(0, SimpleOp::CbOpReg { op: cb_op })
                }
            }
            0x80 | 0xC0 => {
                // RES/SET
                if is_mem {
                    ExecState::CbMemRmw {
                        t: 0,
                        op: cb_op,
                        addr: self.hl(),
                    }
                } else {
                    Self::simple(0, SimpleOp::CbOpReg { op: cb_op })
                }
            }
            _ => ExecState::None,
        }
    }

    fn decode_ed_exec(&mut self, ed_op: u8) -> ExecState {
        // Full documented ED family (with common aliases where applicable).
        match ed_op {
            // -----------------------------
            // LD (nn),rr and LD rr,(nn)
            // -----------------------------
            0x43 | 0x53 | 0x63 | 0x73 => {
                let rr = match ed_op {
                    0x43 => Reg16::BC,
                    0x53 => Reg16::DE,
                    0x63 => Reg16::HL,
                    _ => Reg16::SP,
                };
                ExecState::ImmRead16 {
                    t: 0,
                    lo: 0,
                    dest: ImmDest16::Addr,
                    next: Next::exec(Self::simple(0, SimpleOp::EdLdMemNNFromRR { r: rr })),
                }
            }
            0x4B | 0x5B | 0x6B | 0x7B => {
                let rr = match ed_op {
                    0x4B => Reg16::BC,
                    0x5B => Reg16::DE,
                    0x6B => Reg16::HL,
                    _ => Reg16::SP,
                };
                ExecState::ImmRead16 {
                    t: 0,
                    lo: 0,
                    dest: ImmDest16::Addr,
                    next: Next::exec(Self::simple(0, SimpleOp::EdLdRRFromMemNN { r: rr })),
                }
            }

            // -----------------------------
            // IN r,(C)
            // -----------------------------
            0x40 | 0x48 | 0x50 | 0x58 | 0x60 | 0x68 | 0x78 => {
                let r = match ed_op {
                    0x40 => Reg8::B,
                    0x48 => Reg8::C,
                    0x50 => Reg8::D,
                    0x58 => Reg8::E,
                    0x60 => Reg8::H,
                    0x68 => Reg8::L,
                    _ => Reg8::A,
                };
                let port = ((self.b as u16) << 8) | (self.c as u16);
                ExecState::IoRead {
                    t: 0,
                    port,
                    dest: r,
                    next: Next::End,
                }
            }
            0x70 => Self::simple(0, SimpleOp::EdInFC), // IN F,(C)

            // -----------------------------
            // OUT (C),r / OUT (C),0
            // -----------------------------
            0x41 | 0x49 | 0x51 | 0x59 | 0x61 | 0x69 | 0x79 => {
                let v = match ed_op {
                    0x41 => self.b,
                    0x49 => self.c,
                    0x51 => self.d,
                    0x59 => self.e,
                    0x61 => self.h,
                    0x69 => self.l,
                    _ => self.a,
                };
                let port = ((self.b as u16) << 8) | (self.c as u16);
                ExecState::IoWrite {
                    t: 0,
                    port,
                    value: v,
                    next: Next::End,
                }
            }
            0x71 => Self::simple(0, SimpleOp::EdOutC0),

            // -----------------------------
            // 16-bit ALU: ADC/SBC HL,rr
            // -----------------------------
            0x4A | 0x5A | 0x6A | 0x7A => {
                let rr = match ed_op {
                    0x4A => Reg16::BC,
                    0x5A => Reg16::DE,
                    0x6A => Reg16::HL,
                    _ => Reg16::SP,
                };
                Self::simple(7, SimpleOp::EdAdcHl { r: rr })
            }
            0x42 | 0x52 | 0x62 | 0x72 => {
                let rr = match ed_op {
                    0x42 => Reg16::BC,
                    0x52 => Reg16::DE,
                    0x62 => Reg16::HL,
                    _ => Reg16::SP,
                };
                Self::simple(7, SimpleOp::EdSbcHl { r: rr })
            }

            // -----------------------------
            // NEG (includes aliases commonly accepted)
            // -----------------------------
            0x44 | 0x4C | 0x54 | 0x5C | 0x64 | 0x6C | 0x74 | 0x7C => {
                Self::simple(0, SimpleOp::EdNeg)
            }

            // -----------------------------
            // RETI / RETN (with aliases for RETN)
            // -----------------------------
            0x4D => Self::simple(6, SimpleOp::EdReti),
            0x45 | 0x55 | 0x5D | 0x65 | 0x6D | 0x75 | 0x7D => Self::simple(6, SimpleOp::EdRetn),

            // -----------------------------
            // IM 0/1/2 (includes aliases)
            // -----------------------------
            0x46 | 0x4E | 0x66 | 0x6E => Self::simple(0, SimpleOp::EdIm { mode: 0 }),
            0x56 | 0x76 => Self::simple(0, SimpleOp::EdIm { mode: 1 }),
            0x5E | 0x7E => Self::simple(0, SimpleOp::EdIm { mode: 2 }),

            // -----------------------------
            // LD I,A / LD R,A / LD A,I / LD A,R
            // -----------------------------
            0x47 => Self::simple(1, SimpleOp::EdLdIFromA),
            0x4F => Self::simple(1, SimpleOp::EdLdRFromA),
            0x57 => Self::simple(1, SimpleOp::EdLdAFromI),
            0x5F => Self::simple(1, SimpleOp::EdLdAFromR),

            // -----------------------------
            // RRD / RLD
            // -----------------------------
            0x67 => Self::simple(10, SimpleOp::EdRrd),
            0x6F => Self::simple(10, SimpleOp::EdRld),

            // -----------------------------
            // Block transfer
            // -----------------------------
            0xA0 => Self::simple(
                8,
                SimpleOp::EdLdi {
                    repeat: false,
                    decrement: false,
                },
            ), // LDI
            0xB0 => Self::simple(
                13,
                SimpleOp::EdLdi {
                    repeat: true,
                    decrement: false,
                },
            ), // LDIR (timing simplified)
            0xA8 => Self::simple(
                8,
                SimpleOp::EdLdi {
                    repeat: false,
                    decrement: true,
                },
            ), // LDD
            0xB8 => Self::simple(
                13,
                SimpleOp::EdLdi {
                    repeat: true,
                    decrement: true,
                },
            ), // LDDR

            // Block compare
            0xA1 => Self::simple(
                8,
                SimpleOp::EdCpi {
                    repeat: false,
                    decrement: false,
                },
            ), // CPI
            0xB1 => Self::simple(
                13,
                SimpleOp::EdCpi {
                    repeat: true,
                    decrement: false,
                },
            ), // CPIR
            0xA9 => Self::simple(
                8,
                SimpleOp::EdCpi {
                    repeat: false,
                    decrement: true,
                },
            ), // CPD
            0xB9 => Self::simple(
                13,
                SimpleOp::EdCpi {
                    repeat: true,
                    decrement: true,
                },
            ), // CPDR

            // Block input
            0xA2 => Self::simple(
                8,
                SimpleOp::EdIni {
                    repeat: false,
                    decrement: false,
                },
            ), // INI
            0xB2 => Self::simple(
                13,
                SimpleOp::EdIni {
                    repeat: true,
                    decrement: false,
                },
            ), // INIR
            0xAA => Self::simple(
                8,
                SimpleOp::EdIni {
                    repeat: false,
                    decrement: true,
                },
            ), // IND
            0xBA => Self::simple(
                13,
                SimpleOp::EdIni {
                    repeat: true,
                    decrement: true,
                },
            ), // INDR

            // Block output
            0xA3 => Self::simple(
                8,
                SimpleOp::EdOuti {
                    repeat: false,
                    decrement: false,
                },
            ), // OUTI
            0xB3 => Self::simple(
                13,
                SimpleOp::EdOuti {
                    repeat: true,
                    decrement: false,
                },
            ), // OTIR
            0xAB => Self::simple(
                8,
                SimpleOp::EdOuti {
                    repeat: false,
                    decrement: true,
                },
            ), // OUTD
            0xBB => Self::simple(
                13,
                SimpleOp::EdOuti {
                    repeat: true,
                    decrement: true,
                },
            ), // OTDR

            _ => ExecState::None,
        }
    }

    fn exec_simple(&mut self, memory: &mut impl Z80Memory, io: &mut impl Z80IO, op: SimpleOp) {
        match op {
            SimpleOp::Nop => {}
            SimpleOp::Halt => self.phase = Phase::Halted,

            SimpleOp::LdRR { dst, src } => {
                let v = self.get8(src);
                self.set8(dst, v);
            }

            SimpleOp::LdSpHl => self.stack_pointer = self.hl(),

            SimpleOp::IncR { r } => {
                let v = self.get8(r);
                let res = v.wrapping_add(1);
                self.set8(r, res);
                self.set_szp(res);
                self.set_flag(Self::F_H, (v & 0x0F) == 0x0F);
                self.set_flag(Self::F_N, false);
            }
            SimpleOp::DecR { r } => {
                let v = self.get8(r);
                let res = v.wrapping_sub(1);
                self.set8(r, res);
                self.set_szp(res);
                self.set_flag(Self::F_H, (v & 0x0F) == 0x00);
                self.set_flag(Self::F_N, true);
            }

            SimpleOp::IncRR { r } => {
                let v = self.get16(r).wrapping_add(1);
                self.set16(r, v);
            }
            SimpleOp::DecRR { r } => {
                let v = self.get16(r).wrapping_sub(1);
                self.set16(r, v);
            }

            SimpleOp::AddHL { r } => {
                let hl = self.get16(Reg16::HL);
                let rr = self.get16(r);
                let res = hl.wrapping_add(rr);
                self.set_flag(Self::F_H, ((hl & 0x0FFF) + (rr & 0x0FFF)) > 0x0FFF);
                self.set_flag(Self::F_N, false);
                self.set_flag(Self::F_C, (hl as u32 + rr as u32) > 0xFFFF);
                self.set16(Reg16::HL, res);
            }

            SimpleOp::AddA { v: 0, carry } => {
                let v = self.tmp8;
                self.exec_simple(memory, io, SimpleOp::AddA { v, carry });
            }
            SimpleOp::SubA { v: 0, carry } => {
                let v = self.tmp8;
                self.exec_simple(memory, io, SimpleOp::SubA { v, carry });
            }
            SimpleOp::AndA { v: 0 } => {
                let v = self.tmp8;
                self.exec_simple(memory, io, SimpleOp::AndA { v });
            }
            SimpleOp::OrA { v: 0 } => {
                let v = self.tmp8;
                self.exec_simple(memory, io, SimpleOp::OrA { v });
            }
            SimpleOp::XorA { v: 0 } => {
                let v = self.tmp8;
                self.exec_simple(memory, io, SimpleOp::XorA { v });
            }
            SimpleOp::CpA { v: 0 } => {
                let v = self.tmp8;
                self.exec_simple(memory, io, SimpleOp::CpA { v });
            }

            SimpleOp::AddA { v, carry } => {
                let a = self.a;
                let c = if carry && self.get_flag(Self::F_C) {
                    1
                } else {
                    0
                };
                let sum = a as u16 + v as u16 + (c as u16);
                let res = sum as u8;
                self.a = res;

                self.set_szp(res);
                self.set_flag(Self::F_H, ((a & 0x0F) + (v & 0x0F) + c) > 0x0F);
                self.set_flag(Self::F_N, false);
                self.set_flag(Self::F_C, sum > 0xFF);

                let overflow = ((a ^ res) & (v ^ res) & 0x80) != 0;
                self.set_flag(Self::F_PV, overflow);
            }

            SimpleOp::SubA { v, carry } => {
                let a = self.a;
                let c = if carry && self.get_flag(Self::F_C) {
                    1
                } else {
                    0
                };
                let diff = a as i16 - v as i16 - (c as i16);
                let res = diff as u8;
                self.a = res;

                self.set_szp(res);
                self.set_flag(
                    Self::F_H,
                    ((a & 0x0F) as i8 - (v & 0x0F) as i8 - (c as i8)) < 0,
                );
                self.set_flag(Self::F_N, true);
                self.set_flag(Self::F_C, diff < 0);

                let overflow = ((a ^ v) & (a ^ res) & 0x80) != 0;
                self.set_flag(Self::F_PV, overflow);
            }

            SimpleOp::AndA { v } => {
                let res = self.a & v;
                self.a = res;
                self.set_szp(res);
                self.set_flag(Self::F_H, true);
                self.set_flag(Self::F_N, false);
                self.set_flag(Self::F_C, false);
            }
            SimpleOp::OrA { v } => {
                let res = self.a | v;
                self.a = res;
                self.set_szp(res);
                self.set_flag(Self::F_H, false);
                self.set_flag(Self::F_N, false);
                self.set_flag(Self::F_C, false);
            }
            SimpleOp::XorA { v } => {
                let res = self.a ^ v;
                self.a = res;
                self.set_szp(res);
                self.set_flag(Self::F_H, false);
                self.set_flag(Self::F_N, false);
                self.set_flag(Self::F_C, false);
            }
            SimpleOp::CpA { v } => {
                let a = self.a;
                let diff = a as i16 - v as i16;
                let res = diff as u8;

                self.set_szp(res);
                self.set_flag(Self::F_H, ((a & 0x0F) as i8 - (v & 0x0F) as i8) < 0);
                self.set_flag(Self::F_N, true);
                self.set_flag(Self::F_C, diff < 0);

                let overflow = ((a ^ v) & (a ^ res) & 0x80) != 0;
                self.set_flag(Self::F_PV, overflow);
            }

            SimpleOp::Jp { addr: 0 } => self.program_counter = self.tmp16,
            SimpleOp::Jp { addr } => self.program_counter = addr,
            SimpleOp::JpHl => self.program_counter = self.hl(),

            SimpleOp::Jr { off: 0 } => {
                let off = self.tmp8 as i8;
                self.program_counter = self.program_counter.wrapping_add(off as i16 as u16);
            }
            SimpleOp::Jr { off } => {
                self.program_counter = self.program_counter.wrapping_add(off as i16 as u16);
            }

            SimpleOp::Djnz { off: 0 } => {
                let off = self.tmp8 as i8;
                self.b = self.b.wrapping_sub(1);
                if self.b != 0 {
                    self.program_counter = self.program_counter.wrapping_add(off as i16 as u16);
                }
            }
            SimpleOp::Djnz { off } => {
                self.b = self.b.wrapping_sub(1);
                if self.b != 0 {
                    self.program_counter = self.program_counter.wrapping_add(off as i16 as u16);
                }
            }

            SimpleOp::Call { addr: 0 } => {
                let addr = self.tmp16;
                let ret = self.program_counter;
                self.push8(memory, (ret >> 8) as u8);
                self.push8(memory, (ret & 0xFF) as u8);
                self.program_counter = addr;
            }
            SimpleOp::Call { addr } => {
                let ret = self.program_counter;
                self.push8(memory, (ret >> 8) as u8);
                self.push8(memory, (ret & 0xFF) as u8);
                self.program_counter = addr;
            }

            SimpleOp::Ret => {
                let lo = self.pop8(memory) as u16;
                let hi = self.pop8(memory) as u16;
                self.program_counter = (hi << 8) | lo;
            }

            SimpleOp::Rst { vec } => {
                let ret = self.program_counter;
                self.push8(memory, (ret >> 8) as u8);
                self.push8(memory, (ret & 0xFF) as u8);
                self.program_counter = vec;
            }

            SimpleOp::Push { r, af } => {
                let v = if af {
                    ((self.a as u16) << 8) | (self.flags as u16)
                } else {
                    self.get16(r)
                };
                self.push8(memory, (v >> 8) as u8);
                self.push8(memory, (v & 0xFF) as u8);
            }
            SimpleOp::Pop { r, af } => {
                let lo = self.pop8(memory) as u16;
                let hi = self.pop8(memory) as u16;
                let v = (hi << 8) | lo;
                if af {
                    self.a = (v >> 8) as u8;
                    self.flags = (v & 0xFF) as u8;
                } else {
                    self.set16(r, v);
                }
            }

            SimpleOp::ExDeHl => {
                core::mem::swap(&mut self.d, &mut self.h);
                core::mem::swap(&mut self.e, &mut self.l);
            }
            SimpleOp::ExAfAf => {
                core::mem::swap(&mut self.a, &mut self.a_alt);
                core::mem::swap(&mut self.flags, &mut self.f_alt);
            }
            SimpleOp::Exx => {
                core::mem::swap(&mut self.b, &mut self.b_alt);
                core::mem::swap(&mut self.c, &mut self.c_alt);
                core::mem::swap(&mut self.d, &mut self.d_alt);
                core::mem::swap(&mut self.e, &mut self.e_alt);
                core::mem::swap(&mut self.h, &mut self.h_alt);
                core::mem::swap(&mut self.l, &mut self.l_alt);
            }
            SimpleOp::Di => self.interrupt_flag_1 = false,
            SimpleOp::Ei => self.interrupt_flag_1 = true,

            SimpleOp::Scf => {
                self.set_flag(Self::F_C, true);
                self.set_flag(Self::F_H, false);
                self.set_flag(Self::F_N, false);
            }
            SimpleOp::Ccf => {
                let c = self.get_flag(Self::F_C);
                self.set_flag(Self::F_C, !c);
                self.set_flag(Self::F_H, c);
                self.set_flag(Self::F_N, false);
            }
            SimpleOp::Cpl => {
                self.a = !self.a;
                self.set_flag(Self::F_H, true);
                self.set_flag(Self::F_N, true);
            }

            // ---- CB register ops ----
            SimpleOp::CbOpReg { op } => {
                let rbits = op & 0x07;
                let target = Self::decode_reg8(rbits);
                let group = op & 0xC0;
                match (group, target) {
                    (0x00, Some(r)) => {
                        let v = self.get8(r);
                        let res = self.cb_apply_rot_shift(op, v);
                        self.set8(r, res);
                    }
                    (0x40, Some(r)) => {
                        let v = self.get8(r);
                        self.cb_apply_bit(Self::cb_bit_index(op), v);
                    }
                    (0x80, Some(r)) => {
                        let bit = Self::cb_bit_index(op);
                        let v = self.get8(r) & !(1u8 << bit);
                        self.set8(r, v);
                    }
                    (0xC0, Some(r)) => {
                        let bit = Self::cb_bit_index(op);
                        let v = self.get8(r) | (1u8 << bit);
                        self.set8(r, v);
                    }
                    _ => {}
                }
            }

            // ---- ED family ----
            SimpleOp::EdAdcHl { r } => self.ed_adc_hl(r),
            SimpleOp::EdSbcHl { r } => self.ed_sbc_hl(r),

            SimpleOp::EdNeg => {
                let a = self.a;
                self.a = 0u8.wrapping_sub(a);

                self.set_szp(self.a);
                self.set_flag(Self::F_H, (a & 0x0F) != 0);
                self.set_flag(Self::F_N, true);
                self.set_flag(Self::F_C, a != 0);

                self.set_flag(Self::F_PV, a == 0x80);
            }

            SimpleOp::EdReti | SimpleOp::EdRetn => {
                // Proper IFF2 restore differences not modeled; both behave like RET.
                let lo = self.pop8(memory) as u16;
                let hi = self.pop8(memory) as u16;
                self.program_counter = (hi << 8) | lo;
            }

            SimpleOp::EdIm { mode: _ } => {
                // Interrupt mode not stored (no dedicated field yet).
            }

            SimpleOp::EdLdIFromA => self.interrupt_vector_base = self.a,
            SimpleOp::EdLdRFromA => {
                self.refresh_register = (self.refresh_register & 0x80) | (self.a & 0x7F)
            }
            SimpleOp::EdLdAFromI => {
                self.a = self.interrupt_vector_base;
                self.set_szp(self.a);
                self.set_flag(Self::F_H, false);
                self.set_flag(Self::F_N, false);
                self.set_flag(Self::F_PV, self.interrupt_flag_2);
            }
            SimpleOp::EdLdAFromR => {
                self.a = self.refresh_register;
                self.set_szp(self.a);
                self.set_flag(Self::F_H, false);
                self.set_flag(Self::F_N, false);
                self.set_flag(Self::F_PV, self.interrupt_flag_2);
            }

            SimpleOp::EdLdMemNNFromRR { r } => {
                let addr = self.tmp16;
                let v = self.get16(r);
                memory.write_byte(addr, self.mem_bank, (v & 0x00FF) as u8);
                memory.write_byte(addr.wrapping_add(1), self.mem_bank, (v >> 8) as u8);
            }
            SimpleOp::EdLdRRFromMemNN { r } => {
                let addr = self.tmp16;
                let lo = memory.read_byte(addr, self.mem_bank) as u16;
                let hi = memory.read_byte(addr.wrapping_add(1), self.mem_bank) as u16;
                self.set16(r, (hi << 8) | lo);
            }

            SimpleOp::EdOutC0 => {
                let port = ((self.b as u16) << 8) | (self.c as u16);
                io.write_byte(port, 0);
                if (port & 0x00FF) == 0x001C {
                    self.update_bank_from_kaypro_crt_control(0);
                }
            }

            SimpleOp::EdInFC => {
                let port = ((self.b as u16) << 8) | (self.c as u16);
                let v = io.read_byte(port);
                self.set_szp(v);
                self.set_flag(Self::F_H, false);
                self.set_flag(Self::F_N, false);
            }

            SimpleOp::EdRrd => {
                let addr = self.hl();
                let m = memory.read_byte(addr, self.mem_bank);
                let a = self.a;

                let new_m = ((a & 0x0F) << 4) | (m >> 4);
                let new_a = (a & 0xF0) | (m & 0x0F);

                memory.write_byte(addr, self.mem_bank, new_m);
                self.a = new_a;

                self.set_szp(self.a);
                self.set_flag(Self::F_H, false);
                self.set_flag(Self::F_N, false);
            }

            SimpleOp::EdRld => {
                let addr = self.hl();
                let m = memory.read_byte(addr, self.mem_bank);
                let a = self.a;

                let new_m = ((m & 0x0F) << 4) | (a & 0x0F);
                let new_a = (a & 0xF0) | (m >> 4);

                memory.write_byte(addr, self.mem_bank, new_m);
                self.a = new_a;

                self.set_szp(self.a);
                self.set_flag(Self::F_H, false);
                self.set_flag(Self::F_N, false);
            }

            SimpleOp::EdLdi { repeat, decrement } => {
                let hl = self.hl();
                let de = self.get16(Reg16::DE);

                let v = memory.read_byte(hl, self.mem_bank);
                memory.write_byte(de, self.mem_bank, v);

                let step: i16 = if decrement { -1 } else { 1 };
                self.set16(Reg16::HL, hl.wrapping_add(step as u16));
                self.set16(Reg16::DE, de.wrapping_add(step as u16));

                let bc = self.get16(Reg16::BC).wrapping_sub(1);
                self.set16(Reg16::BC, bc);

                self.set_flag(Self::F_H, false);
                self.set_flag(Self::F_N, false);
                self.set_flag(Self::F_PV, bc != 0);

                if repeat && bc != 0 {
                    self.program_counter = self.program_counter.wrapping_sub(2);
                }
            }

            SimpleOp::EdCpi { repeat, decrement } => {
                let hl = self.hl();
                let v = memory.read_byte(hl, self.mem_bank);

                let a = self.a;
                let diff = (a as i16) - (v as i16);
                let res = diff as u8;

                self.set_flag(Self::F_S, (res & 0x80) != 0);
                self.set_flag(Self::F_Z, res == 0);
                self.set_flag(Self::F_H, ((a & 0x0F) as i8 - (v & 0x0F) as i8) < 0);
                self.set_flag(Self::F_N, true);
                // Carry is unaffected; leave as-is.

                let step: i16 = if decrement { -1 } else { 1 };
                self.set16(Reg16::HL, hl.wrapping_add(step as u16));

                let bc = self.get16(Reg16::BC).wrapping_sub(1);
                self.set16(Reg16::BC, bc);

                self.set_flag(Self::F_PV, bc != 0);

                if repeat && bc != 0 && !self.get_flag(Self::F_Z) {
                    self.program_counter = self.program_counter.wrapping_sub(2);
                }
            }

            SimpleOp::EdIni {
                repeat,
                decrement: _,
            } => {
                let port = ((self.b as u16) << 8) | (self.c as u16);
                let v = io.read_byte(port);

                let hl = self.hl();
                memory.write_byte(hl, self.mem_bank, v);

                self.set16(Reg16::HL, hl.wrapping_add(1));

                self.b = self.b.wrapping_sub(1);

                // Real flags are quirky; approximate.
                self.set_flag(Self::F_N, true);
                self.set_flag(Self::F_Z, self.b == 0);

                if repeat && self.b != 0 {
                    self.program_counter = self.program_counter.wrapping_sub(2);
                }
            }

            SimpleOp::EdOuti { repeat, decrement } => {
                let hl = self.hl();
                let v = memory.read_byte(hl, self.mem_bank);

                let port = ((self.b as u16) << 8) | (self.c as u16);
                io.write_byte(port, v);

                // Bank switch hook (low port byte 0x1C)
                if (port & 0x00FF) == 0x001C {
                    self.update_bank_from_kaypro_crt_control(v);
                }

                let step: i16 = if decrement { -1 } else { 1 };
                self.set16(Reg16::HL, hl.wrapping_add(step as u16));

                self.b = self.b.wrapping_sub(1);

                self.set_flag(Self::F_N, true);
                self.set_flag(Self::F_Z, self.b == 0);

                if repeat && self.b != 0 {
                    self.program_counter = self.program_counter.wrapping_sub(2);
                }
            }
        }
    }

    /// Emulate one Z80 T-state.
    pub fn clock_cycle(&mut self, memory: &mut impl Z80Memory, io: &mut impl Z80IO) {
        // Move phase out to avoid requiring `Phase: Copy`.
        let phase = core::mem::replace(&mut self.phase, Phase::Fetch { t: 0 });

        match phase {
            Phase::Halted => {
                self.phase = Phase::Halted;
                // Interrupts not modeled; remain halted.
            }

            Phase::Fetch { mut t } => match t {
                0 => self.phase = Phase::Fetch { t: 1 },
                1 => {
                    let pc = self.program_counter;
                    self.current_opcode = memory.read_byte(pc, self.mem_bank);
                    self.program_counter = self.program_counter.wrapping_add(1);

                    // R refresh: increment low 7 bits on each opcode fetch (including prefixes).
                    self.refresh_register = (self.refresh_register & 0x80)
                        | ((self.refresh_register.wrapping_add(1)) & 0x7F);

                    self.phase = Phase::Fetch { t: 2 };
                }
                2 => self.phase = Phase::Fetch { t: 3 },
                _ => {
                    let exec = self.decode_base_exec(self.current_opcode);
                    if matches!(self.phase, Phase::PrefixFetch { .. }) {
                        return; // prefix took over
                    }
                    if matches!(exec, ExecState::None) {
                        self.phase = Phase::Fetch { t: 0 };
                    } else {
                        self.phase = Phase::Exec(exec);
                    }
                }
            },

            Phase::PrefixFetch { prefix, mut t } => match t {
                0 => self.phase = Phase::PrefixFetch { prefix, t: 1 },
                1 => {
                    let pc = self.program_counter;
                    let op2 = memory.read_byte(pc, self.mem_bank);
                    self.program_counter = self.program_counter.wrapping_add(1);

                    self.refresh_register = (self.refresh_register & 0x80)
                        | ((self.refresh_register.wrapping_add(1)) & 0x7F);

                    match prefix {
                        Prefix::CB => self.cb_opcode = op2,
                        Prefix::ED => self.ed_opcode = op2,
                    }

                    self.phase = Phase::PrefixFetch { prefix, t: 2 };
                }
                2 => self.phase = Phase::PrefixFetch { prefix, t: 3 },
                _ => {
                    let exec = match prefix {
                        Prefix::CB => self.decode_cb_exec(self.cb_opcode),
                        Prefix::ED => self.decode_ed_exec(self.ed_opcode),
                    };
                    if matches!(exec, ExecState::None) {
                        self.phase = Phase::Fetch { t: 0 };
                    } else {
                        self.phase = Phase::Exec(exec);
                    }
                }
            },

            Phase::Exec(state) => {
                let mut next_phase: Option<Phase> = None;

                match state {
                    ExecState::None => next_phase = Some(Phase::Fetch { t: 0 }),

                    ExecState::Simple {
                        mut remaining,
                        mut performed,
                        op,
                    } => {
                        if !performed {
                            self.exec_simple(memory, io, op);
                            performed = true;
                        }

                        if remaining > 0 {
                            remaining -= 1;
                        }

                        if matches!(self.phase, Phase::Halted) {
                            return;
                        }

                        if remaining == 0 {
                            next_phase = Some(Phase::Fetch { t: 0 });
                        } else {
                            next_phase = Some(Phase::Exec(ExecState::Simple {
                                remaining,
                                performed,
                                op,
                            }));
                        }
                    }

                    ExecState::ImmRead8 { mut t, dest, next } => {
                        match t {
                            0 => t = 1,
                            1 => {
                                let v = memory.read_byte(self.program_counter, self.mem_bank);
                                self.program_counter = self.program_counter.wrapping_add(1);
                                match dest {
                                    ImmDest8::Reg(r) => self.set8(r, v),
                                    ImmDest8::Tmp => self.tmp8 = v,
                                    ImmDest8::PortLow => self.tmp8 = v,
                                    ImmDest8::RelOff => self.tmp8 = v,
                                }
                                t = 2;
                            }
                            _ => next_phase = Some(Self::next_phase(next.clone())),
                        }
                        if next_phase.is_none() {
                            next_phase = Some(Phase::Exec(ExecState::ImmRead8 { t, dest, next }));
                        }
                    }

                    ExecState::ImmRead16 {
                        mut t,
                        mut lo,
                        dest,
                        next,
                    } => {
                        match t {
                            0 => t = 1,
                            1 => {
                                lo = memory.read_byte(self.program_counter, self.mem_bank);
                                self.program_counter = self.program_counter.wrapping_add(1);
                                t = 2;
                            }
                            2 => t = 3,
                            3 => t = 4,
                            4 => {
                                let hi = memory.read_byte(self.program_counter, self.mem_bank);
                                self.program_counter = self.program_counter.wrapping_add(1);
                                let v = ((hi as u16) << 8) | (lo as u16);
                                match dest {
                                    ImmDest16::Addr => self.tmp16 = v,
                                    ImmDest16::Reg(r) => self.set16(r, v),
                                }
                                t = 5;
                            }
                            _ => next_phase = Some(Self::next_phase(next.clone())),
                        }
                        if next_phase.is_none() {
                            next_phase =
                                Some(Phase::Exec(ExecState::ImmRead16 { t, lo, dest, next }));
                        }
                    }

                    ExecState::MemRead8 {
                        mut t,
                        addr,
                        dest,
                        next,
                    } => {
                        match t {
                            0 => t = 1,
                            1 => {
                                let v = memory.read_byte(addr, self.mem_bank);
                                match dest {
                                    MemDest8::Reg(r) => self.set8(r, v),
                                    MemDest8::Tmp => self.tmp8 = v,
                                }
                                t = 2;
                            }
                            _ => next_phase = Some(Self::next_phase(next.clone())),
                        }
                        if next_phase.is_none() {
                            next_phase = Some(Phase::Exec(ExecState::MemRead8 {
                                t,
                                addr,
                                dest,
                                next,
                            }));
                        }
                    }

                    ExecState::MemWrite8 {
                        mut t,
                        addr,
                        mut value,
                        next,
                    } => {
                        match t {
                            0 => t = 1,
                            1 => {
                                if value == 0 {
                                    value = self.tmp8;
                                }
                                memory.write_byte(addr, self.mem_bank, value);
                                t = 2;
                            }
                            _ => next_phase = Some(Self::next_phase(next.clone())),
                        }
                        if next_phase.is_none() {
                            next_phase = Some(Phase::Exec(ExecState::MemWrite8 {
                                t,
                                addr,
                                value,
                                next,
                            }));
                        }
                    }

                    ExecState::IoWrite {
                        mut t,
                        mut port,
                        mut value,
                        next,
                    } => {
                        if port == 0 {
                            // OUT (n),A form
                            port = ((self.a as u16) << 8) | (self.tmp8 as u16);
                        }
                        if value == 0 {
                            value = self.a;
                        }

                        match t {
                            0 => t = 1,
                            1 => {
                                io.write_byte(port, value);

                                // Kaypro bank switching on port 0x1C, bit7.
                                if (port & 0x00FF) == 0x001C {
                                    self.update_bank_from_kaypro_crt_control(value);
                                }

                                t = 2;
                            }
                            2 => t = 3,
                            _ => next_phase = Some(Self::next_phase(next.clone())),
                        }

                        if next_phase.is_none() {
                            next_phase = Some(Phase::Exec(ExecState::IoWrite {
                                t,
                                port,
                                value,
                                next,
                            }));
                        }
                    }

                    ExecState::IoRead {
                        mut t,
                        mut port,
                        dest,
                        next,
                    } => {
                        if port == 0 {
                            port = ((self.a as u16) << 8) | (self.tmp8 as u16);
                        }

                        match t {
                            0 => t = 1,
                            1 => {
                                let v = io.read_byte(port);
                                self.set8(dest, v);
                                t = 2;
                            }
                            2 => t = 3,
                            _ => next_phase = Some(Self::next_phase(next.clone())),
                        }

                        if next_phase.is_none() {
                            next_phase = Some(Phase::Exec(ExecState::IoRead {
                                t,
                                port,
                                dest,
                                next,
                            }));
                        }
                    }

                    ExecState::CbMemRmw { mut t, op, addr } => {
                        match t {
                            0 => t = 1,
                            1 => {
                                self.tmp8 = memory.read_byte(addr, self.mem_bank);
                                t = 2;
                            }
                            2 => t = 3,
                            3 => {
                                let group = op & 0xC0;
                                let bit = Self::cb_bit_index(op);
                                self.tmp8 = match group {
                                    0x00 => self.cb_apply_rot_shift(op, self.tmp8),
                                    0x80 => self.tmp8 & !(1u8 << bit),
                                    0xC0 => self.tmp8 | (1u8 << bit),
                                    _ => self.tmp8,
                                };
                                t = 4;
                            }
                            4 => t = 5,
                            5 => {
                                memory.write_byte(addr, self.mem_bank, self.tmp8);
                                t = 6;
                            }
                            _ => next_phase = Some(Phase::Fetch { t: 0 }),
                        }

                        if next_phase.is_none() {
                            next_phase = Some(Phase::Exec(ExecState::CbMemRmw { t, op, addr }));
                        }
                    }

                    ExecState::CbMemBit { mut t, op, addr } => {
                        match t {
                            0 => t = 1,
                            1 => {
                                self.tmp8 = memory.read_byte(addr, self.mem_bank);
                                t = 2;
                            }
                            2 => t = 3,
                            _ => {
                                let bit = Self::cb_bit_index(op);
                                self.cb_apply_bit(bit, self.tmp8);
                                next_phase = Some(Phase::Fetch { t: 0 });
                            }
                        }

                        if next_phase.is_none() {
                            next_phase = Some(Phase::Exec(ExecState::CbMemBit { t, op, addr }));
                        }
                    }
                }

                if matches!(self.phase, Phase::Halted) {
                    return;
                }
                if let Some(p) = next_phase {
                    self.phase = p;
                }
            }
        }
    }
}
