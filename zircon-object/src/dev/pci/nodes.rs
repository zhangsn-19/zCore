#![allow(dead_code)]

use super::{caps::*, config::*, *, region::*};
use alloc::{boxed::Box, sync::*, vec, vec::Vec};
use kernel_hal::{irq_add_handle, irq_disable};
use numeric_enum_macro::*;
use spin::Mutex;
use crate::vm::PAGE_SIZE;

numeric_enum! {
    #[repr(u8)]
    #[derive(PartialEq, Copy, Clone)]
    pub enum PcieDeviceType {
        Unknown = 0xFF,
        PcieEndpoint = 0x0,
        LegacyPcieEndpoint = 0x1,
        RcIntegratedEndpoint = 0x9,
        RcEventCollector = 0xA,
        // Type 1 config header types
        RcRootPort = 0x4,
        SwitchUpstreamPort = 0x5,
        SwitchDownstreamPort = 0x6,
        PcieToPciBridge = 0x7,
        PciToPcieBridge = 0x8,
    }
}

pub struct PcieUpstream {
    pub managed_bus_id: usize,
    pub inner: Mutex<PcieUpstreamInner>,
}

pub struct PcieUpstreamInner {
    pub weak_super: Weak<dyn IPciNode + Send + Sync>,
    pub downstream: Vec<Option<Arc<dyn IPciNode + Send + Sync>>>,
}

impl PcieUpstream {
    pub fn create(managed_bus_id: usize) -> Arc<Self> {
        Arc::new(PcieUpstream {
            managed_bus_id,
            inner: Mutex::new(PcieUpstreamInner {
                weak_super: Weak::<PciRoot>::new(),
                downstream: vec![None; PCI_MAX_FUNCTIONS_PER_BUS],
            }),
        })
    }
    pub fn scan_downstream(&self, driver: &PCIeBusDriver) {
        for dev_id in 0..PCI_MAX_DEVICES_PER_BUS {
            for func_id in 0..PCI_MAX_FUNCTIONS_PER_DEVICE {
                let cfg = driver.get_config(self.managed_bus_id, dev_id, func_id);
                if cfg.is_none() {
                    warn!("bus being scanned is outside ecam region!\n");
                    return;
                }
                let (cfg, _paddr) = cfg.unwrap();
                let vendor_id = cfg.read16(PciReg16::VendorId);
                let mut good_device = vendor_id as usize != PCIE_INVALID_VENDOR_ID;
                if good_device {
                    let device_id = cfg.read16(PciReg16::DeviceId);
                    info!(
                        "Found device {:#x?}:{:#x?} at {:#x?}:{:#x?}.{:#x?}\n",
                        vendor_id, device_id, self.managed_bus_id, dev_id, func_id
                    );
                    let ndx = dev_id * PCI_MAX_FUNCTIONS_PER_DEVICE + func_id;
                    let downstream_device = self.get_downstream(ndx);
                    match downstream_device {
                        Some(dev) => {
                            if let PciNodeType::Bridge = dev.node_type() {
                                dev.as_upstream().unwrap().scan_downstream(driver);
                            }
                        }
                        None => {
                            if let None = self.scan_device(
                                cfg.as_ref(),
                                dev_id,
                                func_id,
                                Some(vendor_id),
                                driver,
                            ) {
                                info!(
                                    "failed to initialize device {:#x?}:{:#x?}.{:#x?}\n",
                                    self.managed_bus_id, dev_id, func_id
                                );
                                good_device = false;
                            }
                        }
                    }
                }
                // At the point of function #0, if either there is no device, or cfg's
                // header indicates that it is not a multi-function device, just move on to
                // next device
                if func_id == 0
                    && (!good_device
                        || (cfg.read8(PciReg8::HeaderType) & PCI_HEADER_TYPE_MULTI_FN) != 0)
                {
                    break;
                }
            }
        }
    }

    pub fn allocate_downstream_bars(&self) {
        for dev_id in 0..PCI_MAX_DEVICES_PER_BUS {
            let dev = self.get_downstream(dev_id);
            if dev.is_none() {
                continue
            }
            let dev = dev.unwrap();
            if dev.allocate_bars().is_err() {
                dev.disable();
            }
        }
    }

