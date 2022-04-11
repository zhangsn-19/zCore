use super::*;
use bitflags::bitflags;
use core::time::Duration;
use futures::pin_mut;
use kernel_hal::timer::timer_now;
use linux_object::time::*;
use zircon_object::task::ThreadState;

impl Syscall<'_> {
    #[cfg(target_arch = "x86_64")]
    /// set architecture-specific thread state
    /// for x86_64 currently
    pub fn sys_arch_prctl(&mut self, code: i32, addr: usize) -> SysResult {
        const ARCH_SET_FS: i32 = 0x1002;
        match code {
            ARCH_SET_FS => {
                info!("sys_arch_prctl: set FSBASE to {:#x}", addr);
                self.thread.with_context(|ctx| {
                    ctx.set_field(kernel_hal::context::UserContextField::ThreadPointer, addr)
                })?;
                Ok(0)
            }
            _ => Err(LxError::EINVAL),
        }
    }

    /// get name and information about current kernel
    pub fn sys_uname(&self, buf: UserOutPtr<u8>) -> SysResult {
        info!("uname: buf={:?}", buf);

        let release = alloc::string::String::from(concat!(env!("CARGO_PKG_VERSION"), "-zcore"));
        #[cfg(not(target_os = "none"))]
        let release = release + "-libos";

        let vdso_const = kernel_hal::vdso::vdso_constants();

        let arch = if cfg!(target_arch = "x86_64") {
            "x86_64"
        } else if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else if cfg!(target_arch = "riscv64") {
            "riscv64"
        } else {
            "unknown"
        };

        let strings = [
            "Linux",                            // sysname
            "zcore",                            // nodename
            release.as_str(),                   // release
            vdso_const.version_string.as_str(), // version
            arch,                               // machine
            "rcore-os",                         // domainname
        ];

        for (i, &s) in strings.iter().enumerate() {
            const OFFSET: usize = 65;
            buf.add(i * OFFSET).write_cstring(s)?;
        }
        Ok(0)
    }

    /// provides a simple way of getting overall system statistics
    pub fn sys_sysinfo(&mut self, mut sys_info: UserOutPtr<SysInfo>) -> SysResult {
        let sysinfo = SysInfo::default();
        sys_info.write(sysinfo)?;
        Ok(0)
    }

    /// provides a method for waiting until a certain condition becomes true.
    /// - `uaddr` - points to the futex word.
    /// - `op` -  the operation to perform on the futex
    /// - `val` -  a value whose meaning and purpose depends on op
    /// - `timeout_addr` - provides a timeout for the attempt or acts as val2 when op is REQUEUE
    /// - `uaddr2` - when op is REQUEUE, points to the target futex
    /// - `_val3` - is not used
    pub async fn sys_futex(
        &self,
        uaddr: usize,
        op: u32,
        val: u32,
        timeout_addr: usize,
        uaddr2: usize,
        _val3: u32,
    ) -> SysResult {
        if let Err(_) = self.into_inout_userptr::<i32>(uaddr) {
            return Err(LxError::EINVAL);
        }
        let op = FutexFlags::from_bits_truncate(op);
        if !op.contains(FutexFlags::PRIVATE) {
            warn!("process-shared futex is unimplemented");
        }
        let mut val2 = 0;
        if op.contains(FutexFlags::REQUEUE) {
            if let Err(_) = self.into_inout_userptr::<i32>(uaddr2) {
                return Err(LxError::EINVAL);
            }
            val2 = timeout_addr;
        }
        let timeout = if op.contains(FutexFlags::WAKE) {
            self.into_inout_userptr::<TimeSpec>(0).unwrap()
        } else {
            let timeout_result = self.into_inout_userptr::<TimeSpec>(timeout_addr);
            match timeout_result {
                Ok(t) => t,
                Err(_e) => return Err(LxError::EACCES),
            }
        };
        warn!(
            "Futex uaddr: {:#x}, op: {:x}, val: {}, timeout_ptr: {:x?}, val2: {}",
            uaddr,
            op.bits(),
            val,
            timeout,
            val2,
        );
        let futex = self.linux_process().get_futex(uaddr);
        let op = op - FutexFlags::PRIVATE;
        match op {
            FutexFlags::WAIT => {
                let timeout = timeout.read_if_not_null()?;
                let duration: Duration = match timeout {
                    Some(t) => t.into(),
                    None => Duration::from_secs(0),
                };
                let into_lxerror = |e: ZxError| match e {
                    ZxError::BAD_STATE => LxError::EAGAIN,
                    e => e.into(),
                };
                let future = futex.wait(val, false, self.thread.id() as i32);
                let res = if duration.as_millis() == 0 {
                    future.await
                } else {
                    pin_mut!(future);
                    self.thread
                        .blocking_run(
                            future,
                            ThreadState::BlockedFutex,
                            timer_now() + duration,
                            None,
                        )
                        .await
                };
                match res {
                    Ok(_) => return Ok(0),
                    Err(e) => return Err(into_lxerror(e)),
                }
            }
            FutexFlags::WAKE => {
                let woken_up_count = futex.wake(val as usize);
                Ok(woken_up_count)
            }
            FutexFlags::LOCK_PI => {
                warn!("futex LOCK_PI is unimplemented");
                Ok(0)
            }
            FutexFlags::REQUEUE => {
                let requeue_futex = self.linux_process().get_futex(uaddr2);
                let into_lxerror = |e: ZxError| match e {
                    ZxError::BAD_STATE => LxError::EAGAIN,
                    e => e.into(),
                };
                let res = futex.requeue(0, val as usize, val2, &requeue_futex, None, false);
                match res {
                    Ok(_) => return Ok(0),
                    Err(e) => return Err(into_lxerror(e)),
                }
            }
            _ => {
                warn!("unsupported futex operation: {:?}", op);
                Err(LxError::ENOPROTOOPT)
            }
        }
    }

    /// Combines and extends the functionality of setrlimit() and getrlimit()
    pub fn sys_prlimit64(
        &mut self,
        pid: usize,
        resource: usize,
        new_limit: UserInPtr<RLimit>,
        mut old_limit: UserOutPtr<RLimit>,
    ) -> SysResult {
        info!(
            "prlimit64: pid: {}, resource: {}, new_limit: {:x?}, old_limit: {:x?}",
            pid, resource, new_limit, old_limit
        );
        let proc = self.linux_process();
        match resource {
            RLIMIT_STACK => {
                old_limit.write_if_not_null(RLimit {
                    cur: USER_STACK_SIZE as u64,
                    max: USER_STACK_SIZE as u64,
                })?;
                Ok(0)
            }
            RLIMIT_NOFILE => {
                let new_limit = new_limit.read_if_not_null()?;
                old_limit.write_if_not_null(proc.file_limit(new_limit))?;
                Ok(0)
            }
            RLIMIT_RSS | RLIMIT_AS => {
                old_limit.write_if_not_null(RLimit {
                    cur: 1024 * 1024 * 1024,
                    max: 1024 * 1024 * 1024,
                })?;
                Ok(0)
            }
            _ => Err(LxError::ENOSYS),
        }
    }

    #[allow(unsafe_code)]
    /// fills the buffer pointed to by `buf` with up to `buflen` random bytes.
    /// - `buf` - buffer that needed to fill
    /// - `buflen` - length of buffer
    /// - `flag` - a bit mask that can contain zero or more of the following values ORed together:
    ///   - GRND_RANDOM
    ///   - GRND_NONBLOCK
    /// - returns the number of bytes that were copied to the buffer buf.
    pub fn sys_getrandom(&mut self, mut buf: UserOutPtr<u8>, len: usize, flag: u32) -> SysResult {
        info!("getrandom: buf: {:?}, len: {:?}, flag {:?}", buf, len, flag);
        let mut buffer = vec![0u8; len];
        kernel_hal::rand::fill_random(&mut buffer);
        buf.write_array(&buffer[..len])?;
        Ok(len)
    }
}

