//! Shared mutable cell containing data, with the mutability tied to the liveliness of a lifetime ScopedMutabilityOwner struct.
//!
//! Use `scoped_arc_cell` to create a matching `ArcCell` and `ScopedMutabilityOwner` pair. The data can be shared by cloning the `ArcCell`.
//!
//! As long as the `ScopedMutabilityOwner` is alive, mutating the inner data will succeed. Once the `ScopedMutabilityOwner` has been `drop`ped, the calls will fail.

#![warn(missing_docs)]

use crossbeam_utils::atomic::AtomicCell;
use std::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

/// Create a matching `ArcCell` and `ScopedMutabilityOwner` pair wrapping some value.
///
/// See crate-level documentation for mutability rules.
pub fn scoped_arc_cell<T: Copy>(val: T) -> (ArcCell<T>, ScopedMutabilityOwner<T>) {
    let aaaa = ScopedMutabilityOwner::new(val);
    (aaaa.create_reference(), aaaa)
}

/// Container for shared atomic interior mutability.
///
/// See crate-level documentation for mutability rules.
#[derive(Debug, Clone)]
pub struct ArcCell<T: Copy> {
    data: Arc<Data<T>>,
}

/// A type that owns the the mutability lifetime of the matching `ArcCell`.
///
/// See crate-level documentation for mutability rules.
#[derive(Debug)]
pub struct ScopedMutabilityOwner<T: Copy> {
    /// TODO(tangmi): Osspial, this is semantically an Arc<Mutex<Cell<_>>>, but implemented with AtomicCell for perf/avoiding deadlock issues?
    data: Arc<Data<T>>,
}

/// An error returned when trying to mutate a `ArcCell` that is read-only because the `ScopedMutabilityOwner` has already been `drop`ped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StoreError<T>(pub T);

#[derive(Debug)]
struct Data<T: Copy> {
    val: AtomicCell<T>,
    is_read_only: AtomicBool,
}

impl<T: Copy> ArcCell<T> {
    /// Replaces the contained value, and returns it.
    pub fn replace(&self, val: T) -> Result<T, StoreError<T>> {
        match self.data.is_read_only.load(Ordering::Acquire) {
            false => Ok(self.data.val.swap(val)),
            true => Err(StoreError(val)),
        }
    }

    /// Returns a copy of the contained value.
    pub fn get(&self) -> T {
        self.data.val.load()
    }

    /// Returns a raw pointer to the underlying data in this cell.
    pub fn as_ptr(&self) -> *mut T {
        self.data.val.as_ptr()
    }
}

/// Manually implement `PartialEq` to treat `Self` like just a `T`.
impl<T: Copy + PartialEq> PartialEq<ArcCell<T>> for ArcCell<T> {
    fn eq(&self, other: &ArcCell<T>) -> bool {
        // Note: does not compare `is_read_only` flag.
        self.data.val.load() == other.data.val.load()
    }
}

impl<T: Copy> ScopedMutabilityOwner<T> {
    fn new(val: T) -> ScopedMutabilityOwner<T> {
        let data = Arc::new(Data {
            val: AtomicCell::new(val),
            is_read_only: AtomicBool::new(false),
        });
        ScopedMutabilityOwner { data }
    }

    /// Create a new reference from the underlying data.
    ///
    /// The data will remain immutably alive even after this struct's lifetime.
    pub fn create_reference(&self) -> ArcCell<T> {
        ArcCell {
            data: self.data.clone(),
        }
    }
}

impl<T: Copy> Drop for ScopedMutabilityOwner<T> {
    fn drop(&mut self) {
        self.data.is_read_only.store(true, Ordering::Release);
    }
}

impl<T: Debug> Error for StoreError<T> {}

impl<T> Display for StoreError<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "the ScopedMutabilityOwner was destroyed, making this ArcCell read-only"
        )
    }
}
