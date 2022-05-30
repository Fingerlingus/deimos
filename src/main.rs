#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::arch::asm;
use stivale_boot::v2::{StivaleFramebufferHeaderTag, StivaleHeader};

static COM1: u16 = 0x3F8;
static STACK: [u8; 4096] = [0; 4096];

static FRAMEBUFFER_TAG: StivaleFramebufferHeaderTag =
    StivaleFramebufferHeaderTag::new().framebuffer_bpp(24);

#[link_section = ".stivale2hdr"]
#[no_mangle]
#[used]
static STIVALE_HDR: StivaleHeader = StivaleHeader::new()
    .stack(&STACK[4095] as *const u8)
    .tags((&FRAMEBUFFER_TAG as *const StivaleFramebufferHeaderTag).cast());

fn printstr_serial<const N: usize>(s: &[u8; N]) {
    for &char in s.iter() {
        unsafe {
            asm!("out dx, al", in("dx") COM1, in("al") char);
        }
    }
}

#[no_mangle]
extern "C" fn entry_point(_header_addr: usize) -> ! {
    printstr_serial(b"Hello, world!\n");
    loop {}
}

#[panic_handler]
fn panic(_infos: &PanicInfo) -> ! {
    loop {}
}