    fn scan_device(
        &self,
        cfg: &PciConfig,
        dev_id: usize,
        func_id: usize,
        vendor_id: Option<u16>,
        driver: &PCIeBusDriver,
    ) -> Option<Arc<dyn IPciNode + Send + Sync>> {
        let vendor_id = vendor_id.or(Some(cfg.read16(PciReg16::VendorId))).unwrap();
        if vendor_id == PCIE_INVALID_VENDOR_ID as u16 {
            return None;
        }
        let header_type = cfg.read8(PciReg8::HeaderType) & 0x7f;
        if header_type == PCI_HEADER_TYPE_PCI_BRIDGE {
            let secondary_id = cfg.read8(PciReg8::SecondaryBusId);
            PciBridge::new(
                self.inner.lock().weak_super.clone(),
                dev_id,
                func_id,
                secondary_id as usize,
                driver,
            )
            .map(|x| x as _)
        } else {
            PciDeviceNode::new(
                self.inner.lock().weak_super.clone(),
                dev_id,
                func_id,
                driver,
            )
            .map(|x| x as _)
        }
    }

    pub fn get_downstream(&self, index: usize) -> Option<Arc<dyn IPciNode + Send + Sync>> {
        if index >= PCI_MAX_FUNCTIONS_PER_BUS {
            return None;
        }
        self.inner.lock().downstream[index].clone()
    }
    pub fn set_downstream(&self, ind: usize, down: Option<Arc<dyn IPciNode + Send + Sync>>) {
        self.inner.lock().downstream[ind] = down;
    }

    pub fn set_super(&self, weak_super: Weak<dyn IPciNode + Send + Sync>) {
        self.inner.lock().weak_super = weak_super;
    }
}

#[derive(Default)]
struct PcieBarInfo {
    is_mmio: bool,
    is_64bit: bool,
    is_prefetchable: bool,
    first_bar_reg: usize,
    size: u64,
    bus_addr: u64,
    allocation: Option<Region>,
}
#[derive(Default)]
pub struct SharedLegacyIrqHandler {
    pub irq_id: u32,
    device_handler: Vec<Arc<PcieDevice>>,
}

impl SharedLegacyIrqHandler {
    pub fn create(irq_id: u32) -> Option<Arc<SharedLegacyIrqHandler>> {
        irq_disable(irq_id as u8);
        let handler = Arc::new(SharedLegacyIrqHandler {
            irq_id,
            device_handler: Vec::new(),
        });
        let handler_copy = handler.clone();
        let status = irq_add_handle(irq_id as u8, Box::new(move || handler_copy.handle()));
        assert!(status);
        Some(handler)
    }
    pub fn handle(&self) {
        if self.device_handler.is_empty() {
            irq_disable(self.irq_id as u8);
            return;
        }
        // TODO: with device handler not empty...
    }
    pub fn add_device(&mut self, _device: Arc<PcieDevice>) {
        // TODO:
    }
}
#[derive(Default)]
struct PcieLegacyIrqState {
    pub pin: u8,
    pub id: usize,
    pub shared_handler: Arc<SharedLegacyIrqHandler>,
}

pub struct PcieDevice {
    pub managed_bus_id: usize,
    pub dev_id: usize,
    pub func_id: usize,
    pub is_bridge: bool,
    pub bar_count: usize,
    cfg: Option<Arc<PciConfig>>,
    cfg_phys: usize,
    dev_lock: Mutex<()>,
    command_lock: Mutex<()>,
    vendor_id: u16,
    device_id: u16,
    class_id: u8,
    subclass_id: u8,
    prog_if: u8,
    rev_id: u8,
    inner: Mutex<PcieDeviceInner>,
}

struct PcieDeviceInner {
    pub irq: PcieLegacyIrqState,
    pub bars: [PcieBarInfo; 6],
    pub caps: Vec<PciCapacity>,
    pub plugged_in: bool,
    pub upstream: Weak<(dyn IPciNode + Send + Sync)>,
    pub weak_super: Weak<(dyn IPciNode + Send + Sync)>,
}

impl Default for PcieDeviceInner {
    fn default() -> Self {
        PcieDeviceInner {
            irq: Default::default(),
            bars: Default::default(),
            caps: Default::default(),
            plugged_in: false,
            upstream: Weak::<PciRoot>::new(),
            weak_super: Weak::<PciRoot>::new(),
        }
    }
}

