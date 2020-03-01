use {super::*, bitflags::bitflags, zircon_object::vm::*};

fn amount_of_alignments(options: u32) -> ZxResult<usize> {
    let align_pow2 = (options >> 24) as usize;
    if (align_pow2 < 10 && align_pow2 != 0) || (align_pow2 > 32) {
        Err(ZxError::INVALID_ARGS)
    } else {
        Ok(1 << align_pow2)
    }
}

impl Syscall {
    pub fn sys_vmar_allocate(
        &self,
        parent_vmar: HandleValue,
        options: u32,
        offset: u64,
        size: u64,
        mut out_child_vmar: UserOutPtr<HandleValue>,
        mut out_child_addr: UserOutPtr<usize>,
    ) -> ZxResult<usize> {
        let vm_options = VmOptions::from_bits(options).ok_or(ZxError::INVALID_ARGS)?;
        info!(
            "vmar.allocate: parent={:?}, options={:?}, offset={:#x?}, size={:#x?}",
            parent_vmar, options, offset, size,
        );
        // try to get parent_vmar
        let perm_rights = vm_options.to_rights();
        let proc = self.thread.proc();
        let parent = proc.get_object_with_rights::<VmAddressRegion>(parent_vmar, perm_rights)?;

        // get vmar_flags
        let vmar_flags = vm_options.to_flags();
        if vmar_flags.contains(
            !(VmarFlags::SPECIFIC
                | VmarFlags::CAN_MAP_SPECIFIC
                | VmarFlags::COMPACT
                | VmarFlags::CAN_MAP_RXW),
        ) {
            return Err(ZxError::INVALID_ARGS);
        }

        // get align
        let align = amount_of_alignments(options)?;

        // get offest with options
        let offset = if vm_options.contains(VmOptions::SPECIFIC) {
            Some(offset as usize)
        } else if vm_options.contains(VmOptions::SPECIFIC_OVERWRITE) {
            unimplemented!()
        } else {
            if offset != 0 {
                return Err(ZxError::INVALID_ARGS);
            }
            None
        };

        // check `size`
        if size == 0u64 {
            return Err(ZxError::INVALID_ARGS);
        }
        let child = parent.allocate(offset, size as usize, vmar_flags, align)?;
        let child_addr = child.addr();
        let child_handle = proc.add_handle(Handle::new(child, Rights::DEFAULT_VMAR | perm_rights));
        info!("vmar.allocate: at {:#x?}", child_addr);
        out_child_vmar.write(child_handle)?;
        out_child_addr.write(child_addr)?;
        Ok(0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sys_vmar_map(
        &self,
        vmar_handle: HandleValue,
        options: u32,
        vmar_offset: usize,
        vmo_handle: HandleValue,
        vmo_offset: usize,
        len: usize,
        mut mapped_addr: UserOutPtr<VirtAddr>,
    ) -> ZxResult<usize> {
        let options = VmOptions::from_bits(options).ok_or(ZxError::INVALID_ARGS)?;
        info!(
            "vmar.map: vmar_handle={:?}, options={:?}, vmar_offset={:#x?}, vmo_handle={:?}, vmo_offset={:#x?}, len={:#x?}",
            vmar_handle, options, vmar_offset, vmo_handle, vmo_offset, len
        );
        let proc = self.thread.proc();
        let (vmar, vmar_rights) = proc.get_object_and_rights::<VmAddressRegion>(vmar_handle)?;
        let (vmo, vmo_rights) = proc.get_vmo_and_rights(vmo_handle)?;
        if !vmo_rights.contains(Rights::MAP) {
            return Err(ZxError::ACCESS_DENIED);
        };
        if !options.contains(VmOptions::PERM_READ)
            && (!options.contains(VmOptions::PERM_WRITE)
                || options.contains(VmOptions::PERM_EXECUTE))
        {
            return Err(ZxError::INVALID_ARGS);
        }
        if options.contains(VmOptions::CAN_MAP_RXW) {
            return Err(ZxError::INVALID_ARGS);
        }
        // check SPECIFIC options with offset
        let is_specific = options.contains(VmOptions::SPECIFIC)
            || options.contains(VmOptions::SPECIFIC_OVERWRITE);
        if !is_specific && vmar_offset != 0 {
            return Err(ZxError::INVALID_ARGS);
        }
        let mut mapping_flags = MMUFlags::USER;
        mapping_flags.set(
            MMUFlags::READ,
            vmar_rights.contains(Rights::READ) && vmo_rights.contains(Rights::READ),
        );
        mapping_flags.set(
            MMUFlags::WRITE,
            vmar_rights.contains(Rights::WRITE) && vmo_rights.contains(Rights::WRITE),
        );
        mapping_flags.set(
            MMUFlags::EXECUTE,
            vmar_rights.contains(Rights::EXECUTE) && vmo_rights.contains(Rights::EXECUTE),
        );
        let vaddr = if is_specific {
            vmar.map_at(vmar_offset, vmo, vmo_offset, len, mapping_flags)?
        } else {
            vmar.map(None, vmo, vmo_offset, len, mapping_flags)?
        };
        info!("vmar.map: at {:#x?}", vaddr);
        mapped_addr.write(vaddr)?;
        Ok(0)
    }
}

bitflags! {
    struct VmOptions: u32 {
        #[allow(clippy::identity_op)]
        const PERM_READ             = 1 << 0;
        const PERM_WRITE            = 1 << 1;
        const PERM_EXECUTE          = 1 << 2;
        const COMPACT               = 1 << 3;
        const SPECIFIC              = 1 << 4;
        const SPECIFIC_OVERWRITE    = 1 << 5;
        const CAN_MAP_SPECIFIC      = 1 << 6;
        const CAN_MAP_READ          = 1 << 7;
        const CAN_MAP_WRITE         = 1 << 8;
        const CAN_MAP_EXECUTE       = 1 << 9;
        const MAP_RANGE             = 1 << 10;
        const REQUIRE_NON_RESIZABLE = 1 << 11;
        const ALLOW_FAULTS          = 1 << 12;
        const CAN_MAP_RXW           = Self::CAN_MAP_READ.bits | Self::CAN_MAP_EXECUTE.bits | Self::CAN_MAP_WRITE.bits;
    }
}

impl VmOptions {
    fn to_rights(self) -> Rights {
        let mut rights = Rights::empty();
        if self.contains(VmOptions::CAN_MAP_READ) {
            rights.insert(Rights::READ);
        }
        if self.contains(VmOptions::CAN_MAP_WRITE) {
            rights.insert(Rights::WRITE);
        }
        if self.contains(VmOptions::CAN_MAP_EXECUTE) {
            rights.insert(Rights::EXECUTE);
        }
        rights
    }

    fn to_flags(self) -> VmarFlags {
        let mut flags = VmarFlags::empty();
        if self.contains(VmOptions::COMPACT) {
            flags.insert(VmarFlags::COMPACT);
        }
        if self.contains(VmOptions::SPECIFIC) {
            flags.insert(VmarFlags::SPECIFIC);
        }
        if self.contains(VmOptions::SPECIFIC_OVERWRITE) {
            flags.insert(VmarFlags::SPECIFIC_OVERWRITE);
        }
        if self.contains(VmOptions::CAN_MAP_SPECIFIC) {
            flags.insert(VmarFlags::CAN_MAP_SPECIFIC);
        }
        if self.contains(VmOptions::CAN_MAP_READ) {
            flags.insert(VmarFlags::CAN_MAP_READ);
        }
        if self.contains(VmOptions::CAN_MAP_WRITE) {
            flags.insert(VmarFlags::CAN_MAP_WRITE);
        }
        if self.contains(VmOptions::CAN_MAP_EXECUTE) {
            flags.insert(VmarFlags::CAN_MAP_EXECUTE);
        }
        if self.contains(VmOptions::REQUIRE_NON_RESIZABLE) {
            flags.insert(VmarFlags::REQUIRE_NON_RESIZABLE);
        }
        if self.contains(VmOptions::ALLOW_FAULTS) {
            flags.insert(VmarFlags::ALLOW_FAULTS);
        }
        flags
    }
}
