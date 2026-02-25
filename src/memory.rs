
// Bank 1:
// ROM: 0x0000 0x07ff
// VRAM 0x0300 0x3bff
//

const ROM: &'static [u8; _] = include_bytes!("../rom/test.bin");

#[derive(Debug, Copy, Clone)]
pub struct Kaypro2Memory {
    ram: [u8; 64 * 1024],
    vram: [u8; 2 * 1024],
}

impl Kaypro2Memory {

    fn new() -> Self {
        Self {
            ram: [0; _],
            vram: [0; _],
        }
    }

    pub fn read(&self, address: u16, bank: u8) -> u8 {
        if bank == 0 {
            self.ram[address as usize]
        } else {
            match address {
                0x0000..=0x07FF => ROM[(address - 0x0000) as usize],
                0x0300..=0x3BFF => self.vram[(address - 0x0300) as usize],
                0x03FF..=0x3FFF => self.ram[(address - 0x03FF) as usize],
                _ => 0,
            }
        }
    }

    pub fn write(&mut self, address: u16, value: u8, bank: u8) {
        if bank == 0 {
            self.ram[address as usize] = value;
        } else {
            match address {
                0x0000..=0x07FF => {},
                0x0300..=0x3BFF => self.vram[(address - 0x0300) as usize] = value,
                0x03FF..=0x3FFF => self.ram[(address - 0x03FF) as usize] = value,
                _ => {},
            };
        }
    }
}