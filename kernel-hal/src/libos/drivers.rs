use alloc::sync::Arc;

use zcore_drivers::mock::uart::MockUart;
use zcore_drivers::{scheme::Scheme, Device};

cfg_if! {
    if #[cfg(feature = "graphic")] {
        use crate::{addr::page_count, mem::PhysFrame};
        use alloc::vec::Vec;
        use zcore_drivers::prelude::ColorFormat;

        const FB_WIDTH: u32 = 1280;
        const FB_HEIGHT: u32 = 720;
        const FB_FORMAT: ColorFormat = ColorFormat::ARGB8888;

        lazy_static! {
            /// Put the framebuffer into the physical frames pool to support mmap.
            static ref FB_FRAMES: Vec<PhysFrame> = PhysFrame::new_contiguous(
                page_count((FB_WIDTH * FB_HEIGHT * FB_FORMAT.bytes() as u32) as usize),
                0,
            );
        }
    }
}

pub(super) fn init() {
    let uart = Arc::new(MockUart::new());
    crate::drivers::add_device(Device::Uart(uart.clone()));
    MockUart::start_irq_service(move || uart.handle_irq(0));

    #[cfg(feature = "graphic")]
    {
        use zcore_drivers::mock::display::MockDisplay;
        let display = Arc::new(unsafe {
            MockDisplay::from_raw_parts(FB_WIDTH, FB_HEIGHT, FB_FORMAT, FB_FRAMES[0].as_mut_ptr())
        });
        crate::drivers::add_device(Device::Display(display.clone()));
        crate::console::init_graphic_console(display);
    }
}