impl PcieDevice {
    pub fn create(
        upstream: Weak<dyn IPciNode + Send + Sync>,
        dev_id: usize,
        func_id: usize,
        driver: &PCIeBusDriver,
    ) -> Option<Arc<Self>> {
        let ups = upstream.upgrade().unwrap().as_upstream();
        if let None = ups {
            return None;
        }
        let ups = ups.unwrap();
        let result = driver.get_config(ups.managed_bus_id, dev_id, func_id);
        if let None = result {
            warn!("Failed to fetch config for device ");
            return None;
        }
        let (cfg, paddr) = result.unwrap();
        let inst = Arc::new(PcieDevice {
            managed_bus_id: ups.managed_bus_id,
            dev_id,
            func_id,
            is_bridge: false,
            bar_count: 6, // PCIE BAR regs per device
            cfg: Some(cfg.clone()),
            cfg_phys: paddr,
            dev_lock: Mutex::default(),
            command_lock: Mutex::default(),
            vendor_id: cfg.read16(PciReg16::VendorId),
            device_id: cfg.read16(PciReg16::DeviceId),
            class_id: cfg.read8(PciReg8::BaseClass),
            subclass_id: cfg.read8(PciReg8::SubClass),
            prog_if: cfg.read8(PciReg8::ProgramInterface),
            rev_id: cfg.read8(PciReg8::RevisionId),
            inner: Default::default(),
        });
        inst.init(upstream, driver).unwrap();
        Some(inst)
    }
    fn init(&self, upstream: Weak<dyn IPciNode + Send + Sync>, driver: &PCIeBusDriver) -> ZxResult {
        self.init_probe_bars()?;
        self.init_capabilities()?;
        self.init_legacy_irq(&upstream, driver)?;
        let mut inner = self.inner.lock();
        inner.plugged_in = true;
        let sup = inner.weak_super.upgrade().unwrap().clone();
        drop(inner);
        driver.link_device_to_upstream(sup, upstream);
        Ok(())
    }

