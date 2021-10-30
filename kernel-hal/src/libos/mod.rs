mod drivers;
mod dummy;
mod mock_mem;

pub mod boot;
pub mod config;
pub mod cpu;
pub mod mem;
pub mod thread;
pub mod timer;
pub mod vdso;
pub mod vm;

#[path = "special.rs"]
#[doc(cfg(feature = "libos"))]
pub mod libos;

pub use super::hal_fn::{context, interrupt, rand};

hal_fn_impl_default!(context, interrupt, rand, super::hal_fn::console);

#[cfg(target_os = "macos")]
mod macos;

/// Non-SMP initialization.
#[doc(cfg(any(feature = "libos", not(feature = "smp"))))]
pub fn init() {
    drivers::init_early();
    boot::primary_init();
}
