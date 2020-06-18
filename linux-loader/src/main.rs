#![deny(warnings, unused_must_use)]
#![feature(thread_id_value)]

extern crate log;

use linux_loader::*;
use rcore_fs_hostfs::HostFS;
use std::io::Write;
use std::sync::Arc;
use zircon_object::object::*;

#[async_std::main]
async fn main() {
    init_logger();
    kernel_hal_unix::init();

    let args: Vec<_> = std::env::args().skip(1).collect();
    let envs = vec!["PATH=/usr/sbin:/usr/bin:/sbin:/bin:/usr/x86_64-alpine-linux-musl/bin".into()];

    let hostfs = HostFS::new("rootfs");
    let proc: Arc<dyn KernelObject> = run(args, envs, hostfs);
    proc.wait_signal(Signal::PROCESS_TERMINATED).await;
}

fn init_logger() {
    env_logger::builder()
        .format(|buf, record| {
            use env_logger::fmt::Color;
            use log::Level;

            let tid = async_std::task::current().id();
            let mut style = buf.style();
            match record.level() {
                Level::Trace => style.set_color(Color::Black).set_intense(true),
                Level::Debug => style.set_color(Color::White),
                Level::Info => style.set_color(Color::Green),
                Level::Warn => style.set_color(Color::Yellow),
                Level::Error => style.set_color(Color::Red).set_bold(true),
            };
            let level = style.value(record.level());
            writeln!(buf, "[{:>5}][{}] {}", level, tid, record.args())
        })
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test(cmdline: &str) {
        kernel_hal_unix::init();

        let args: Vec<String> = cmdline.split(' ').map(|s| s.into()).collect();
        let envs = vec![]; // TODO
        let hostfs = HostFS::new("../rootfs");
        let proc = run(args, envs, hostfs);
        let proc: Arc<dyn KernelObject> = proc;
        proc.wait_signal(Signal::PROCESS_TERMINATED).await;
    }

    #[async_std::test]
    async fn busybox() {
        test("/bin/busybox").await;
    }
}
