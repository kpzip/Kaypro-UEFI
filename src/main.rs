#![no_main]
#![no_std]

mod z80;
mod memory;

use core::time::Duration;
use log::info;
use uefi::prelude::*;

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    info!("Hello, World!");
    boot::stall(Duration::from_secs(10));
    Status::SUCCESS
}
