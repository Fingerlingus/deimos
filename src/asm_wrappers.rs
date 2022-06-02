use core::arch::asm;

pub unsafe extern "C" fn inb(port: u16) -> u8 {
    let mut _data: u8 = 0x00;
    asm!("in al, dx", in("dx") port, out("al") _data);
    _data
}

pub unsafe extern "C" fn inw(port: u16) -> u16 {
    let mut _data: u16 = 0x00;
    asm!("in ax, dx", in("dx") port, out("ax") _data);
    _data
}

pub unsafe extern "C" fn ind(port: u16) -> u32 {
    let mut _data: u32 = 0x00;
    asm!("in eax, dx", in("dx") port, out("eax") _data);
    _data
}

pub unsafe extern "C" fn outb(port: u16, data: u8) {
    asm!("out dx, al", in("dx") port, in("al") data);
}

pub unsafe extern "C" fn outw(port: u16, data: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") data);
}

pub unsafe extern "C" fn outd(port: u16, data: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") data);
}

pub unsafe extern "C" fn lcr3(pml4t_phys_addr: usize) {
    asm!("mov cr3, {p}", p = in(reg) pml4t_phys_addr);
}