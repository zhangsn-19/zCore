use {super::*, alloc::sync::Arc, spin::Mutex};

/// VMO representing a physical range of memory.
pub struct VMObjectPhysical {
    paddr: PhysAddr,
    pages: usize,
    /// Lock this when access physical memory.
    data_lock: Mutex<()>,
}

impl VMObjectPhysical {
    /// Create a new VMO representing a piece of contiguous physical memory.
    ///
    /// # Safety
    ///
    /// You must ensure nobody has the ownership of this piece of memory yet.
    #[allow(unsafe_code)]
    pub unsafe fn new(paddr: PhysAddr, pages: usize) -> Arc<Self> {
        assert!(page_aligned(paddr));
        Arc::new(VMObjectPhysical {
            paddr,
            pages,
            data_lock: Mutex::default(),
        })
    }
}

impl VMObjectTrait for VMObjectPhysical {
    fn read(&self, offset: usize, buf: &mut [u8]) -> ZxResult {
        let _ = self.data_lock.lock();
        assert!(offset + buf.len() <= self.len());
        kernel_hal::pmem_read(self.paddr + offset, buf);
        Ok(())
    }

    fn write(&self, offset: usize, buf: &[u8]) -> ZxResult {
        let _ = self.data_lock.lock();
        assert!(offset + buf.len() <= self.len());
        kernel_hal::pmem_write(self.paddr + offset, buf);
        Ok(())
    }

    fn len(&self) -> usize {
        self.pages * PAGE_SIZE
    }

    fn set_len(&self, _len: usize) {
        unimplemented!()
    }

    fn commit_page(&self, page_idx: usize, _flags: MMUFlags) -> ZxResult<PhysAddr> {
        Ok(self.paddr + page_idx * PAGE_SIZE)
    }

    fn commit(&self, _offset: usize, _len: usize) -> ZxResult {
        // do nothing
        Ok(())
    }

    fn decommit(&self, _offset: usize, _len: usize) -> ZxResult {
        // do nothing
        Ok(())
    }

    fn create_child(&self, _offset: usize, _len: usize, _user_id: KoID) -> Arc<dyn VMObjectTrait> {
        unimplemented!()
    }

    fn append_mapping(&self, _mapping: Weak<VmMapping>) {
        //        unimplemented!()
        // TODO this function is only used when physical-vmo supports create_child
    }

    fn complete_info(&self, _info: &mut ZxInfoVmo) {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    #![allow(unsafe_code)]
    use super::*;

    #[test]
    fn read_write() {
        let vmo = unsafe { VmObject::new_physical(0x1000, 2) };
        super::super::tests::read_write(&vmo);
    }
}
