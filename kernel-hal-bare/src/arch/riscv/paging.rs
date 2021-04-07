use core::mem::ManuallyDrop;
use log::*;
use rcore_memory::paging::*;
use riscv::addr::{Page, PhysAddr, VirtAddr};
use riscv::asm::{sfence_vma, sfence_vma_all};
use riscv::paging::{FrameAllocator, FrameDeallocator};
use riscv::paging::{Mapper, PageTable as RvPageTable, PageTableEntry, PageTableFlags as EF};
use riscv::register::satp;

use crate::{phys_to_virt, Frame};
use super::{FrameAllocatorImpl, PHYSICAL_MEMORY_OFFSET};

#[cfg(target_arch = "riscv32")]
type TopLevelPageTable<'a> = riscv::paging::Rv32PageTable<'a>;
#[cfg(target_arch = "riscv64")]
type TopLevelPageTable<'a> = riscv::paging::Rv39PageTable<'a>;

pub struct PageTableImpl {
    pub page_table: TopLevelPageTable<'static>,
    pub root_frame: riscv::addr::Frame,
    pub entry: Option<PageEntry>,
}

/// PageTableEntry: the contents of this entry.
/// Page: this entry is the pte of page `Page`.
pub struct PageEntry(&'static mut PageTableEntry, Page);

impl PageTable for PageTableImpl {
    fn map(&mut self, addr: usize, target: usize) -> &mut dyn Entry {
        // map the 4K `page` to the 4K `frame` with `flags`
        let flags = EF::VALID | EF::READABLE | EF::WRITABLE;
        let page = riscv::addr::Page::of_addr(riscv::addr::VirtAddr::new(addr));
        let frame = riscv::addr::Frame::of_addr(PhysAddr::new(target));
        // we may need frame allocator to alloc frame for new page table(first/second)

        trace!("map PageTableImpl page:{:#x} -> frame:{:#x}", addr, target);

        // map() may stuck here
        // 注意不要在已经map_kernel()进行1G大页映射后的root_table中, 重建p3 p2 p1多级页表，否则出错；
        // 因与Rv39PageTable的create_p1_if_not_exist()多级页表的建立产生冲突
        self.page_table
            .map_to(page, frame, flags, &mut FrameAllocatorImpl)
            .unwrap()
            .flush();
        self.get_entry(addr).expect("fail to get entry")
    }

    fn unmap(&mut self, addr: usize) {
        let page = Page::of_addr(VirtAddr::new(addr));
        let (_, flush) = self.page_table.unmap(page).unwrap();
        flush.flush();
    }

    fn get_entry(&mut self, vaddr: usize) -> Option<&mut dyn Entry> {
        let page = Page::of_addr(VirtAddr::new(vaddr));
        if let Ok(e) = self.page_table.ref_entry(page.clone()) {
            let e = unsafe { &mut *(e as *mut PageTableEntry) };
            self.entry = Some(PageEntry(e, page));
            Some(self.entry.as_mut().unwrap())
        } else {
            None
        }
    }

    fn get_page_slice_mut<'a>(&mut self, addr: usize) -> &'a mut [u8] {
        let frame = self
            .page_table
            .translate_page(Page::of_addr(VirtAddr::new(addr)))
            .unwrap();
        let vaddr = frame.start_address().as_usize() + PHYSICAL_MEMORY_OFFSET;
        unsafe { core::slice::from_raw_parts_mut(vaddr as *mut u8, 0x1000) }
    }

    fn flush_cache_copy_user(&mut self, _start: usize, _end: usize, _execute: bool) {}
}

/// implementation for the Entry trait in /crate/memory/src/paging/mod.rs
impl Entry for PageEntry {
    fn update(&mut self) {
        unsafe {
            sfence_vma(0, self.1.start_address().as_usize());
        }
    }
    fn accessed(&self) -> bool {
        self.0.flags().contains(EF::ACCESSED)
    }
    fn dirty(&self) -> bool {
        self.0.flags().contains(EF::DIRTY)
    }
    fn writable(&self) -> bool {
        self.0.flags().contains(EF::WRITABLE)
    }
    fn present(&self) -> bool {
        self.0.flags().contains(EF::VALID | EF::READABLE)
    }
    //access和dirty两位在set()函数中会被自动置位??? 注意下
    fn clear_accessed(&mut self) {
        warn!("PageTableImpl clear access, may not work out");
        let flags = self.0.flags() & !EF::ACCESSED;
        let frame = self.0.frame();
        self.0.set(frame, flags);
    }
    fn clear_dirty(&mut self) {
        warn!("PageTableImpl clear access, may not work out");
        let flags = self.0.flags() & !EF::DIRTY;
        let frame = self.0.frame();
        self.0.set(frame, flags);
    }
    fn set_writable(&mut self, value: bool) {
        let flags = if value {
            self.0.flags() | EF::WRITABLE
        } else {
            self.0.flags() & !EF::WRITABLE
        };

        let frame = self.0.frame();
        self.0.set(frame, flags);
        //let pte = (self.0.frame().number() << 10) | flags.bits();
        //warn!("PageTableImpl set {} writable: {:#x?}", value, self.0.flags());
    }
    fn set_present(&mut self, value: bool) {
        let flags = if value {
            self.0.flags() | (EF::VALID | EF::READABLE)
        } else {
            self.0.flags() & !(EF::VALID | EF::READABLE)
        };
        let frame = self.0.frame();
        self.0.set(frame, flags);
    }
    fn target(&self) -> usize {
        self.0.addr().as_usize()
    }
    fn set_target(&mut self, target: usize) {
        let flags = self.0.flags();
        let frame = riscv::addr::Frame::of_addr(PhysAddr::new(target));
        self.0.set(frame, flags);
    }
    fn writable_shared(&self) -> bool {
        self.0.flags().contains(EF::RESERVED1)
    }
    fn readonly_shared(&self) -> bool {
        self.0.flags().contains(EF::RESERVED2)
    }
    fn set_shared(&mut self, writable: bool) {
        let flags = if writable {
            (self.0.flags() | EF::RESERVED1) & !EF::RESERVED2
        } else {
            (self.0.flags() | EF::RESERVED2) & !EF::RESERVED1
        };
        let frame = self.0.frame();
        self.0.set(frame, flags);
    }
    fn clear_shared(&mut self) {
        let flags = self.0.flags() & !(EF::RESERVED1 | EF::RESERVED2);
        let frame = self.0.frame();
        self.0.set(frame, flags);
    }
    fn swapped(&self) -> bool {
        self.0.flags().contains(EF::RESERVED1)
    }
    fn set_swapped(&mut self, value: bool) {
        let flags = if value {
            self.0.flags() | EF::RESERVED1
        } else {
            self.0.flags() & !EF::RESERVED1
        };
        let frame = self.0.frame();
        self.0.set(frame, flags);
    }
    fn user(&self) -> bool {
        self.0.flags().contains(EF::USER)
    }
    fn set_user(&mut self, value: bool) {
        let flags = if value {
            self.0.flags() | EF::USER
        } else {
            self.0.flags() & !EF::USER
        };
        let frame = self.0.frame();
        self.0.set(frame, flags);
    }
    fn execute(&self) -> bool {
        self.0.flags().contains(EF::EXECUTABLE)
    }
    fn set_execute(&mut self, value: bool) {
        let flags = if value {
            self.0.flags() | EF::EXECUTABLE
        } else {
            self.0.flags() & !EF::EXECUTABLE
        };
        let frame = self.0.frame();
        self.0.set(frame, flags);
    }
    fn mmio(&self) -> u8 {
        0
    }
    fn set_mmio(&mut self, _value: u8) {}
}

impl PageTableImpl {
    /// Unsafely get the current active page table.
    /// Using ManuallyDrop to wrap the page table: this is how `core::mem::forget` is implemented now.
    pub unsafe fn active() -> ManuallyDrop<Self> {
        #[cfg(target_arch = "riscv32")]
        let mask = 0x7fffffff;
        #[cfg(target_arch = "riscv64")]
        let mask = 0x0fffffff_ffffffff;
        let frame = riscv::addr::Frame::of_ppn(PageTableImpl::active_token() & mask);
        let table = frame.as_kernel_mut(PHYSICAL_MEMORY_OFFSET);
        ManuallyDrop::new(PageTableImpl {
            page_table: TopLevelPageTable::new(table, PHYSICAL_MEMORY_OFFSET),
            root_frame: frame,
            entry: None,
        })
    }
    /// The method for getting the kernel page table.
    /// In riscv kernel page table and user page table are the same table. However you have to do the initialization.
    pub unsafe fn kernel_table() -> ManuallyDrop<Self> {
        Self::active()
    }

    /// When `vaddr` is not mapped, map it to `paddr`.
    pub fn map_if_not_exists(&mut self, vaddr: usize, paddr: usize) -> bool {
        if let Some(entry) = self.get_entry(vaddr) {
            if entry.present() {
                return false;
            }
        }
        self.map(vaddr, paddr);
        true
    }
}

impl PageTableExt for PageTableImpl {
    fn new_bare() -> Self {
        let RFrame = Frame::alloc().expect("failed to alloc frame");
        let target = RFrame.paddr;
        //let target = alloc_frame().expect("failed to allocate frame");
        let frame = riscv::addr::Frame::of_addr(PhysAddr::new(target));

        let table = unsafe { &mut *(phys_to_virt(target) as *mut RvPageTable) };
        table.zero();

        debug!("new_bare(), frame:{:#x?}, table:{:p}", frame, table);
        //root页表的虚拟地址啥时候映射?
        PageTableImpl {
            page_table: TopLevelPageTable::new(table, PHYSICAL_MEMORY_OFFSET),
            root_frame: frame,
            entry: None,
        }
    }

    fn map_kernel(&mut self) {
        info!("map_kernel linear mapping");
        let table = unsafe {
            &mut *(phys_to_virt(self.root_frame.start_address().as_usize()) as *mut RvPageTable)
        };
        #[cfg(target_arch = "riscv32")]
        for i in 256..1024 {
            let flags =
                EF::VALID | EF::READABLE | EF::WRITABLE | EF::EXECUTABLE | EF::ACCESSED | EF::DIRTY;
            let frame = riscv::addr::Frame::of_addr(PhysAddr::new((i << 22) - PHYSICAL_MEMORY_OFFSET));
            table[i].set(frame, flags);
        }
        #[cfg(target_arch = "riscv64")]
        for i in 509..512 {
            if i == 510 {
                // MMIO range 0x60000000 - 0x7FFFFFFF does not work as a large page, dunno why
                continue;
            }
            let flags =
                EF::VALID | EF::READABLE | EF::WRITABLE | EF::EXECUTABLE | EF::ACCESSED | EF::DIRTY;
            //                          地址4K对齐
            let frame = riscv::addr::Frame::of_addr(PhysAddr::new(
                // ?                     i * 1G
                (0xFFFFFF80_00000000 + (i << 30)) - PHYSICAL_MEMORY_OFFSET,
            ));
            debug!("table[{:?}] frame: {:#x?}", i, frame);
            table[i].set(frame, flags);
        }
    }

    fn token(&self) -> usize {
        #[cfg(target_arch = "riscv32")]
        return self.root_frame.number() | (1 << 31);
        #[cfg(target_arch = "riscv64")]
        return self.root_frame.number() | (8 << 60);
    }

    //设置satp
    unsafe fn set_token(token: usize) {
        satp::write(token);
    }

    fn active_token() -> usize {
        satp::read().bits()
    }

    fn flush_tlb() {
        unsafe {
            sfence_vma_all();
        }
    }
}

impl Drop for PageTableImpl {
    fn drop(&mut self) {
        Frame {
            paddr: self.root_frame.start_address().as_usize(),
        }
        .dealloc()
    }
}
