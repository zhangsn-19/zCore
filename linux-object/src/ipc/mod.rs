//! Linux Inter-Process Communication
#![deny(missing_docs)]
mod semary;
mod shared_mem;

pub use self::semary::*;
pub use self::shared_mem::*;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use zircon_object::vm::*;

/// Semaphore table in a process
#[derive(Default)]
pub struct SemProc {
    /// Semaphore arrays
    arrays: BTreeMap<SemId, Arc<SemArray>>,
    /// Undo operations when process terminates
    undos: BTreeMap<(SemId, SemNum), SemOp>,
}

/// TODO: Remove hack
#[derive(Default)]
pub struct ShmProc {
    shm_identifiers: BTreeMap<ShmId, ShmIdentifier>,
}

/// Semaphore set identifier (in a process)
type SemId = usize;

/// Semaphore number (in an array)
type SemNum = u16;
type ShmId = usize;

/// Semaphore operation value
type SemOp = i16;

impl SemProc {
    /// Insert the `array` and return its ID
    pub fn add(&mut self, array: Arc<SemArray>) -> SemId {
        let id = self.get_free_id();
        self.arrays.insert(id, array);
        id
    }

    /// Remove an `array` by ID
    pub fn remove(&mut self, id: SemId) {
        self.arrays.remove(&id);
    }

    /// Get a free ID
    fn get_free_id(&self) -> SemId {
        (0..).find(|i| self.arrays.get(i).is_none()).unwrap()
    }

    /// Get an semaphore set by `id`
    pub fn get(&self, id: SemId) -> Option<Arc<SemArray>> {
        self.arrays.get(&id).cloned()
    }

    /// Add an undo operation
    pub fn add_undo(&mut self, id: SemId, num: SemNum, op: SemOp) {
        let old_val = *self.undos.get(&(id, num)).unwrap_or(&0);
        let new_val = old_val - op;
        self.undos.insert((id, num), new_val);
    }
}

/// Fork the semaphore table. Clear undo info.
impl Clone for SemProc {
    fn clone(&self) -> Self {
        SemProc {
            arrays: self.arrays.clone(),
            undos: BTreeMap::default(),
        }
    }
}

/// Auto perform semaphores undo on drop
impl Drop for SemProc {
    fn drop(&mut self) {
        for (&(id, num), &op) in self.undos.iter() {
            debug!("semundo: id: {}, num: {}, op: {}", id, num, op);
            let sem_array = self.arrays[&id].clone();
            let sem = &sem_array[num as usize];
            match op {
                1 => sem.release(),
                0 => {}
                _ => unimplemented!("Semaphore: semundo.(Not 1)"),
            }
        }
    }
}

impl ShmProc {
    /// Insert the `SharedGuard` and return its ID
    pub fn add(&mut self, shared_guard: Arc<spin::Mutex<Arc<VmObject>>>) -> ShmId {
        let id = self.get_free_id();
        let shm_identifier = ShmIdentifier {
            addr: 0,
            shared_guard,
        };
        self.shm_identifiers.insert(id, shm_identifier);
        id
    }

    /// Get a free ID
    fn get_free_id(&self) -> ShmId {
        (0..)
            .find(|i| self.shm_identifiers.get(i).is_none())
            .unwrap()
    }

    /// Get an semaphore set by `id`
    pub fn get(&self, id: ShmId) -> Option<ShmIdentifier> {
        self.shm_identifiers.get(&id).cloned()
    }

    /// Used to set Virtual Addr
    pub fn set(&mut self, id: ShmId, shm_id: ShmIdentifier) {
        self.shm_identifiers.insert(id, shm_id);
    }

    /// get id from virtaddr
    pub fn get_id(&self, addr: usize) -> Option<ShmId> {
        for (key, value) in &self.shm_identifiers {
            if value.addr == addr {
                return Some(*key);
            }
        }
        None
    }

    /// Pop Shared Area
    pub fn pop(&mut self, id: ShmId) {
        self.shm_identifiers.remove(&id);
    }
}

/// Fork the semaphore table. Clear undo info.
impl Clone for ShmProc {
    fn clone(&self) -> Self {
        ShmProc {
            shm_identifiers: self.shm_identifiers.clone(),
        }
    }
}
