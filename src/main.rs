#![no_main]
#![no_std]

extern crate alloc;
mod memory;
mod z80;

use core::time::Duration;
use log::info;
use uefi::prelude::*;
use crate::memory::{Kaypro2IO, Kaypro2Memory, Z80Memory, Z80IO};
use crate::z80::Z80;

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    let mut z80 = Z80::new();
    let mut kaypro_memory = Kaypro2Memory::new();
    let mut kaypro_io = Kaypro2IO::new();
    loop {
        z80.clock_cycle(&mut kaypro_memory, &mut kaypro_io);
        
    }
}