    fn init_probe_bars(&self) -> ZxResult {
        // probe bars
        let mut i = 0;
        let cfg = self.cfg.as_ref().unwrap();
        while i < self.bar_count {
            let bar_val = cfg.read_bar(i);
            let is_mmio = (bar_val & PCI_BAR_IO_TYPE_MASK) == PCI_BAR_IO_TYPE_MMIO;
            let is_64bit = is_mmio && (bar_val & PCI_BAR_MMIO_TYPE_MASK) == PCI_BAR_MMIO_TYPE_64BIT;
            if is_64bit {
                if i + 1 >= self.bar_count {
                    warn!(
                        "Illegal 64-bit MMIO BAR position {}/{} while fetching BAR info\n",
                        i, self.bar_count
                    );
                    return Err(ZxError::BAD_STATE);
                }
            } else {
                if is_mmio && ((bar_val & PCI_BAR_MMIO_TYPE_MASK) != PCI_BAR_MMIO_TYPE_32BIT) {
                    warn!(
                        "Unrecognized MMIO BAR type (BAR[{}] == {:#x?}) while fetching BAR info\n",
                        i, bar_val
                    );
                    return Err(ZxError::BAD_STATE);
                }
            }
            // Disable either MMIO or PIO (depending on the BAR type) access while we perform the probe.
            {
                // let _cmd_lock = self.command_lock.lock(); lock is useless during init
                let backup = cfg.read16(PciReg16::Command);
                cfg.write16(
                    PciReg16::Command,
                    backup
                        & !(if is_mmio {
                            PCI_COMMAND_MEM_EN
                        } else {
                            PCI_COMMAND_IO_EN
                        }),
                );
            }
            // Figure out the size of this BAR region by writing 1's to the address bits
            let addr_mask = if is_mmio {
                PCI_BAR_MMIO_ADDR_MASK
            } else {
                PCI_BAR_PIO_ADDR_MASK
            };
            let addr_lo = bar_val & addr_mask;
            cfg.write_bar(i, bar_val | addr_mask);
            let mut size_mask: u64 = !(cfg.read_bar(i) & addr_mask) as u64;
            cfg.write_bar(i, bar_val);
            if is_mmio && is_64bit {
                let bar_id = i + 1;
                let bar_val = cfg.read_bar(bar_id);
                cfg.write_bar(bar_id, 0xFFFF_FFFF);
                size_mask |= (!cfg.read_bar(bar_id) as u64) << 32;
                cfg.write_bar(bar_id, bar_val);
            }
            let size = if is_mmio {
                size_mask + 1
            } else {
                (size_mask + 1) & PCIE_PIO_ADDR_SPACE_MASK
            };
            let bus_addr = if is_mmio && is_64bit {
                (addr_lo as u64) | ((bar_val as u64) << 32)
            } else {
                addr_lo as u64
            };
            let bar_info = PcieBarInfo {
                is_mmio,
                is_64bit,
                is_prefetchable: is_mmio && (bar_val & PCI_BAR_MMIO_PREFETCH_MASK) != 0,
                first_bar_reg: i,
                size,
                bus_addr,
                allocation: None,
            };
            let bar_info_size = bar_info.size;
            self.inner.lock().bars[i] = bar_info;
            i += 1;
            if is_64bit && bar_info_size > 0 {
                i += 1;
                if i >= self.bar_count {
                    return Err(ZxError::BAD_STATE);
                }
            }
        }
        Ok(())
    }
    fn init_capabilities(&self) -> ZxResult {
        let cfg = self.cfg.as_ref().unwrap();
        let mut cap_offset = cfg.read8(PciReg8::CapabilitiesPtr);
        let mut found_num = 0;
        while cap_offset != 0 && found_num < (256 - 64) / 4 {
            if cap_offset == 0xff || cap_offset < 64 || cap_offset > 252 {
                return Err(ZxError::INVALID_ARGS);
            }
            let id = cfg.read8_offset(cap_offset as usize);
            let std = PciCapacityStd::create(cap_offset as u16, id);
            let cap = match id {
                0x5 => PciCapacity::Msi(
                    std,
                    PciCapacityMsi::create(cfg.as_ref(), cap_offset as u16, id),
                ),
                0x10 => {
                    PciCapacity::Pcie(std, PciCapPcie::create(cfg.as_ref(), cap_offset as u16, id))
                }
                0x13 => PciCapacity::AdvFeatures(
                    std,
                    PciCapAdvFeatures::create(cfg.as_ref(), cap_offset as u16, id),
                ),
                _ => PciCapacity::Std(std),
            };
            self.inner.lock().caps.push(cap);
            cap_offset = cfg.read8_offset(cap_offset as usize + 1) & 0xFC;
            found_num += 1;
        }
        Ok(())
    }
    fn init_legacy_irq(
        &self,
        upstream: &Weak<dyn IPciNode + Send + Sync>,
        driver: &PCIeBusDriver,
    ) -> ZxResult {
        self.modify_cmd(0, 1 << 10);
        let cfg = self.cfg.as_ref().unwrap();
        let pin = cfg.read8(PciReg8::InterruptPin);
        let mut inner = self.inner.lock();
        inner.irq.pin = pin;
        if pin != 0 {
            inner.irq.pin = self.map_pin_to_irq_locked(upstream, pin)? as u8;
            inner.irq.shared_handler = driver.find_legacy_irq_handler(inner.irq.id as u32)?;
        }
        Ok(())
    }
    fn map_pin_to_irq_locked(
        &self,
        // _lock: &MutexGuard<()>, lock is useless during init
        upstream: &Weak<(dyn IPciNode + Send + Sync)>,
        mut pin: u8,
    ) -> ZxResult<usize> {
        // Don't use self.inner.lock() in this function !!!
        if pin == 0 || pin > 4 {
            return Err(ZxError::BAD_STATE);
        }
        pin -= 1;
        let mut dev_id = self.dev_id;
        let mut func_id = self.func_id;
        let mut upstream = upstream.clone();
        while let Some(up) = upstream.upgrade() {
            if let PciNodeType::Bridge = up.node_type() {
                let bdev = up.device().unwrap();
                match bdev.pcie_device_type() {
                    PcieDeviceType::Unknown
                    | PcieDeviceType::SwitchUpstreamPort
                    | PcieDeviceType::PcieToPciBridge
                    | PcieDeviceType::PciToPcieBridge => {
                        pin = (pin + dev_id as u8) % 4;
                    }
                    _ => (),
                }
                let dev = up.device().unwrap();
                dev_id = dev.dev_id;
                func_id = dev.func_id;
                upstream = dev.upstream();
            } else {
                break;
            }
        }
        let upstream = upstream.upgrade();
        if let Some(up_ptr) = upstream {
            if let Some(up) = up_ptr.to_root() {
                return up.swizzle(dev_id, func_id, pin as usize);
            }
        }
        Err(ZxError::BAD_STATE)
    }

