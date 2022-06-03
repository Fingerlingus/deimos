#![no_std]
#![no_main]
#![feature(linkage)]
#![feature(exclusive_range_pattern)]
#![feature(const_option)]
#![feature(const_mut_refs)]

use core::{panic::PanicInfo, sync::atomic::{AtomicPtr}};

use limine::*;

use constants::PAGE_SIZE;
use pager::Pager;

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
static mut KERNEL_BEGIN_VIRT: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());
static mut KERNEL_BEGIN_PHYS: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());
static mut PAGE_TABLE: Pager = Pager::new();
static mut LIMINE_TERMINAL_RESPONSE: Option<&LimineTerminalResponse> = None;

static mut LIMINE_TERMINAL_REQUEST:         LimineTerminalRequest       = LimineTerminalRequest::new(0);
static mut LIMINE_RSDP_REQUEST:             LimineRsdpRequest           = LimineRsdpRequest::new(0);
static mut LIMINE_SMBIOS_REQUEST:           LimineSmbiosRequest         = LimineSmbiosRequest::new(0);
static mut LIMINE_EFI_SYSTEM_TABLE_REQUEST: LimineEfiSystemTableRequest = LimineEfiSystemTableRequest::new(0);
static mut LIMINE_BOOT_TIME_REQUEST:        LimineBootTimeRequest       = LimineBootTimeRequest::new(0);
static mut LIMINE_KERNEL_ADDRESS_REQUEST:   LimineKernelAddressRequest  = LimineKernelAddressRequest::new(0);
static mut LIMINE_STACK_SIZE_REQUEST:       LimineStackSizeRequest      = LimineStackSizeRequest::new(0).stack_size(16 * 1024 * 1024);

#[link_section = ".limine_reqs"]
#[no_mangle]
#[used]
static LIMINE_REQUESTS_ARRAY: [AtomicPtr<()>; 8] = [
    unsafe { AtomicPtr::new(&mut LIMINE_TERMINAL_REQUEST         as *mut LimineTerminalRequest       as *mut ()) },
    unsafe { AtomicPtr::new(&mut LIMINE_RSDP_REQUEST             as *mut LimineRsdpRequest           as *mut ()) },
    unsafe { AtomicPtr::new(&mut LIMINE_SMBIOS_REQUEST           as *mut LimineSmbiosRequest         as *mut ()) },
    unsafe { AtomicPtr::new(&mut LIMINE_EFI_SYSTEM_TABLE_REQUEST as *mut LimineEfiSystemTableRequest as *mut ()) },
    unsafe { AtomicPtr::new(&mut LIMINE_BOOT_TIME_REQUEST        as *mut LimineBootTimeRequest       as *mut ()) },
    unsafe { AtomicPtr::new(&mut LIMINE_KERNEL_ADDRESS_REQUEST   as *mut LimineKernelAddressRequest  as *mut ()) },
    unsafe { AtomicPtr::new(&mut LIMINE_STACK_SIZE_REQUEST       as *mut LimineStackSizeRequest      as *mut ()) },
             AtomicPtr::new(core::ptr::null_mut()                                                    as *mut ())    
                
];
    
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
        match LIMINE_TERMINAL_RESPONSE {
            None => { },
            Some(r) => match r.terminal_count {
                0 => { },
                _ => {
                    match r.write() {
                        None => {  },
                        Some(w) => match r.terminals() {
                            None => {  },
                            Some(a) => match a.first() {
                                None => {  },
                                Some(t) => w(t, s)
                            }
                        }
                    }
                }
            }
        };
    }
}

fn printstr(s: &str) {
    printstr_terminal(s);
    printstr_serial(s);
}

fn log(s: &str) {
    printstr(s);
    printstr("\n");
}

fn done() -> ! {
    loop{}
}

#[no_mangle]
extern "C" fn entry() {
    init();
    done()
}

fn init() {
    unsafe { 
        LIMINE_TERMINAL_RESPONSE = match LIMINE_TERMINAL_REQUEST.get_response().get() {
            Some(p) => Some(p),
            None => panic("Failed to acquire limine terminal request response."),
        };

        if __stack_end as usize - __stack_start as usize <= 4096 {
            panic("Stack is too small!");
        } else {
            log("Stack size is valid.");
        }

        PAGE_TABLE.init();
        log("Page table successfully initialized.");

        KERNEL_BEGIN_VIRT = match LIMINE_KERNEL_ADDRESS_REQUEST.get_response().get() {
            None => panic("Failed to acquire limine kernel base address response."),
            Some(r) => match r.virtual_base {
                0 => panic("Limine kernel base address response virtual address is null."),
                _ => AtomicPtr::<()>::new(r.virtual_base as *mut ())
            }
        };
        log("Acquired kernel base virtual address.");

        KERNEL_BEGIN_PHYS = match LIMINE_KERNEL_ADDRESS_REQUEST.get_response().get() {
            None => panic("Failed to acquire limine kernel base address response."),
            Some(r) => match r.physical_base {
                0 => panic("Limine kernel base address response physical address is null."),
                _ => AtomicPtr::<()>::new(r.physical_base as *mut ())
            }
        };
        log("Acquired kernel base physical address.");

        KERNEL_SIZE          = Some(__kernel_end as usize - __kernel_start as usize);
        let num_pages: usize = KERNEL_SIZE.unwrap() / constants::PAGE_SIZE; 
        PAGE_TABLE.allocate_physically_contiguous_pages(Some(*KERNEL_BEGIN_PHYS.get_mut()), 
                                                        Some(*KERNEL_BEGIN_VIRT.get_mut()), 
                                                        num_pages);
        log("Kernel successfully mapped to new page table.");
        PAGE_TABLE.activate();
        log("New page table successfully loaded.");
    }
}

fn panic(s: &str) -> ! {
    printstr("PANIC: ");
    printstr(s);
    printstr("\n");
    done()
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    printstr("Panicking!");
    done()
}
