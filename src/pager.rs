use core::mem::size_of;

use crate::constants::*;

const NUM_PML4TES: usize = match VMEM_MAX / (512 * 1024 * 1024 * 1024) {
    0 => 1,
    1..511 => VMEM_MAX / (512 * 1024 * 1024 * 1024),
    _ => 512
};

const NUM_PDPTES: usize = match VMEM_MAX / (1 * 1024 * 1024 * 1024) {
    0 => 1,
    1..511 => VMEM_MAX / (1 * 1024 * 1024 * 1024),
    _ => 512
};

const NUM_PDTES: usize = match VMEM_MAX / (2 * 1024 * 1024) {
    0 => 1,
    1..511 => VMEM_MAX / (2 * 1024 * 1024),
    _ => 512
};

const NUM_PHYS_ADDR_MAP_ENTRIES: usize = MAX_PAGES / 64;

pub struct Pager {
    ptes: [[[[u64; 512]; NUM_PDTES  ]; NUM_PDPTES ]; NUM_PML4TES],
    pdtes: [[[u64; 512]; NUM_PDPTES ]; NUM_PML4TES],
    pdptes: [[u64; 512]; NUM_PML4TES],
    pml4tes: [u64; 512],

    allocated_pages: [Option<*const ()>; MAX_PAGES],
    phys_addr_map:   [u64; NUM_PHYS_ADDR_MAP_ENTRIES],

    last_mapped_phys_addr: Option<*const ()>,
    last_mapped_virt_addr: Option<*const ()>
}

fn memset<T>(t: &mut T, c: u8) {
    let ptr = t as *mut T as *mut u8;
    for i in 0..size_of::<T>() {
        unsafe {
            *((ptr as usize + i) as *mut u8) = c;
        }
    }
}

impl Pager {
    pub fn new() -> Self {
        Pager {
            ptes: [[[[0; 512]; NUM_PDTES  ]; NUM_PDPTES ]; NUM_PML4TES],
            pdtes: [[[0; 512]; NUM_PDPTES ]; NUM_PML4TES],
            pdptes: [[0; 512]; NUM_PML4TES],
            pml4tes: [0; 512],

            allocated_pages: [None; MAX_PAGES],
            phys_addr_map:   [0;    NUM_PHYS_ADDR_MAP_ENTRIES],

            last_mapped_phys_addr: None,
            last_mapped_virt_addr: None
        }
    }

    pub fn init(&mut self) {
        //memset(self.ptes, 0);
        //memset(self.pdtes, 0);
        //memset(self.pdptes, 0);
        //memset(self.pml4tes, 0);
        //memset(self.phys_addr_map, 0);

        self.pml4tes[0]   = (&self.pdptes[0][0]   as *const u64 as u64 & !0xFFF) | 0x03;
        self.pdptes[0][0] = (&self.pdtes[0][0][0] as *const u64 as u64 & !0xFFF) | 0x03;

        for i in 0..8 {
            self.pdtes[0][0][i] = (&self.ptes[0][0][i][0] as *const u64 as u64 & !0xFFF) | 0x03;
            for j in 0..512 {
                self.ptes[0][0][i][j] = ((((i * PT_SIZE) + (j * PAGE_SIZE)) & !0xFFF) | 0x03) as u64;
            }
        }
    }

    pub fn is_virtually_allocated(&self, ptr: Option<*const ()>) -> bool {
        if ptr.is_none() {
            return true
        }

        let pml4t_index: usize = (ptr.unwrap() as usize >> 39) & 0x1FF;
        let pdpt_index:  usize = (ptr.unwrap() as usize >> 30) & 0x1FF;
        let pdt_index:   usize = (ptr.unwrap() as usize >> 21) & 0x1FF;
        let pt_index:    usize = (ptr.unwrap() as usize >> 12) & 0x1FF;

        if self.pml4tes[pml4t_index] & 0x01 == 0x00000000 {
            return false
        }

        if self.pdptes[pml4t_index][pdpt_index] & 0x01 == 0x00000000 {
            return false
        }

        if self.pdtes[pml4t_index][pdpt_index][pdt_index] & 0x01 == 0x00000000 {
            return false
        }
    
        self.ptes[pml4t_index][pdpt_index][pdt_index][pt_index] & 0x01 != 0x00000000
    }

    pub fn is_physically_allocated(&self, ptr: Option<*const ()>) -> bool {
        if ptr.is_none() {
            return true
        }

        if ptr == self.last_mapped_phys_addr {
            return true
        }

        if ptr.unwrap() < ((16 * 1024 * 1024) as *const ()) {
            return true 
        }

        let temp   = (ptr.unwrap() as usize & !0xFFF) / PAGE_SIZE;
        let index  = temp / 64;
        let offset = temp % 64;

        self.phys_addr_map[index] & (1 << offset) != 0x00000000
    }

