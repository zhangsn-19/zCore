#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(llvm_asm)]
#![feature(panic_info_message)]
#![deny(unused_must_use)]
#![feature(global_asm)]
/*
#![deny(warnings)] // comment this on develop
*/

extern crate alloc;
#[macro_use]
extern crate log;
extern crate rlibc;

#[cfg(target_arch = "x86_64")]
extern crate rlibc_opt; //Only for x86_64

#[cfg(target_arch = "riscv64")]
mod consts;

#[macro_use]
mod logging;
mod lang;
mod memory;

#[cfg(target_arch = "x86_64")]
use rboot::BootInfo;

#[cfg(target_arch = "riscv64")]
use kernel_hal_bare::{BootInfo, GraphicInfo};

use alloc::vec::Vec;
pub use memory::{hal_frame_alloc, hal_frame_dealloc, hal_pt_map_kernel, phys_to_virt, write_readonly_test, execute_unexecutable_test, read_invalid_test};

#[cfg(target_arch = "riscv64")]
global_asm!(include_str!("arch/riscv/boot/entry64.asm"));

#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn _start(boot_info: &BootInfo) -> ! {
    logging::init(get_log_level(boot_info.cmdline));
    memory::init_heap();
    memory::init_frame_allocator(boot_info);

    #[cfg(feature = "graphic")]
    init_framebuffer(boot_info);

    info!("{:#x?}", boot_info);

    #[cfg(target_arch = "x86_64")]
    kernel_hal_bare::init(kernel_hal_bare::Config {
        acpi_rsdp: boot_info.acpi2_rsdp_addr,
        smbios: boot_info.smbios_addr,
        ap_fn: run,
    });

    #[cfg(target_arch = "riscv64")]
    kernel_hal_bare::init(kernel_hal_bare::Config {
        mconfig: 0,
    });

    let ramfs_data = unsafe {
        core::slice::from_raw_parts_mut(
            (boot_info.initramfs_addr + boot_info.physical_memory_offset) as *mut u8,
            boot_info.initramfs_size as usize,
        )
    };
    main(ramfs_data, boot_info.cmdline);
    unreachable!();
}

#[cfg(feature = "zircon")]
fn main(ramfs_data: &[u8], cmdline: &str) {
    use zircon_loader::{run_userboot, Images};
    let images = Images::<&[u8]> {
        userboot: include_bytes!("../../prebuilt/zircon/x64/userboot.so"),
        vdso: include_bytes!("../../prebuilt/zircon/x64/libzircon.so"),
        zbi: ramfs_data,
    };
    let _proc = run_userboot(&images, cmdline);
    run();
}

#[cfg(target_arch = "riscv64")]
#[no_mangle]
pub extern "C" fn rust_main(hartid: usize, device_tree_paddr: usize) -> ! {
    let device_tree_vaddr = phys_to_virt(device_tree_paddr);

    let boot_info = BootInfo {
        memory_map: Vec::new(),
        physical_memory_offset: 0,
        graphic_info: GraphicInfo{mode: 0, fb_addr: 0, fb_size: 0},
        acpi2_rsdp_addr: 0,
        smbios_addr: 0,
        initramfs_addr: 0,
        initramfs_size: 0,
        cmdline: "LOG=trace:TERM=xterm-256color:console.shell=true:virtcon.disable=true",
    };

    unsafe {
        memory::clear_bss();
    }

    logging::init(get_log_level(boot_info.cmdline));
    warn!("rust_main(), After logging init\n\n");
    memory::init_heap();
    memory::init_frame_allocator(&boot_info);
    memory::remap_the_kernel(device_tree_vaddr);

    #[cfg(feature = "graphic")]
    init_framebuffer(boot_info);

    info!("{:#x?}", boot_info);

    kernel_hal_bare::init(kernel_hal_bare::Config {
        mconfig: 0,
    });

    // memory test
    //write_readonly_test();
    //execute_unexecutable_test();
    //read_invalid_test();


    loop {} //remove me

    // 挂载virtio-blk-device rootfs
    //
    let ramfs_data = unsafe {
        core::slice::from_raw_parts_mut(
            (boot_info.initramfs_addr + boot_info.physical_memory_offset) as *mut u8,
            boot_info.initramfs_size as usize,
        )
    };

    main(ramfs_data, boot_info.cmdline);
    unreachable!();
}

#[cfg(feature = "linux")]
fn main(ramfs_data: &'static mut [u8], _cmdline: &str) {
    use alloc::boxed::Box;
    use alloc::sync::Arc;
    use alloc::vec;
    use alloc::string::String;

    /*
    use linux_object::fs::MemBuf;
    use linux_object::fs::STDIN;
    */

    kernel_hal_bare::serial_set_callback(Box::new({
        move || {
            let mut buffer = [0; 255];
            let len = kernel_hal_bare::serial_read(&mut buffer);
            for c in &buffer[..len] {
                /*
                STDIN.push((*c).into());
                */
                kernel_hal_bare::serial_write(alloc::format!("{}", *c as char).as_str());
            }
            false
        }
    }));

    let args: Vec<String> = vec!["/bin/busybox".into(), "sh".into()];
    let envs: Vec<String> = vec!["PATH=/usr/sbin:/usr/bin:/sbin:/bin:/usr/x86_64-alpine-linux-musl/bin".into()];

    /*
    let device = Arc::new(MemBuf::new(ramfs_data));
    let rootfs = rcore_fs_sfs::SimpleFileSystem::open(device).unwrap();
    let _proc = linux_loader::run(args, envs, rootfs);
    run();
    */
}

#[cfg(target_arch = "x86_64")]
fn run() -> ! {
    loop {
        executor::run_until_idle();
        x86_64::instructions::interrupts::enable_and_hlt();
        x86_64::instructions::interrupts::disable();
    }
}

fn get_log_level(cmdline: &str) -> &str {
    for opt in cmdline.split(':') {
        // parse 'key=value'
        let mut iter = opt.trim().splitn(2, '=');
        let key = iter.next().expect("failed to parse key");
        let value = iter.next().expect("failed to parse value");
        if key == "LOG" {
            return value;
        }
    }
    ""
}

#[cfg(feature = "graphic")]
fn init_framebuffer(boot_info: &BootInfo) {
    let (width, height) = boot_info.graphic_info.mode.resolution();
    let fb_addr = boot_info.graphic_info.fb_addr as usize;
    kernel_hal_bare::init_framebuffer(width as u32, height as u32, fb_addr);
}