    pub fn allocate_bars(&self) -> ZxResult {
        let mut inner = self.inner.lock();
        assert_eq!(inner.plugged_in, true);
        for i in 0..self.bar_count {
            let mut bar_info = &mut inner.bars[i];
            if bar_info.size == 0 || bar_info.allocation.is_some() {
                continue;
            }
            let upstream = self.upstream().upgrade().ok_or(ZxError::UNAVAILABLE)?;
            if bar_info.bus_addr != 0 {
                let allocator = if upstream.node_type() == PciNodeType::Bridge && bar_info.is_prefetchable {
                    Some(upstream.pf_mmio_regions())
                } else if bar_info.is_mmio {
                    let inclusive_end = bar_info.bus_addr + bar_info.size - 1;
                    if inclusive_end <= u32::MAX.into() {
                        Some(upstream.mmio_lo_regions())
                    } else if bar_info.bus_addr > u32::MAX.into() {
                        Some(upstream.mmio_hi_regions())
                    } else {
                        None
                    }
                } else {
                    Some(upstream.pio_regions())
                };
                if allocator.is_some() {
                    if let Ok(a) = allocator.unwrap().as_ref().get_region(bar_info.bus_addr, bar_info.size) {
                        bar_info.allocation = Some(a);
                        continue;
                    }
                }
                error!("Failed to preserve device window");
                bar_info.bus_addr = 0;
            }
            self.assign_cmd(PCIE_CFG_COMMAND_INT_DISABLE);
            let allocator = if bar_info.is_mmio {
                if bar_info.is_64bit {upstream.mmio_hi_regions()} else {upstream.mmio_lo_regions()}
            } else {
                upstream.pio_regions()
            };
            let addr_mask: u32 = if bar_info.is_mmio {PCI_BAR_MMIO_ADDR_MASK} else {PCI_BAR_PIO_ADDR_MASK};
            let is_io_space = PCIE_HAS_IO_ADDR_SPACE && bar_info.is_mmio;
            let align_size = if bar_info.size as usize >= PAGE_SIZE || is_io_space { bar_info.size } else { PAGE_SIZE as u64};
            match allocator.get_region(align_size, align_size) {
                Ok(a) => bar_info.allocation = Some(a),
                Err(e) => {
                    if e == ZxError::NOT_FOUND && bar_info.is_mmio && bar_info.is_64bit {
                        bar_info.allocation = Some(upstream.mmio_lo_regions().as_ref().get_region(align_size, align_size)?);
                    } else {
                        return Err(e);
                    }
                }
            }
            let bar_reg = bar_info.first_bar_reg;
            bar_info.bus_addr = bar_info.allocation.as_ref().unwrap().base;
            let cfg = self.cfg.as_ref().unwrap();
            let bar_val = cfg.read_bar(bar_reg) & !addr_mask;
            cfg.write_bar(bar_reg, (bar_info.bus_addr & 0xFFFF_FFFF) as u32 | bar_val);
            if bar_info.is_64bit {
                cfg.write_bar(bar_reg + 1, (bar_info.bus_addr >> 32) as u32);
            }
        }
        Ok(())
    }

    fn assign_cmd(&self, value: u16) {
        self.modify_cmd(0xffff, value)
    }