    pub fn last_mapped_phys_addr(&self) -> Option<*const ()> {
        self.last_mapped_phys_addr
    }

    pub fn last_mapped_virt_addr(&self) -> Option<*const ()> {
        self.last_mapped_phys_addr
    }

    pub fn allocated_pages(&self) -> &[Option<*const ()>; MAX_PAGES] {
        &self.allocated_pages
    }

    pub fn pml4t(&self) -> &[u64; 512] {
        &self.pml4tes
    }

    pub fn activate(&self) {

    }

    pub fn map_virt_addr_to_phys_addr(&mut self, vaddr: Option<*const ()>, paddr: Option<*const ()>) -> u8 {
        if vaddr.is_none() {
            return 1
        }

        if paddr.is_none() {
            return 2
        }

        if (vaddr.unwrap() as usize & 0xFFF) != 0x000 {
            return 3
        }

        if (paddr.unwrap() as usize & 0xFFF) != 0x000 {
            return 4
        }

        if self.is_virtually_allocated(vaddr) {
            return 5
        }
 
        let pml4t_index: usize = (vaddr.unwrap() as usize >> 39) & 0x1FF;
        let pdpt_index:  usize = (vaddr.unwrap() as usize >> 30) & 0x1FF;
        let pdt_index:   usize = (vaddr.unwrap() as usize >> 21) & 0x1FF;
        let pt_index:    usize = (vaddr.unwrap() as usize >> 12) & 0x1FF;

        if (self.pml4tes[pml4t_index] & 0x03) != 0x03 {
            self.pml4tes[pml4t_index] = (&self.pdptes[pml4t_index][pdpt_index] as *const _) as u64 | 0x03;
        }

        if (self.pdptes[pml4t_index][pdpt_index] & 0x03) != 0x03 {
            self.pdptes[pml4t_index][pdpt_index] = 
                (&self.pdtes[pml4t_index][pdpt_index][pdt_index] as *const _) as u64 | 0x03;
        }

        if (self.pdtes[pml4t_index][pdpt_index][pdt_index] & 0x03) != 0x03 {
            self.pdtes[pml4t_index][pdpt_index][pdt_index] = 
                (&self.ptes[pml4t_index][pdpt_index][pdt_index][pt_index] as *const _) as u64 | 0x03;
        }

        if (self.ptes[pml4t_index][pdpt_index][pdt_index][pt_index] & 0x03) != 0x03 {
            self.ptes[pml4t_index][pdpt_index][pdt_index][pt_index] = paddr.unwrap() as u64 | 0x03;
        }
 
        let temp   = (paddr.unwrap() as usize & !0xFFF) / PAGE_SIZE;
        let index  = temp / 64;
        let offset = temp % 64; 

        self.phys_addr_map[index] |= 1 << offset;
        return 0
    }

    fn allocate_page(&mut self, ptr: Option<*const ()>) -> Option<*const ()> {
        let vaddr: *const () = match ptr {
            None => match self.find_free_virt_addr() {
                None => return None,
                Some(p) => p
            },
            Some(p) => match p as usize & 0xFFF == 0 {
                false => return None,
                true => match self.is_virtually_allocated(ptr) {
                    false => p,
                    true => return None
                }
            }
        };

        if (vaddr as usize & 0xFFF) != 0x000 {
            return None
        }

        let paddr = match self.find_free_phys_addr() {
            None => return None,
            Some(p) => p
        };

        return match self.map_virt_addr_to_phys_addr(Some(vaddr), Some(paddr)) {
            0 => {
                self.last_mapped_phys_addr = Some(paddr);
                self.last_mapped_virt_addr = Some(vaddr);
                Some(paddr)
            },
            _ => None
        }
    }

    fn allocate_pages(&mut self, ptr: Option<*const ()>, num_pages: usize) -> Option<*const ()> {
        match num_pages {
            0 => return None,
            1 => return match self.is_virtually_allocated(ptr) {
                true => ptr,
                false => self.allocate_page(ptr)
            },
            _ => match ptr {
                Some(_n) => {
                    for i in 0..num_pages {
                        let p: *const () = (ptr.unwrap() as usize + i * PAGE_SIZE) as *const ();
                        if !self.is_virtually_allocated(Some(p)) {
                            match self.allocate_page(Some(p)) {
                                None => return None,
                                _ => { }
                            };
                        }
                    }
                    return ptr
                },
                None => {
                    let mut p: *const () = (16 * 1024 * 1024) as *const ();
                    loop {
                        if p as usize + num_pages * PAGE_SIZE > VMEM_MAX {
                            break;
                        }
                        let mut b: bool = false;
                        for i in 0..num_pages {
                            if self.is_virtually_allocated(Some((p as usize + i * PAGE_SIZE) as *const ())) {
                                    b = true;
                                break;
                            }
                        }
                        if b == false {
                            return self.allocate_pages(ptr, num_pages)
                        }

                        p = (p as usize + PAGE_SIZE) as *const ();
                    }
                    None
                }
            }
        }
    }

