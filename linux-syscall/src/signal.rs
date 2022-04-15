//! Syscalls of signal
//!
//! - rt_sigaction
//! - rt_sigreturn
//! - rt_sigprocmask
//! - kill
//! - tkill
//! - sigaltstack

use super::*;
use linux_object::signal::{Signal, SignalAction, SignalStack, SignalStackFlags, Sigset};
use linux_object::thread::ThreadExt;
use numeric_enum_macro::numeric_enum;

impl Syscall<'_> {
    /// Used to change the action taken by a process on receipt of a specific signal.
    pub fn sys_rt_sigaction(
        &self,
        signum: usize,
        act: UserInPtr<SignalAction>,
        mut oldact: UserOutPtr<SignalAction>,
        sigsetsize: usize,
    ) -> SysResult {
        let signal = Signal::try_from(signum as u8).map_err(|_| LxError::EINVAL)?;
        info!(
            "rt_sigaction: signal={:?}, act={:?}, oldact={:?}, sigsetsize={}",
            signal, act, oldact, sigsetsize
        );
        if sigsetsize != core::mem::size_of::<Sigset>()
            || signal == Signal::SIGKILL
            || signal == Signal::SIGSTOP
        {
            return Err(LxError::EINVAL);
        }
        let proc = self.linux_process();
        oldact.write_if_not_null(proc.signal_action(signal))?;
        if let Some(act) = act.read_if_not_null()? {
            info!("new action: {:?} -> {:x?}", signal, act);
            proc.set_signal_action(signal, act);
        }
        Ok(0)
    }

    /// Used to fetch and/or change the signal mask of the calling thread
    pub fn sys_rt_sigprocmask(
        &mut self,
        how: i32,
        set: UserInPtr<Sigset>,
        mut oldset: UserOutPtr<Sigset>,
        sigsetsize: usize,
    ) -> SysResult {
        numeric_enum! {
            #[repr(i32)]
            #[derive(Debug)]
            enum How {
                Block = 0,
                Unblock = 1,
                SetMask = 2,
            }
        }
        let how = How::try_from(how).map_err(|_| LxError::EINVAL)?;
        info!(
            "rt_sigprocmask: how={:?}, set={:?}, oldset={:?}, sigsetsize={}",
            how, set, oldset, sigsetsize
        );
        if sigsetsize != core::mem::size_of::<Sigset>() {
            return Err(LxError::EINVAL);
        }
        oldset.write_if_not_null(self.thread.lock_linux().signal_mask)?;
        if set.is_null() {
            return Ok(0);
        }
        let set = set.read()?;
        let mut thread = self.thread.lock_linux();
        match how {
            How::Block => thread.signal_mask.insert_set(&set),
            How::Unblock => thread.signal_mask.remove_set(&set),
            How::SetMask => thread.signal_mask = set,
        }
        Ok(0)
    }

    /// Allows a process to define a new alternate signal stack
    /// and/or retrieve the state of an existing alternate signal stack
    pub fn sys_sigaltstack(
        &self,
        ss: UserInPtr<SignalStack>,
        mut old_ss: UserOutPtr<SignalStack>,
    ) -> SysResult {
        info!("sigaltstack: ss={:?}, old_ss={:?}", ss, old_ss);
        let mut thread = self.thread.lock_linux();
        old_ss.write_if_not_null(thread.signal_alternate_stack)?;
        if ss.is_null() {
            return Ok(0);
        }
        let ss = ss.read()?;
        // check stack size when not disable
        const MIN_SIGSTACK_SIZE: usize = 2048;
        if ss.flags.contains(SignalStackFlags::DISABLE) && ss.size < MIN_SIGSTACK_SIZE {
            return Err(LxError::ENOMEM);
        }
        // only allow SS_AUTODISARM and SS_DISABLE
        if !(SignalStackFlags::AUTODISARM | SignalStackFlags::DISABLE).contains(ss.flags) {
            return Err(LxError::EINVAL);
        }
        let old_ss = &mut thread.signal_alternate_stack;
        if old_ss.flags.contains(SignalStackFlags::ONSTACK) {
            // cannot change signal alternate stack when we are on it
            // see man sigaltstack(2)
            return Err(LxError::EPERM);
        }
        *old_ss = ss;
        Ok(0)
    }

    /// Send a signal to a process specified by pid
    /// TODO1: support all the arguments
    /// TODO2: support all the signals
    pub fn sys_kill(&self, pid: isize, signum: usize) -> SysResult {
        // Other signals except SIGKILL are not supported
        let signal = Signal::try_from(signum as u8).map_err(|_| LxError::EINVAL)?;
        info!(
            "kill: thread {} kill process {} with signal {:?}",
            self.thread.id(),
            pid,
            signal
        );
        enum SendTarget {
            EveryProcessInGroup,
            EveryProcess,
            EveryProcessInGroupByPID(KoID),
            Pid(KoID),
        }
        let target = match pid {
            p if p > 0 => SendTarget::Pid(p as KoID),
            0 => SendTarget::EveryProcessInGroup,
            -1 => SendTarget::EveryProcess,
            p if p < -1 => SendTarget::EveryProcessInGroupByPID((-p) as KoID),
            _ => unimplemented!()
        };
        let parent = self.zircon_process().clone();
        match target {
            SendTarget::Pid(pid) => {
                match parent.job().get_child(pid as u64) {
                    Ok(obj) => {
                        match signal {
                            Signal::SIGKILL => {
                                let current_pid = parent.id();
                                if current_pid == (pid as u64) {
                                    // killing myself
                                    parent.exit(-1);
                                } else {
                                    let process: Arc<Process> = obj.downcast_arc().unwrap();
                                    process.exit(-1);
                                }
                            }
                            _ => unimplemented!()
                        };
                        Ok(0)
                    }
                    Err(_) => Err(LxError::EINVAL)
                }
            }
            _ => unimplemented!()
        }
    }


    /// Send a signal to a thread specified by tid
    /// TODO: support all the signals
    pub fn sys_tkill(&mut self, tid: usize, signum: usize) -> SysResult {
        // Other signals except SIGKILL are not supported
        let signal = Signal::try_from(signum as u8).map_err(|_| LxError::EINVAL)?;
        info!(
            "tkill: thread {} kill thread {} with signal {:?}",
            self.thread.id(),
            tid,
            signum
        );
        let parent = self.zircon_process().clone();
        match parent.get_child(tid as u64) {
            Ok(obj) => {
                match signal {
                    Signal::SIGRT33 => {
                        let current_tid = self.thread.id();
                        if current_tid == (tid as u64) {
                            // killing myself
                            self.sys_exit(-1).unwrap();
                        } else {
                            let thread: Arc<Thread> = obj.downcast_arc().unwrap();
                            let mut thread_linux = thread.lock_linux();
                            thread_linux.signal_mask.insert(signal);
                            drop(thread_linux);
                        }
                    },
                    _ => unimplemented!()
                };
                Ok(0)
            }
            Err(_) => Err(LxError::EINVAL)
        }
    }

    /// Send a signal to a thread specified by tgid (i.e., process) and pid
    /// TODO: support all the signals
    pub fn sys_tgkill(&mut self, tgid: usize, tid: usize, signum: usize) -> SysResult {
        // Other signals except SIGKILL are not supported
        let signal = Signal::try_from(signum as u8).map_err(|_| LxError::EINVAL)?;
        info!(
            "tkill: thread {} kill thread {} in process {} with signal {:?}",
            self.thread.id(),
            tid,
            tgid,
            signum
        );
        let parent = self.zircon_process().clone();
        match parent.job().get_child(tgid as u64).map(|proc| proc.get_child(tid as u64)) {
            Ok(Ok(obj)) => {
                match signal {
                    Signal::SIGRT33 => {
                        let current_tgid = parent.id();
                        let current_tid = self.thread.id();
                        if current_tgid == (tgid as u64) && current_tid == (tid as u64) {
                            // killing myself
                            self.sys_exit(-1).unwrap();
                        } else {
                            let thread: Arc<Thread> = obj.downcast_arc().unwrap();
                            let mut thread_linux = thread.lock_linux();
                            thread_linux.signal_mask.insert(signal);
                            drop(thread_linux);
                        }
                    },
                    _ => unimplemented!()
                };
                Ok(0)
            }
            _ => Err(LxError::EINVAL)
        }
    }



}