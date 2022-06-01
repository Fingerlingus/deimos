pub const VMEM_MAX:  usize = 8 * 1024 * 1024 * 1024;
pub const MAX_PAGES: usize = VMEM_MAX / 4096;
pub const PAGE_SIZE: usize = 4096;
pub const PT_SIZE:   usize = 512 * PAGE_SIZE;