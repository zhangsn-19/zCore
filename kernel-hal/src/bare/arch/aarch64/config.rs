//! Kernel configuration.
use crate::PAGE_SIZE;

/// Kernel configuration passed by kernel when calls [`crate::primary_init_early()`].
#[derive(Debug)]
pub struct KernelConfig {
    pub rt_services_addr: usize,
    pub rsdp_addr: usize,
    pub phys_to_virt_offset: usize,
}

pub const PHYS_MEMORY_BASE: usize = 0x4000_0000;
pub const UART_BASE: usize = 0x0900_0000;
pub const UART_SIZE: usize = 0x1000;
pub const GIC_BASE: usize = 0x0800_0000;
pub const GIC_SIZE: usize = 0x2_0000;
pub const VIRTIO_BASE: usize = 0x0a00_0000;
pub const VIRTIO_SIZE: usize = 0x100;
pub const PA_1TB_BITS: usize = 40;
pub const PHYS_ADDR_MAX: usize = (1 << PA_1TB_BITS) - 1;
pub const PHYS_ADDR_MASK: usize = PHYS_ADDR_MAX & !(PAGE_SIZE - 1);
pub const PHYS_MEMORY_END: usize = PHYS_MEMORY_BASE + 100 * 1024 * 1024;
pub const USER_TABLE_FLAG: usize = 0xabcd_0000_0000_0000;