    fn modify_cmd(&self, clr: u16, set: u16) {
        let _cmd_lock = self.command_lock.lock();
        let cfg = self.cfg.as_ref().unwrap();
        let oldval = cfg.read16(PciReg16::Command);
        cfg.write16(PciReg16::Command, oldval & !clr | set)
    }
    pub fn upstream(&self) -> Weak<dyn IPciNode + Send + Sync> {
        self.inner.lock().upstream.clone()
    }
    pub fn dev_id(&self) -> usize {
        self.dev_id
    }
    pub fn func_id(&self) -> usize {
        self.func_id
    }
    pub fn set_upstream(&self, up: Weak<dyn IPciNode + Send + Sync>) {
        self.inner.lock().upstream = up;
    }
    pub fn set_super(&self, sup: Weak<dyn IPciNode + Send + Sync>) {
        self.inner.lock().weak_super = sup;
    }
    fn pcie_device_type(&self) -> PcieDeviceType {
        for cap in self.inner.lock().caps.iter() {
            if let PciCapacity::Pcie(_std, pcie) = cap {
                return pcie.dev_type;
            }
        }
        PcieDeviceType::Unknown
    }
    pub fn config(&self) -> Option<Arc<PciConfig>> {
        self.cfg.clone()
    }
}

#[derive(PartialEq, Eq)]
pub enum PciNodeType {
    Root,
    Bridge,
    Device,
}

pub trait IPciNode {
    fn node_type(&self) -> PciNodeType;
    fn device(&self) -> Option<Arc<PcieDevice>>;
    fn as_upstream(&self) -> Option<Arc<PcieUpstream>>;
    fn to_root(&self) -> Option<&PciRoot>;
    fn to_device(&mut self) -> Option<&mut PciDeviceNode>;
    fn to_bridge(&mut self) -> Option<&mut PciBridge>;
    fn allocate_bars(&self) -> ZxResult {unimplemented!("IPciNode.allocate_bars")}
    fn disable(&self) { unimplemented!("IPciNode.disable");}
    fn pf_mmio_regions(&self) -> Arc<RegionAllocator> { unimplemented!("IPciNode.pf_mmio_regions");}
    fn mmio_lo_regions(&self) -> Arc<RegionAllocator> { unimplemented!("IPciNode.mmio_lo_regions");}
    fn mmio_hi_regions(&self) -> Arc<RegionAllocator> { unimplemented!("IPciNode.mmio_hi_regions");}
    fn pio_regions(&self) -> Arc<RegionAllocator> { unimplemented!("IPciNode.pio_regions");}
}

pub struct PciRoot {
    pub base_upstream: Arc<PcieUpstream>,
    lut: PciIrqSwizzleLut,
}

impl PciRoot {
    pub fn new(bus_id: usize, lut: PciIrqSwizzleLut) -> Arc<Self> {
        let inner_ups = PcieUpstream::create(bus_id);
        let node = Arc::new(PciRoot {
            base_upstream: inner_ups,
            lut,
        });
        node.base_upstream
            .set_super(Arc::downgrade(&(node.clone() as _)));
        node
    }
    pub fn swizzle(&self, dev_id: usize, func_id: usize, pin: usize) -> ZxResult<usize> {
        self.lut.swizzle(dev_id, func_id, pin)
    }
    pub fn managed_bus_id(&self) -> usize {
        self.base_upstream.managed_bus_id
    }
}

impl IPciNode for PciRoot {
    fn node_type(&self) -> PciNodeType {
        PciNodeType::Root
    }
    fn device(&self) -> Option<Arc<PcieDevice>> {
        None
    }
    fn as_upstream(&self) -> Option<Arc<PcieUpstream>> {
        Some(self.base_upstream.clone())
    }
    fn to_root(&self) -> Option<&PciRoot> {
        Some(self)
    }
    fn to_device(&mut self) -> Option<&mut PciDeviceNode> {
        None
    }
    fn to_bridge(&mut self) -> Option<&mut PciBridge> {
        None
    }
    fn allocate_bars(&self) -> ZxResult {
        unimplemented!();
    }
}

pub struct PciDeviceNode {
    base_device: Arc<PcieDevice>,
}

impl PciDeviceNode {
    pub fn new(
        upstream: Weak<dyn IPciNode + Send + Sync>,
        dev_id: usize,
        func_id: usize,
        driver: &PCIeBusDriver,
    ) -> Option<Arc<Self>> {
        PcieDevice::create(upstream, dev_id, func_id, driver).map(|x| {
            let node = Arc::new(PciDeviceNode { base_device: x });
            node.base_device
                .as_ref()
                .set_super(Arc::downgrade(&(node.clone() as _)));
            test_interface(node.clone() as _);
            node
        })
    }
}

