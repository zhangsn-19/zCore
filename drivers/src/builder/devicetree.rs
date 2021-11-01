use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

use super::IoMapper;
use crate::utils::devicetree::{Devicetree, InheritProps, Node, StringList};
use crate::{Device, DeviceError, DeviceResult, VirtAddr};

/// A wrapper of [`Device`] which provides interrupt information additionally.
#[derive(Debug)]
struct DevWithInterrupt {
    /// For interrupt controller, represent the `phandle` property, otherwise
    /// is `None`.
    phandle: Option<u32>,
    /// For interrupt controller, represent the `interrupt_cells` property,
    /// otherwise is `None`.
    interrupt_cells: Option<u32>,
    /// A unified representation of the `interrupts` and `interrupts_extended`
    /// properties for any interrupt generating device.
    interrupts_extended: Vec<u32>,
    /// The inner [`Device`] structure.
    dev: Device,
}

pub struct DevicetreeDriverBuilder<M: IoMapper> {
    dt: Devicetree,
    io_mapper: M,
}

impl<M: IoMapper> DevicetreeDriverBuilder<M> {
    pub fn new(dtb_base_vaddr: VirtAddr, io_mapper: M) -> DeviceResult<Self> {
        Ok(Self {
            dt: Devicetree::from(dtb_base_vaddr)?,
            io_mapper,
        })
    }

    pub fn build(&self) -> DeviceResult<Vec<Device>> {
        let mut intc_map = BTreeMap::new();
        let mut dev_list = Vec::new();

        self.dt.walk(&mut |node, comp, props| {
            if let Ok(dev) = self.parse_device(node, comp, props) {
                if node.has_prop("interrupt-controller") {
                    if let Some(phandle) = dev.phandle {
                        intc_map.insert(phandle, dev_list.len());
                    }
                }
                dev_list.push(dev);
            }
        });

        for dev in &dev_list {
            register_interrupt(dev, &dev_list, &intc_map).ok();
        }

        Ok(dev_list.into_iter().map(|d| d.dev).collect())
    }
}

#[allow(dead_code)]
#[allow(unused_imports)]
#[allow(unused_variables)]
#[allow(unreachable_code)]
impl<M: IoMapper> DevicetreeDriverBuilder<M> {
    fn parse_device(
        &self,
        node: &Node,
        comp: &StringList,
        props: &InheritProps,
    ) -> DeviceResult<DevWithInterrupt> {
        debug!(
            "device-tree: parsing node {:?} with compatible {:?}",
            node.name, comp
        );

        let res = if node.has_prop("interrupt-controller") {
            self.parse_intc(node, comp, props)
        } else {
            match comp {
                #[cfg(feature = "virtio")]
                c if c.contains("virtio,mmio") => self.parse_virtio(node, props),
                c if c.contains("ns16550a") => self.parse_uart(node, comp, props),
                _ => Err(DeviceError::NotSupported),
            }
        };

        if let Err(err) = &res {
            if !matches!(err, DeviceError::NotSupported) {
                warn!(
                    "device-tree: failed to parsing node {:?}: {:?}",
                    node.name, err
                );
            }
        }
        res
    }

    fn parse_intc(
        &self,
        node: &Node,
        comp: &StringList,
        props: &InheritProps,
    ) -> DeviceResult<DevWithInterrupt> {
        let phandle = node.prop_u32("phandle").ok();
        let interrupt_cells = node.prop_u32("#interrupt-cells").ok();
        let interrupts_extended = parse_interrupts(node, props)?;
        if phandle.is_none() || interrupt_cells.is_none() {
            return Err(DeviceError::InvalidParam);
        }
        let base_vaddr = parse_reg(node, props).and_then(|(paddr, size)| {
            self.io_mapper
                .query_or_map(paddr as usize, size as usize)
                .ok_or(DeviceError::NoResources)
        });

        use crate::irq::*;
        let dev = Device::Irq(match comp {
            #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
            c if c.contains("riscv,cpu-intc") => Arc::new(riscv::Intc::new()),
            #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
            c if c.contains("riscv,plic0") => Arc::new(riscv::Plic::new(base_vaddr?)),
            _ => return Err(DeviceError::NotSupported),
        });

        Ok(DevWithInterrupt {
            phandle,
            interrupt_cells,
            interrupts_extended,
            dev,
        })
    }

