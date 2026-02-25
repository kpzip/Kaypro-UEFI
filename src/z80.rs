use crate::memory::Kaypro2Memory;

#[derive(Debug, Copy, Clone, Default)]
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
    interrupt_vector_base: u8,
    refresh_register: u8,
}

impl Z80 {

    pub fn new() -> Self {
        Default::default()
    }

    pub fn clock_cycle(&mut self) {

    }

    pub fn fetch(&self, memory: &Kaypro2Memory) -> [u8; 4] {
        let mut ret = [0; 4];
        for i in 0..4 {
            ret[i] = memory.read(self.program_counter + i, 0);
        }
        ret
    }

    pub fn execute(&mut self, instruction: [u8; 4], memory: &mut Kaypro2Memory) -> u8 {

    }


}