fn test_interface(_t: Arc<(dyn IPciNode + Send + Sync)>) {}

impl IPciNode for PciDeviceNode {
    fn node_type(&self) -> PciNodeType {
        PciNodeType::Device
    }
    fn device(&self) -> Option<Arc<PcieDevice>> {
        Some(self.base_device.clone())
    }
    fn as_upstream(&self) -> Option<Arc<PcieUpstream>> {
        None
    }
    fn to_root(&self) -> Option<&PciRoot> {
        None
    }
    fn to_device(&mut self) -> Option<&mut PciDeviceNode> {
        Some(self)
    }
    fn to_bridge(&mut self) -> Option<&mut PciBridge> {
        None
    }
    fn allocate_bars(&self) -> ZxResult {
        self.base_device.allocate_bars()
    }
}

pub struct PciBridge {
    base_device: Arc<PcieDevice>,
    base_upstream: Arc<PcieUpstream>,
}

impl PciBridge {
    pub fn new(
        upstream: Weak<dyn IPciNode + Send + Sync>,
        dev_id: usize,
        func_id: usize,
        managed_bus_id: usize,
        driver: &PCIeBusDriver,
    ) -> Option<Arc<Self>> {
        let father = upstream.upgrade().and_then(|x| x.as_upstream());
        if father.is_none() {
            return None;
        }
        let inner_ups = PcieUpstream::create(managed_bus_id);
        let inner_dev = PcieDevice::create(upstream, dev_id, func_id, driver);
        inner_dev.map(move |x| {
            let node = Arc::new(PciBridge {
                base_device: x,
                base_upstream: inner_ups,
            });
            node.base_device
                .set_super(Arc::downgrade(&(node.clone() as _)));
            node.base_upstream
                .set_super(Arc::downgrade(&(node.clone() as _)));
            node
        })
    }
}

impl IPciNode for PciBridge {
    fn node_type(&self) -> PciNodeType {
        PciNodeType::Bridge
    }
    fn device(&self) -> Option<Arc<PcieDevice>> {
        Some(self.base_device.clone())
    }
    fn as_upstream(&self) -> Option<Arc<PcieUpstream>> {
        Some(self.base_upstream.clone())
    }
    fn to_root(&self) -> Option<&PciRoot> {
        None
    }
    fn to_device(&mut self) -> Option<&mut PciDeviceNode> {
        None
    }
    fn to_bridge(&mut self) -> Option<&mut PciBridge> {
        Some(self)
    }
}

const PCI_HEADER_TYPE_MULTI_FN: u8 = 0x80;
const PCI_HEADER_TYPE_STANDARD: u8 = 0x00;
const PCI_HEADER_TYPE_PCI_BRIDGE: u8 = 0x01;

const PCI_BAR_IO_TYPE_MASK: u32 = 0x1;
const PCI_BAR_IO_TYPE_MMIO: u32 = 0x0;
const PCI_BAR_IO_TYPE_PIO: u32 = 0x1;

const PCI_BAR_MMIO_TYPE_MASK: u32 = 0x6;
const PCI_BAR_MMIO_TYPE_32BIT: u32 = 0x0;
const PCI_BAR_MMIO_TYPE_64BIT: u32 = 0x4;
const PCI_BAR_MMIO_ADDR_MASK: u32 = 0xFFFF_FFF0;
const PCI_BAR_PIO_ADDR_MASK: u32 = 0xFFFF_FFFC;

const PCI_BAR_MMIO_PREFETCH_MASK: u32 = 0x8;

const PCI_COMMAND_IO_EN: u16 = 0x0001;
const PCI_COMMAND_MEM_EN: u16 = 0x0002;

const PCIE_CFG_COMMAND_INT_DISABLE: u16 = 1 << 10;
const PCIE_CFG_STATUS_INT_SYS: u16 = 1 << 3;

#[cfg(target_arch = "x86_64")]
const PCIE_HAS_IO_ADDR_SPACE: bool = true;
#[cfg(not(target_arch = "x86_64"))]
const PCIE_HAS_IO_ADDR_SPACE: bool = false;