    fn unmap_phys_addr(&mut self, ptr: Option<*const ()>) -> u8 {
        if ptr.is_none() {
            return 1
        }

        if (ptr.unwrap() as usize & 0xFFF) != 0x000 {
            return 2
        }

        if !self.is_physically_allocated(ptr) {
            return 3
        }

        let temp = ptr.unwrap() as usize / PAGE_SIZE;
        let index = temp / 64;
        let offset = temp % 64;

        self.phys_addr_map[index] &= !(1 << offset);
        return 0
    }

    fn unmap_virt_addr(&mut self, ptr: Option<*const ()>) -> u8 {
        if ptr.is_none() {
            return 1
        }

        if (ptr.unwrap() as usize & 0xFFF) != 0x000 {
            return 2
        }

        if !self.is_virtually_allocated(ptr) {
            return 3
        }
 
        let pml4t_index: usize = (ptr.unwrap() as usize >> 39) & 0x1FF;
        let pdpt_index:  usize = (ptr.unwrap() as usize >> 30) & 0x1FF;
        let pdt_index:   usize = (ptr.unwrap() as usize >> 21) & 0x1FF;
        let pt_index:    usize = (ptr.unwrap() as usize >> 12) & 0x1FF;

        self.ptes[pml4t_index][pdpt_index][pdt_index][pt_index] = 0x00000000;
        return 0
    }

    fn deallocate_page(&mut self, ptr: Option<*const ()>) -> u8 {
        if ptr.is_none() {
            return 1
        }

        if ptr.unwrap() as usize & 0xFFF != 0x000 {
            return 2
        }

        if !self.is_virtually_allocated(ptr) {
            return 3
        }

        let b: bool = self.unmap_virt_addr(ptr) != 0;

        if self.unmap_phys_addr(self.as_phys_addr(ptr)) != 0 {
            if b == true {
                return 4
            } else {
                return 5
            }
        }

        return 0
    }

    fn deallocate_pages(&mut self, ptr: Option<*const ()>, num_pages: usize) -> u8 {
        if ptr.is_none() {
            return 1
        }

        if ptr.unwrap() as usize & 0xFFF != 0x000 {
            return 2
        }

        let mut b: bool = false;
        for i in 0..num_pages {
            if self.deallocate_page(Some((ptr.unwrap() as usize + i * PAGE_SIZE) as *const ())) != 0 {
                b = true;
            }
        }
        return match b {
            true => 3,
            false => 0
        };
    }

    fn find_free_virt_addr(&self) -> Option<*const ()> {
        let mut addr: usize = match self.last_mapped_virt_addr.is_none() {
            true => 16 * 1024 * 1024,
            false => self.last_mapped_virt_addr.unwrap() as usize
        };

        let mut b: bool = false;
        loop {
            if addr > VMEM_MAX - PAGE_SIZE {
                break;
            }

            if self.is_virtually_allocated(Some(addr as *const ())) {
                b = true;
                break;
            }
            addr += PAGE_SIZE;
        }
        if b == false {
            return None
        }
        Some(addr as *const ())
    }

    fn find_free_phys_addr(&self) -> Option<*const ()> {
        let mut addr: usize = match self.last_mapped_virt_addr.is_none() {
            true => 16 * 1024 * 1024,
            false => self.last_mapped_phys_addr.unwrap() as usize
        };

        let mut b: bool = false;
        loop {
            if addr > VMEM_MAX - PAGE_SIZE {
                break;
            }

            if self.is_physically_allocated(Some(addr as *const ())) {
                b = true;
                break;
            }
            addr += PAGE_SIZE;
        }
        if b == false {
            return None
        }
        Some(addr as *const ())
    }

    pub fn as_phys_addr(&self, ptr: Option<*const ()>) -> Option<*const ()> {
        if ptr.is_none() {
            return None
        }

        let pml4t_index: usize = (ptr.unwrap() as usize >> 39) & 0x1FF;
        let pdpt_index:  usize = (ptr.unwrap() as usize >> 30) & 0x1FF;
        let pdt_index:   usize = (ptr.unwrap() as usize >> 21) & 0x1FF;
        let pt_index:    usize = (ptr.unwrap() as usize >> 12) & 0x1FF;
        let offset:      usize =  ptr.unwrap() as usize & 0xFFF;

        if self.pml4tes[pml4t_index] & 0x01 == 0x00000000 {
            return None
        }

        if self.pdptes[pml4t_index][pdpt_index] & 0x01 == 0x00000000 {
            return None
        }

        if self.pdtes[pml4t_index][pdpt_index][pdt_index] & 0x01 == 0x00000000 {
            return None
        }
    
        Some(((self.ptes[pml4t_index][pdpt_index][pdt_index][pt_index] & !0xFFF) | offset as u64) as *const ())
    }

}