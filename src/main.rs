#![no_std]
#![no_main]
#![feature(linkage)]
#![feature(exclusive_range_pattern)]
#![feature(const_option)]

use core::panic::PanicInfo;
use constants::PAGE_SIZE;
use pager::Pager;
use stivale_boot::v2::*;

mod constants;
mod pager;
mod asm_wrappers;

const COM1: u16 = 0x03F8;


static INIT_STACK: [u8; 4096] = [0; 4096];

extern "C" {
    #[linkage = "external"] static __stack_start:  *const ();
    #[linkage = "external"] static __stack_end:    *const ();
    #[linkage = "external"] static __kernel_start: *const ();
    #[linkage = "external"] static __kernel_end:   *const ();
}

static mut KERNEL_SIZE: Option<usize> = None;
static mut KERNEL_BEGIN_VIRT: Option<usize> = None;
static mut KERNEL_BEGIN_PHYS: Option<usize> = None;
static mut TERM_TAG: Option<&StivaleTerminalTag> = None;
static mut PAGE_TABLE: Pager = Pager::new();

static STIVALE_FRAMEBUFFER_TAG: StivaleFramebufferHeaderTag = StivaleFramebufferHeaderTag::new();

static STIVALE_TERMINAL_TAG: StivaleTerminalHeaderTag = StivaleTerminalHeaderTag::new()
    .next((&STIVALE_FRAMEBUFFER_TAG as *const StivaleFramebufferHeaderTag).cast());

#[link_section = ".stivale2hdr"]
#[no_mangle]
#[used]
static STIVALE_HDR: StivaleHeader = StivaleHeader::new()
    .stack(&INIT_STACK[4095] as *const u8)
    .tags((&STIVALE_TERMINAL_TAG as *const StivaleTerminalHeaderTag).cast());  
    
unsafe fn write_com1(c: u8) {
    asm_wrappers::outb(COM1, c);
}

fn printstr_serial(s: &str) {
    for &c in s.as_bytes().iter() {
        unsafe {
            write_com1(c);
        }
    }
}

fn printstr_terminal(s: &str) {
    unsafe {
        match TERM_TAG {
            Some(t) => t.term_write()(s),
            None => { }
        };
    }
}

fn printstr(s: &str) {
    printstr_terminal(s);
    printstr_serial(s);
}

fn done() -> ! {
    loop{}
}

#[no_mangle]
extern "C" fn entry(boot_info: &'static StivaleStruct) {
    init(boot_info);
    done()
}

fn init(boot_info: &'static StivaleStruct) {
    unsafe { 
        TERM_TAG = match boot_info.terminal() {
            Some(t) => Some(t),
            None => None
        };

        if __stack_end as usize - __stack_start as usize <= 4096 {
            panic("Stack is too small!");
        } else {
            printstr("Stack size is valid.\n");
        }

        PAGE_TABLE.init();
        printstr("Page table successfully initialized.\n");
        KERNEL_BEGIN_VIRT = Some(boot_info.kernel_base_addr().unwrap().virtual_base_address as usize);
        KERNEL_BEGIN_PHYS = Some(boot_info.kernel_base_addr().unwrap().physical_base_address as usize);
        KERNEL_SIZE       = Some(__kernel_end as usize - __kernel_start as usize);
        let num_pages: usize = KERNEL_SIZE.unwrap() / constants::PAGE_SIZE;
        PAGE_TABLE.allocate_physically_contiguous_pages(Some(KERNEL_BEGIN_PHYS.unwrap() as *const()), 
                                                        Some(KERNEL_BEGIN_VIRT.unwrap() as *const()), 
                                                        num_pages);
        PAGE_TABLE.activate();
    }
}

fn panic(s: &str) {
    printstr(s);
    done()
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    printstr("Panicking!");
    done()
}
