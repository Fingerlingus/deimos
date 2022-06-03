use core::mem::size_of;

use crate::constants::*;
use crate::asm_wrappers::lcr3;

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

#[repr(align(4096), C)]
struct aligned_array<T, const N: usize> {
    pub arr: [T; N]
}

#[repr(align(4096), C)]
pub struct Pager {
    //ptes: aligned_array<aligned_array<aligned_array<aligned_array<u64, 512>, NUM_PDTES>, NUM_PDPTES, NUM_PML4TES>,
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
    // Return new Pager with fields all zeroed
    pub const fn new() -> Self {
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

    // Identity-map first 16MB, leave rest to be allocated on demand
    pub fn init(&mut self) {
        // Identity-map first PML4T and PDPT entries
        self.pml4tes[0]   = (&self.pdptes[0][0]   as *const u64 as u64 & !0xFFF) | 0x03;
        self.pdptes[0][0] = (&self.pdtes[0][0][0] as *const u64 as u64 & !0xFFF) | 0x03;

        // Identity-map first 8 PDT entires (each 2MB)
        for i in 0..8 {
            self.pdtes[0][0][i] = (&self.ptes[0][0][i][0] as *const u64 as u64 & !0xFFF) | 0x03;
            // ...and identity map each respective PT
            for j in 0..512 {
                self.ptes[0][0][i][j] = ((((i * PT_SIZE) + (j * PAGE_SIZE)) & !0xFFF) | 0x03) as u64;
            }
        }
    }

    // Check if virtual address lies within allocated virtual memory
    pub fn is_virtually_allocated(&self, ptr: Option<*const ()>) -> bool {
        if ptr.is_none() {
            return true
        }

        let pml4t_index: usize = (ptr.unwrap() as usize >> 39) & 0x1FF;
        let pdpt_index:  usize = (ptr.unwrap() as usize >> 30) & 0x1FF;
        let pdt_index:   usize = (ptr.unwrap() as usize >> 21) & 0x1FF;
        let pt_index:    usize = (ptr.unwrap() as usize >> 12) & 0x1FF;

        // Bail if entries aren't marked present (can't be allocated)
        if self.pml4tes[pml4t_index] & 0x01 == 0x00000000 {
            return false
        }

        if self.pdptes[pml4t_index][pdpt_index] & 0x01 == 0x00000000 {
            return false
        }

        if self.pdtes[pml4t_index][pdpt_index][pdt_index] & 0x01 == 0x00000000 {
            return false
        }
    
        // Check if entry is present
        self.ptes[pml4t_index][pdpt_index][pdt_index][pt_index] & 0x01 != 0x00000000
    }

    // Check if physical address lies within allocated physical memory
    pub fn is_physically_allocated(&self, ptr: Option<*const ()>) -> bool {
        if ptr.is_none() {
            return true
        }

        if ptr == self.last_mapped_phys_addr {
            return true
        }

        // Addresses less than 16MB are guaranteed to be indentity-mapped
        if ptr.unwrap() < ((16 * 1024 * 1024) as *const ()) {
            return true 
        }

        let temp   = (ptr.unwrap() as usize & !0xFFF) / PAGE_SIZE;
        let index  = temp / 64;
        let offset = temp % 64;

        self.phys_addr_map[index] & (1 << offset) != 0x00000000
    }

    // Get last mapped physical address
    pub const fn last_mapped_phys_addr(&self) -> Option<*const ()> {
        self.last_mapped_phys_addr
    }

    // Get last mapped virtual address
    pub const fn last_mapped_virt_addr(&self) -> Option<*const ()> {
        self.last_mapped_phys_addr
    }

    // Get allocated pages
    pub const fn allocated_pages(&self) -> &[Option<*const ()>; MAX_PAGES] {
        &self.allocated_pages
    }

    // Get PML4 Table
    pub const fn pml4t(&self) -> &[u64; 512] {
        &self.pml4tes
    }

    // Activate page table
    pub unsafe fn activate(&self) {
        lcr3(&self.pml4tes as *const u64 as usize);
    }

    // Map provided physical address to provided virtual address (neither can be None)
    pub unsafe fn map_phys_addr_to_virt_addr(&mut self, paddr: Option<*const ()>, vaddr: Option<*const ()>) -> Option<*const ()> {
        if vaddr.is_none() {
            return None
        }

        if paddr.is_none() {
            return None
        }

        // Can't map unaligned address
        if (vaddr.unwrap() as usize & 0xFFF) != 0x000 {
            return None
        }

        // Can't map unaligned address
        if (paddr.unwrap() as usize & 0xFFF) != 0x000 {
            return None
        }

        // Can't map address if address is already allocated
        if self.is_virtually_allocated(vaddr) {
            return None
        }
 
        let pml4t_index: usize = (vaddr.unwrap() as usize >> 39) & 0x1FF;
        let pdpt_index:  usize = (vaddr.unwrap() as usize >> 30) & 0x1FF;
        let pdt_index:   usize = (vaddr.unwrap() as usize >> 21) & 0x1FF;
        let pt_index:    usize = (vaddr.unwrap() as usize >> 12) & 0x1FF;

        // Mark entries as present if not already marked
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

        // Perform the actual allocation
        if (self.ptes[pml4t_index][pdpt_index][pdt_index][pt_index] & 0x03) != 0x03 {
            self.ptes[pml4t_index][pdpt_index][pdt_index][pt_index] = paddr.unwrap() as u64 | 0x03;
        }
 
        // and mark page as allocated in physical address map
        let temp   = (paddr.unwrap() as usize & !0xFFF) / PAGE_SIZE;
        let index  = temp / 64;
        let offset = temp % 64; 

        self.phys_addr_map[index] |= 1 << offset;
        return vaddr
    }

    // Allocate page either at provided virtual address, or at a random virtual address if none provided
    pub unsafe fn allocate_page(&mut self, ptr: Option<*const ()>) -> Option<*const ()> {
        let vaddr: *const () = match ptr {
            // If no virtual address is provided, find a free one
            None => match self.find_free_virtual_page() {
                // If no free virtual address can be foud, bail
                None => return None,
                Some(p) => p
            },
            // Else ensure address is aligned
            Some(p) => match p as usize & 0xFFF == 0 {
                false => return None,
                // And ensure it's not already allocated
                true => match self.is_virtually_allocated(ptr) {
                    false => p,
                    true => return None
                }
            }
        };

        // Find free physical page to map virtual page to
        let paddr = match self.find_free_physical_page() {
            // Bail if none can be found
            None => return None,
            Some(p) => p
        };

        // Map physical address to virtual address
        return match self.map_phys_addr_to_virt_addr(Some(paddr), Some(vaddr)) {
            Some(_p) => {
                // If successful, update last_mapped_phys_addr and last_mapped_virt_addr accordingly
                self.last_mapped_phys_addr = Some(paddr);
                self.last_mapped_virt_addr = Some(vaddr);
                // And return virtual address
                Some(paddr)
            },
            // Else bail
            None => None
        }
    }

    // Allocate virtually-contiguous pages either at provided virtual address, or random address if none provided
    pub unsafe fn allocate_pages(&mut self, ptr: Option<*const ()>, num_pages: usize) -> Option<*const ()> {
        // Alias to allocate_virtually_contiguous_pages()
        self.allocate_virtually_contiguous_pages(ptr, num_pages)
    }

    // Allocate virtually-contiguous pages either at provided virtual address, or random address if none provided
    pub unsafe fn allocate_virtually_contiguous_pages(&mut self, ptr: Option<*const ()>, num_pages: usize) -> Option<*const ()> {
        // Can't allocate zero pages
        if num_pages == 0 {
            return None
        }

        let p = match ptr {
            // If no virtual page is provided, find a free one
            None => match self.find_free_contiguous_virtual_pages(num_pages) {
                // If none can be found, bail
                None => return None,
                Some(p) => p
            },
            // If one is provided, ensure it's properly aligned
            Some(p) => match (p as usize & !0xFFF) != 0 {
                // If not, bail
                true => return None,
                false => p
            }
        };

        // Map each page
        for i in 0..num_pages {
            // Find random free physical page to map virtual page to
            let paddr = match self.find_free_physical_page() {
                // Bail if none can be found
                None => return None,
                Some(p) => p
            };

            // Perform the actual mapping
            match self.map_phys_addr_to_virt_addr(Some(paddr), Some((p as usize + i * PAGE_SIZE) as *const ())) {
                Some(_p) => { },
                // Bail if mapping fails
                None => return None
            };
        }
        // Return virtual address
        return Some(p)
    }

    // Allocate virtually- and physically-contiguous pages either at provided addresses, or random address if none provided respectively
    pub unsafe fn allocate_physically_contiguous_pages(&mut self, paddr: Option<*const ()>, vaddr: Option<*const ()>, num_pages: usize) -> Option<*const ()> {
        let p = match paddr {
            // If no physical address is provided, find one
            None => match self.find_free_contiguous_physical_pages(num_pages) {
                // Bail if none can be found
                None => return None,
                Some(p) => p
            },
            // Else ensure address is correctly aligned
            Some(p) => match (p as usize & 0xFFF) != 0x000 {
                // Bail if not
                false => return None,
                true => p
            }
        };

        let v = match vaddr {
            // If no physical address is provided, find one
            None => match self.find_free_contiguous_virtual_pages(num_pages) {
                // Bail if none can be found
                None => return None,
                Some(p) => p
            },
            // Else ensure address is correctly aligned
            Some(p) => match (p as usize & 0xFFF) != 0x000 {
                // Bail if not
                false => return None,
                true => p
            }
        };

        match num_pages {
            // Can't map zero pages
            0 => return None,
            // Map single page
            1 => return match self.map_phys_addr_to_virt_addr(Some(p), Some(v)) {
                Some(_p) => Some(v),
                None => None
            },
            // Map pages
            _ => {
                for i in 0..num_pages {
                    // Calculate pointers
                    let temp_paddr = (paddr.unwrap() as usize + i * PAGE_SIZE) as *const ();
                    let temp_vaddr = (vaddr.unwrap() as usize + i * PAGE_SIZE) as *const ();
                    
                    // Perform actual mapping
                    match self.map_phys_addr_to_virt_addr(Some(temp_paddr), Some(temp_vaddr)) {
                        Some(_p) => { },
                        // Bail if mapping fails
                        None => return None
                    }
                }
                None
            }
        }
    }

    // Unmap physical address
    unsafe fn unmap_phys_addr(&mut self, ptr: Option<*const ()>) -> Option<*const ()> {
        // Can't unmap nonexistent pointer
        if ptr.is_none() {
            return None
        }

        // Can't unmap unaligned pointer
        if (ptr.unwrap() as usize & 0xFFF) != 0x000 {
            return None
        }

        // Can't unmap pointer that's not allocated
        if !self.is_physically_allocated(ptr) {
            return None
        }

        let temp = ptr.unwrap() as usize / PAGE_SIZE;
        let index = temp / 64;
        let offset = temp % 64;

        self.phys_addr_map[index] &= !(1 << offset);
        return ptr
    }

    // Unmap virtual address
    unsafe fn unmap_virt_addr(&mut self, ptr: Option<*const ()>) -> Option<*const ()> {
        // Can't unmap nonexistent pointer
        if ptr.is_none() {
            return None
        }

        // Can't unmap unaligned pointer
        if (ptr.unwrap() as usize & 0xFFF) != 0x000 {
            return None
        }

        // Can't unmap pointer not already allocated
        if !self.is_virtually_allocated(ptr) {
            return None
        }
 
        let pml4t_index: usize = (ptr.unwrap() as usize >> 39) & 0x1FF;
        let pdpt_index:  usize = (ptr.unwrap() as usize >> 30) & 0x1FF;
        let pdt_index:   usize = (ptr.unwrap() as usize >> 21) & 0x1FF;
        let pt_index:    usize = (ptr.unwrap() as usize >> 12) & 0x1FF;

        // Perform actual unmapping
        self.ptes[pml4t_index][pdpt_index][pdt_index][pt_index] = 0x00000000;
        return ptr
    }

    // Deallocate page at provided virtal address
    pub unsafe fn deallocate_page(&mut self, ptr: Option<*const ()>) -> Option<*const ()> {
        // Can't deallocate nonexistent page
        if ptr.is_none() {
            return None
        }

        // Can't deallocate unaligned page
        if ptr.unwrap() as usize & 0xFFF != 0x000 {
            return None
        }

        // Can't deallocate page not already allocated
        if !self.is_virtually_allocated(ptr) {
            return None
        }

        // Perform unmapping of virtual address and keep result
        let b: bool = self.unmap_virt_addr(ptr) == None;

        // Perform unmapping of physical address, and return None if fail
        if self.unmap_phys_addr(self.as_phys_addr(ptr)).is_none() {
            return None
        }

        // Return none if virtual unmap failed, ptr if neither failed
        return match b {
            true => None,
            false => ptr
        }
    }

    // Deallocate virtually-contiguous pages starting at proved virtual address
    pub unsafe fn deallocate_pages(&mut self, ptr: Option<*const ()>, num_pages: usize) -> Option<*const ()> {
        // Can't deallocate nonexistent pointer
        if ptr.is_none() {
            return None
        }

        // Can't deallocate unaligned pointer
        if ptr.unwrap() as usize & 0xFFF != 0x000 {
            return None
        }

        let mut b: bool = false;
        // deallocate pages, note if any failed but still try the rest
        for i in 0..num_pages {
            if self.deallocate_page(Some((ptr.unwrap() as usize + i * PAGE_SIZE) as *const ())).is_none() {
                b = true;
            }
        }

        // return None if any failed, ptr if none failed
        return match b {
            true => None,
            false => ptr
        };
    }

    // Find free virtual page
    pub fn find_free_virtual_page(&self) -> Option<*const ()> {
        let mut addr: usize = match self.last_mapped_virt_addr.is_none() {
            // If no virtual address has been mapped yet, use 16MB
            true => 16 * 1024 * 1024,
            false => self.last_mapped_virt_addr.unwrap() as usize
        };

        let mut b: bool = false;
        // Find free page
        loop {
            if addr > VMEM_MAX - PAGE_SIZE {
                break;
            }

            if !self.is_virtually_allocated(Some(addr as *const ())) {
                b = true;
                break;
            }
            addr += PAGE_SIZE;
        }
        // Return None if none can be found
        if b == false {
            return None
        }
        // Else return pointer
        Some(addr as *const ())
    }

    // Find free physical page -- Same functioning as above
    pub fn find_free_physical_page(&self) -> Option<*const ()> {
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

    // Find range of free contiguous virtual pages
    pub fn find_free_contiguous_virtual_pages(&self, num_pages: usize) -> Option<*const ()> {
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
                return Some(p)
            }
            p = (p as usize + PAGE_SIZE) as *const ();
        }
        None
    }

    // Find range of free contiguous physical pages
    pub fn find_free_contiguous_physical_pages(&self, num_pages: usize) -> Option<*const ()> {
        let mut mask: u64 = 0;
        for i in 0..num_pages {
            mask |= 1 << i;
        }

        for i in 0..NUM_PHYS_ADDR_MAP_ENTRIES {
            for j in 0..64 {
                if (self.phys_addr_map[i] >> j) & mask == mask {
                    return Some(((i * 64 * PAGE_SIZE) + (j * PAGE_SIZE)) as *const ())
                }
            }
        }
        return None
    }

    // Get physical address from provided virtual address
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