use std::{
    any::TypeId,
    cell::UnsafeCell,
    sync::{
        Arc,
        mpsc::{Receiver, Sender},
    },
};

use hashbrown::{HashMap, HashSet};
use parking_lot::{Mutex, RawRwLock, lock_api::RawRwLock as _};
use smallvec::SmallVec;

use crate::{HandleAny, StorageGeneral, WorldCommand};

/// new version of [`World`]
pub struct World {
    shared: Arc<WorldShare>,
    validator: UnsafeCell<WorldValidator>,
    queue: Receiver<WorldCommand>,
    commander: Sender<WorldCommand>,
}

pub struct WorldShare {
    syncing: RawRwLock,
    storage: UnsafeCell<WorldStorage>,
    /// lock by 1. thread switch view 2. inserting ElemRef/ViewRef
    threads: Mutex<WorldLock>,
}

/// shared, only available in flush process
struct WorldStorage {
    // TODO dependencies will be in indices
    indices: HashMap<HandleAny, HandleIndex>,
    tables: HashMap<TypeId, Box<dyn StorageGeneral>>,
}

struct HandleIndex {
    tid: TypeId,
    view: HandleAny,
    parents: SmallVec<[HandleAny; 1]>,
    children: Vec<HandleAny>,
    elemrefs: Vec<HandleAny>,
    viewrefs: SmallVec<[HandleAny; 1]>,
}

/// shared, available as long as no other thread trying to switch view state
struct WorldLock {
    /// by ElemRef
    elem: HashMap<HandleAny, isize>,
    /// by view entry or ViewRef
    view: HashMap<HandleAny, isize>,
}

/// per thread local
#[derive(Default)]
struct WorldValidator {
    occupied: HashMap<HandleAny, isize>,
    inserted: HashSet<HandleAny>,
    removed: HashSet<HandleAny>,
}

impl World {
    fn sync(&mut self) {
        self.release(|world| {
            world.shared.syncing.lock_exclusive();
            let storage = world.shared.storage.get();
            // TODO payload here
            unsafe { world.shared.syncing.unlock_exclusive() };
        })
    }

    fn wait_sync(&mut self) {
        self.release(|_| {})
    }

    fn release<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        unsafe { self.shared.syncing.unlock_shared() };
        let ret = f(self);
        self.shared.syncing.lock_shared();
        ret
    }
}

impl Drop for World {
    fn drop(&mut self) {
        unsafe { self.shared.syncing.unlock_shared() };
    }
}

impl WorldShare {}
