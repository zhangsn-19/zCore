use core::fmt::{Error, Write};
use spin::Mutex;

use crate::{mem::phys_to_virt, utils::init_once::InitOnce, PhysAddr, VirtAddr};

pub(super) struct Uart {
    base_address: VirtAddr,
}

pub(super) static UART: InitOnce<Mutex<Uart>> = InitOnce::new();

// 结构体Uart的实现块
impl Uart {
    pub fn new(base_address: VirtAddr) -> Self {
        Uart { base_address }
    }

    #[cfg(not(feature = "board_d1"))]
    pub fn simple_init(&mut self) {
        let ptr = self.base_address as *mut u8;
        unsafe {
            // Enable FIFO; (base + 2)
            ptr.add(2).write_volatile(0xC7);

            // MODEM Ctrl; (base + 4)
            ptr.add(4).write_volatile(0x0B);

            // Enable interrupts; (base + 1)
            ptr.add(1).write_volatile(0x01);
        }
    }

    #[cfg(feature = "board_d1")]
    pub fn simple_init(&mut self) {
        let ptr = self.base_address as *mut u32;
        unsafe {
            // Enable FIFO; (base + 2)
            ptr.add(2).write_volatile(0x7);

            // MODEM Ctrl; (base + 4)
            ptr.add(4).write_volatile(0x3);

            //D1 ALLWINNER的uart中断使能
            // D1 UART_IER offset = 0x4
            //
            // Enable interrupts; (base + 1)
            ptr.add(1).write_volatile(0x1);
        }
    }

    pub fn get(&mut self) -> Option<u8> {
        #[cfg(not(feature = "board_d1"))]
        let ptr = self.base_address as *mut u8;
        #[cfg(feature = "board_d1")]
        let ptr = self.base_address as *mut u32;

        unsafe {
            //查看LSR的DR位为1则有数据
            if ptr.add(5).read_volatile() & 0b1 == 0 {
                None
            } else {
                Some(ptr.add(0).read_volatile() as u8)
            }
        }
    }

    pub fn put(&mut self, c: u8) {
        let ptr = self.base_address as *mut u8;
        unsafe {
            //此时transmitter empty
            ptr.add(0).write_volatile(c);
        }
    }
}

// 需要实现的write_str()重要函数
impl Write for Uart {
    fn write_str(&mut self, out: &str) -> Result<(), Error> {
        for c in out.bytes() {
            self.put(c);
        }
        Ok(())
    }
}

pub(super) fn handle_interrupt() {
    if let Some(uart) = UART.try_get() {
        if let Some(c) = uart.lock().get() {
            //CONSOLE
            crate::serial::serial_put(c);
        }
    }
}

pub(super) fn init(base_paddr: PhysAddr) {
    UART.init(|| {
        let mut uart = Uart::new(phys_to_virt(base_paddr));
        uart.simple_init();
        Mutex::new(uart)
    });
}