bitflags! {
    /// for op argument in futex()
    struct FutexFlags: u32 {
        /// tests that the value at the futex word pointed
        /// to by the address uaddr still contains the expected value val,
        /// and if so, then sleeps waiting for a FUTEX_WAKE operation on the futex word.
        const WAIT      = 0;
        /// wakes at most val of the waiters that are waiting on the futex word at the address uaddr.
        const WAKE      = 1;
        /// wakes up a maximum of val waiters that are waiting on the futex at uaddr.  If there are more than val waiters, then the remaining waiters are removed from the wait queue of the source futex at uaddr and added to the wait queue of the target futex at uaddr2.  The val2 argument specifies an upper limit on the number of waiters that are requeued to the futex at uaddr2.
        const REQUEUE   = 3;
        /// (unsupported) is used after an attempt to acquire the lock via an atomic user-mode instruction failed.
        const LOCK_PI   = 6;
        /// (unsupported) is called when the user-space value at uaddr cannot be changed atomically from a TID (of the owner) to 0.
        const UNLOCK_PI = 7;
        /// can be employed with all futex operations, tells the kernel that the futex is process-private and not shared with another process
        const PRIVATE   = 0x80;
    }
}

const USER_STACK_SIZE: usize = 8 * 1024 * 1024; // 8 MB, the default config of Linux

const RLIMIT_STACK: usize = 3;
const RLIMIT_RSS: usize = 5;
const RLIMIT_NOFILE: usize = 7;
const RLIMIT_AS: usize = 9;

/// sysinfo() return information sturct
#[repr(C)]
#[derive(Debug, Default)]
pub struct SysInfo {
    /// Seconds since boot
    uptime: u64,
    /// 1, 5, and 15 minute load averages
    loads: [u64; 3],
    /// Total usable main memory size
    totalram: u64,
    /// Available memory size
    freeram: u64,
    /// Amount of shared memory
    sharedram: u64,
    /// Memory used by buffers
    bufferram: u64,
    /// Total swa Total swap space sizep space size
    totalswap: u64,
    /// swap space still available
    freeswap: u64,
    /// Number of current processes
    procs: u16,
    /// Total high memory size
    totalhigh: u64,
    /// Available high memory size
    freehigh: u64,
    /// Memory unit size in bytes
    mem_unit: u32,
}
