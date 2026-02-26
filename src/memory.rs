// Bank 1:
// ROM: 0x0000 0x07ff
// VRAM 0x0300 0x3bff
//

use core::slice;

const ROM_RAW: &[u8] = include_bytes!("../rom/test.bin");
const ROM: [u8; 2048] = {
    let mut temp = [0u8; 2048];
    let mut i = 0;
    while i < ROM_RAW.len() {
        temp[i] = ROM_RAW[i];
        i += 1;
    }
    temp
};

pub trait Z80Memory {
    fn new() -> Self;

    fn read_byte(&self, addr: u16, bank: u8) -> u8;

    fn write_byte(&mut self, addr: u16, bank: u8, value: u8);
}

pub trait Z80IO {
    fn new() -> Self;

    fn read_byte(&self, addr: u16) -> u8;

    fn write_byte(&mut self, addr: u16, value: u8);
}

#[derive(Debug, Copy, Clone)]
pub struct Kaypro2Memory {
    ram: [u8; 64 * 1024],
    vram: [u8; 2 * 1024],
}

impl Z80Memory for Kaypro2Memory {
    fn new() -> Self {
        Self {
            ram: [0; _],
            vram: [0; _],
        }
    }

    fn read_byte(&self, addr: u16, bank: u8) -> u8 {
        if bank == 0 {
            self.ram[addr as usize]
        } else {
            match addr {
                0x0000..=0x07FF => ROM[(addr - 0x0000) as usize],
                0x0300..=0x3BFF => self.vram[(addr - 0x0300) as usize],
                0x03FF..=0x3FFF => self.ram[(addr - 0x03FF) as usize],
                _ => 0,
            }
        }
    }

    fn write_byte(&mut self, addr: u16, bank: u8, value: u8) {
        if bank == 0 {
            self.ram[addr as usize] = value;
        } else {
            match addr {
                0x0000..=0x07FF => {}
                0x0300..=0x3BFF => self.vram[(addr - 0x0300) as usize] = value,
                0x03FF..=0x3FFF => self.ram[(addr - 0x03FF) as usize] = value,
                _ => {}
            };
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Kaypro2IO {
    serial_baud_rate: [u8; 4],     // 0x00-0x03
    serial_data_io: u8,            // 0x04
    keyboard_data: u8,             // 0x05
    serial_control: u8,            // 0x06
    keyboard_control: u8,          // 0x07
    printer_baud_rate: [u8; 4],    // 0x08-0x0B
    serial_printer_data_io: u8,    // 0x0C
    modem_data_io: u8,             // 0x0D
    serial_printer_control: u8,    // 0x0E
    modem_status: u8,              // 0x0F
    floppy_command: u8,            // 0x10
    floppy_track_register: u8,     // 0x11
    floppy_sector_register: u8,    // 0x12
    floppy_data: u8,               // 0x13
    system_io_port: [u8; 4],       // 0x14-0x17
    parallel_printer_out: [u8; 4], // 0x18-1B
    crt_control: u8,               // 0x1C
    crt_data_io: u8,               // 0x1D
    padding_1: [u8; 2],            // 0x1E-0x1F
    rtc_register_select: u8,       // 0x20
    modem_control: u8,             // 0x21
    rtc_pio_control: u8,           // 0x22
    modem_pio_control: u8,         // 0x23
    rtc_data_io: u8,               // 0x24
    padding_2: [u8; 0x5B],         // 0x25-0x7F
    hard_disk_data_io: u8,         // 0x80
    hard_disk_error: u8,           // 0x81
    hard_disk_sector_count: u8,    // 0x82
    hard_disk_sector_number: u8,   // 0x83
    hard_disk_cylinder_low: u8,    // 0x84
    hard_disk_cylinder_high: u8,   // 0x84
    size_drive_head_reg: u8,       // 0x85
    hard_disk_status_command: u8,  // 0x86
}

impl Z80IO for Kaypro2IO {
    fn new() -> Kaypro2IO {
        Self {
            serial_baud_rate: [0; 4],
            serial_data_io: 0,
            keyboard_data: 0,
            serial_control: 0,
            keyboard_control: 0,
            printer_baud_rate: [0; 4],
            serial_printer_data_io: 0,
            modem_data_io: 0,
            serial_printer_control: 0,
            modem_status: 0,
            floppy_command: 0,
            floppy_track_register: 0,
            floppy_sector_register: 0,
            floppy_data: 0,
            system_io_port: [0; 4],
            parallel_printer_out: [0; 4],
            crt_control: 0,
            crt_data_io: 0,
            padding_1: [0; 2],
            rtc_register_select: 0,
            modem_control: 0,
            rtc_pio_control: 0,
            modem_pio_control: 0,
            rtc_data_io: 0,
            padding_2: [0; 91],
            hard_disk_data_io: 0,
            hard_disk_error: 0,
            hard_disk_sector_count: 0,
            hard_disk_sector_number: 0,
            hard_disk_cylinder_low: 0,
            hard_disk_cylinder_high: 0,
            size_drive_head_reg: 0,
            hard_disk_status_command: 0,
        }
    }

    fn read_byte(&self, addr: u16) -> u8 {
        let ptr: *const Self = self;
        let byte_ptr: *const u8 = ptr as *const u8;
        let byte_slice: &[u8] = unsafe { slice::from_raw_parts(byte_ptr, size_of::<Self>()) };
        byte_slice[addr as usize]
    }

    fn write_byte<'a>(&'a mut self, addr: u16, value: u8) {
        let ptr: *mut Self = self;
        let byte_ptr: *mut u8 = ptr as *mut u8;
        let byte_slice: &'a mut [u8] =
            unsafe { slice::from_raw_parts_mut(byte_ptr, size_of::<Self>()) };
        byte_slice[addr as usize] = value;
    }
}