    #[cfg(feature = "virtio")]
    fn parse_virtio(&self, node: &Node, props: &InheritProps) -> DeviceResult<DevWithInterrupt> {
        use crate::virtio::*;
        use virtio_drivers::{DeviceType, VirtIOHeader};

        let interrupts_extended = parse_interrupts(node, props)?;
        let base_vaddr = parse_reg(node, props).and_then(|(paddr, size)| {
            self.io_mapper
                .query_or_map(paddr as usize, size as usize)
                .ok_or(DeviceError::NoResources)
        })?;
        let header = unsafe { &mut *(base_vaddr as *mut VirtIOHeader) };
        if !header.verify() {
            return Err(DeviceError::NotSupported);
        }
        info!(
            "device-tree: detected virtio device: vendor_id={:#X}, type={:?}",
            header.vendor_id(),
            header.device_type()
        );

        let dev = match header.device_type() {
            DeviceType::Block => Device::Block(Arc::new(VirtIoBlk::new(header)?)),
            DeviceType::GPU => Device::Display(Arc::new(VirtIoGpu::new(header)?)),
            DeviceType::Input => Device::Input(Arc::new(VirtIoInput::new(header)?)),
            DeviceType::Console => Device::Uart(Arc::new(VirtIoConsole::new(header)?)),
            _ => return Err(DeviceError::NotSupported),
        };

        Ok(DevWithInterrupt {
            phandle: None,
            interrupt_cells: None,
            interrupts_extended,
            dev,
        })
    }

    fn parse_uart(
        &self,
        node: &Node,
        comp: &StringList,
        props: &InheritProps,
    ) -> DeviceResult<DevWithInterrupt> {
        let interrupts_extended = parse_interrupts(node, props)?;
        let base_vaddr = parse_reg(node, props).and_then(|(paddr, size)| {
            self.io_mapper
                .query_or_map(paddr as usize, size as usize)
                .ok_or(DeviceError::NoResources)
        });

        use crate::uart::*;
        let dev = Device::Uart(match comp {
            c if c.contains("ns16550a") => {
                Arc::new(unsafe { Uart16550Mmio::<u8>::new(base_vaddr?) })
            }
            _ => return Err(DeviceError::NotSupported),
        });

        Ok(DevWithInterrupt {
            phandle: None,
            interrupt_cells: None,
            interrupts_extended,
            dev,
        })
    }
}

fn register_interrupt(
    dev: &DevWithInterrupt,
    dev_list: &[DevWithInterrupt],
    intc_map: &BTreeMap<u32, usize>,
) -> DeviceResult {
    let mut pos = 0;
    while pos < dev.interrupts_extended.len() {
        let parent = dev.interrupts_extended[pos];
        if let Some(intc) = intc_map.get(&parent).map(|&i| &dev_list[i]) {
            let cells = intc.interrupt_cells.ok_or(DeviceError::InvalidParam)?;
            if let Device::Irq(irq) = &intc.dev {
                let irq_num = dev.interrupts_extended[pos + 1] as usize;
                if irq_num != 0xffff_ffff {
                    info!(
                        "device-tree: register interrupts for {:?}: {:?}, irq_num={:#x}",
                        intc.dev, dev.dev, irq_num
                    );
                    irq.register_device(irq_num, dev.dev.inner())?;
                    irq.unmask(irq_num)?;
                }
            } else {
                warn!(
                    "device-tree: node with phandle {:#x} is not an interrupt-controller",
                    parent
                );
                return Err(DeviceError::InvalidParam);
            }
            pos += 1 + cells as usize;
        } else {
            warn!(
                "device-tree: no such node with phandle {:#x} as the interrupt-parent",
                parent
            );
            return Err(DeviceError::InvalidParam);
        }
    }
    Ok(())
}

fn from_cells(cells: &[u32], cell_num: u32) -> DeviceResult<u64> {
    if cell_num as usize > cells.len() {
        return Err(DeviceError::InvalidParam);
    }
    let mut value = 0;
    for &c in &cells[..cell_num as usize] {
        value = value << 32 | c as u64;
    }
    Ok(value)
}

fn parse_reg(node: &Node, props: &InheritProps) -> DeviceResult<(u64, u64)> {
    let cells = node.prop_cells("reg")?;
    let addr = from_cells(&cells, props.parent_address_cells)?;
    let size = from_cells(
        &cells[props.parent_address_cells as usize..],
        props.parent_size_cells,
    )?;
    Ok((addr, size))
}

fn parse_interrupts(node: &Node, props: &InheritProps) -> DeviceResult<Vec<u32>> {
    if node.has_prop("interrupts-extended") {
        Ok(node.prop_cells("interrupts-extended")?)
    } else if node.has_prop("interrupts") && props.interrupt_parent > 0 {
        let mut ret = node.prop_cells("interrupts")?;
        ret.insert(0, props.interrupt_parent);
        Ok(ret)
    } else {
        Ok(Vec::new())
    }
}
