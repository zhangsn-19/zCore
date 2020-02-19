//! Zircon syscall implementations

#![no_std]
#![deny(warnings, unsafe_code, unused_must_use, unreachable_patterns)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate log;

use {
    alloc::sync::Arc, alloc::vec::Vec, kernel_hal::user::*, zircon_object::object::*,
    zircon_object::task::Thread,
};

mod channel;
mod consts;
mod debug;
mod debuglog;
mod handle;
mod object;
mod task;
mod vmar;
mod vmo;

pub use consts::SyscallType;

pub struct Syscall {
    pub thread: Arc<Thread>,
    pub exit: bool,
}

impl Syscall {
    pub fn syscall(&mut self, sys_type: SyscallType, args: [usize; 8]) -> isize {
        info!("{:?}=> args={:x?}", sys_type, args);
        let [a0, a1, a2, a3, a4, a5, a6, a7] = args;
        let ret = match sys_type {
            SyscallType::HANDLE_DUPLICATE => self.sys_handle_duplicate(a0 as _, a1 as _, a2.into()),
            SyscallType::HANDLE_CLOSE => self.sys_handle_close(a0 as _),
            SyscallType::HANDLE_CLOSE_MANY => self.sys_handle_close_many(a0.into(), a1 as _),
            SyscallType::CHANNEL_READ => self.sys_channel_read(
                a0 as _,
                a1 as _,
                a2.into(),
                a3.into(),
                a4 as _,
                a5 as _,
                a6.into(),
                a7.into(),
            ),
            SyscallType::OBJECT_GET_PROPERTY => {
                self.sys_object_get_property(a0 as _, a1 as _, a2.into(), a3 as _)
            }
            SyscallType::OBJECT_SET_PROPERTY => {
                self.sys_object_set_property(a0 as _, a1 as _, a2.into(), a3 as _)
            }
            SyscallType::DEBUG_WRITE => self.sys_debug_write(a0.into(), a1 as _),
            SyscallType::PROCESS_CREATE => {
                self.sys_process_create(a0 as _, a1.into(), a2 as _, a3 as _, a4.into(), a5.into())
            }
            SyscallType::PROCESS_EXIT => self.sys_process_exit(a0 as _),
            SyscallType::DEBUGLOG_CREATE => self.sys_debuglog_create(a0 as _, a1 as _, a2.into()),
            SyscallType::DEBUGLOG_WRITE => {
                self.sys_debuglog_write(a0 as _, a1 as _, a2.into(), a3 as _)
            }
            SyscallType::VMO_CREATE => self.sys_vmo_create(a0 as _, a1 as _, a2.into()),
            SyscallType::VMO_READ => self.sys_vmo_read(a0 as _, a1.into(), a2 as _, a3 as _),
            SyscallType::VMAR_ALLOCATE => {
                self.sys_vmar_allocate(a0 as _, a1 as _, a2 as _, a3 as _, a4.into(), a5.into())
            }
            _ => {
                warn!("syscall unimplemented");
                Err(ZxError::NOT_SUPPORTED)
            }
        };
        info!("{:?}<= {:?}", sys_type, ret);
        match ret {
            Ok(_) => 0,
            Err(err) => err as isize,
        }
    }
}
