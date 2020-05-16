//! Shared mutable datastructure, with the mutability tied to the liveliness of a owner struct.

use crossbeam_utils::atomic::AtomicCell;
use std::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

pub fn scoped_arc_cell<T: Copy>(val: T) -> (ScopedArcCell<T>, ScopedArcCellOwner<T>) {
    let owner = ScopedArcCellOwner::new(val);
    (owner.create_arc_cell(), owner)
}

#[derive(Debug, Clone)]
pub struct ScopedArcCell<T: Copy> {
    data: Arc<Data<T>>,
}

#[derive(Debug)]
pub struct ScopedArcCellOwner<T: Copy> {
    data: Arc<Data<T>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StoreError<T>(pub T);

#[derive(Debug)]
struct Data<T: Copy> {
    val: AtomicCell<T>,
    is_read_only: AtomicBool,
}

impl<T: Copy> ScopedArcCell<T> {
    pub fn store(&self, val: T) -> Result<(), StoreError<T>> {
        match self.data.is_read_only.load(Ordering::Acquire) {
            false => Ok(self.data.val.store(val)),
            true => Err(StoreError(val)),
        }
    }
    pub fn swap(&self, val: T) -> Result<T, StoreError<T>> {
        match self.data.is_read_only.load(Ordering::Acquire) {
            false => Ok(self.data.val.swap(val)),
            true => Err(StoreError(val)),
        }
    }

    pub fn load(&self) -> T {
        self.data.val.load()
    }

    pub fn as_ptr(&self) -> *mut T {
        self.data.val.as_ptr()
    }
}

impl<T: Copy> ScopedArcCellOwner<T> {
    pub fn new(val: T) -> ScopedArcCellOwner<T> {
        let data = Arc::new(Data {
            val: AtomicCell::new(val),
            is_read_only: AtomicBool::new(false),
        });
        ScopedArcCellOwner { data }
    }

    pub fn create_arc_cell(&self) -> ScopedArcCell<T> {
        ScopedArcCell {
            data: self.data.clone(),
        }
    }

    pub fn store(&self, val: T) {
        self.data.val.store(val)
    }

    pub fn swap(&self, val: T) -> T {
        self.data.val.swap(val)
    }

    pub fn load(&self) -> T {
        self.data.val.load()
    }

    pub fn as_ptr(&self) -> *mut T {
        self.data.val.as_ptr()
    }
}

impl<T: Copy> Drop for ScopedArcCellOwner<T> {
    fn drop(&mut self) {
        self.data.is_read_only.store(true, Ordering::Release);
    }
}

impl<T: Debug> Error for StoreError<T> {}
impl<T> Display for StoreError<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "the ScopedArcCellOwner was destroyed, making this ScopedArcCell read-only"
        )
    }
}
