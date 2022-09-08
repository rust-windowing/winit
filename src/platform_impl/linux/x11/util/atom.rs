use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    fmt::Debug,
    os::raw::*,
    sync::Mutex,
};

use once_cell::sync::Lazy;

use super::*;

type AtomCache = HashMap<CString, ffi::Atom>;

static ATOM_CACHE: Lazy<Mutex<AtomCache>> = Lazy::new(|| Mutex::new(HashMap::with_capacity(2048)));

impl XConnection {
    pub fn get_atom<T: AsRef<CStr> + Debug>(&self, name: T) -> ffi::Atom {
        let name = name.as_ref();
        let mut atom_cache_lock = ATOM_CACHE.lock().unwrap();
        let cached_atom = (*atom_cache_lock).get(name).cloned();
        if let Some(atom) = cached_atom {
            atom
        } else {
            let atom = unsafe {
                (self.xlib.XInternAtom)(self.display, name.as_ptr() as *const c_char, ffi::False)
            };
            if atom == 0 {
                panic!(
                    "`XInternAtom` failed, which really shouldn't happen. Atom: {:?}, Error: {:#?}",
                    name,
                    self.check_errors(),
                );
            }
            /*println!(
                "XInternAtom name:{:?} atom:{:?}",
                name,
                atom,
            );*/
            (*atom_cache_lock).insert(name.to_owned(), atom);
            atom
        }
    }

    pub unsafe fn get_atom_unchecked(&self, name: &[u8]) -> ffi::Atom {
        debug_assert!(CStr::from_bytes_with_nul(name).is_ok());
        let name = CStr::from_bytes_with_nul_unchecked(name);
        self.get_atom(name)
    }

    // Note: this doesn't use caching, for the sake of simplicity.
    // If you're dealing with this many atoms, you'll usually want to cache them locally anyway.
    pub unsafe fn get_atoms(&self, names: &[*mut c_char]) -> Result<Vec<ffi::Atom>, XError> {
        let mut atoms = Vec::with_capacity(names.len());
        (self.xlib.XInternAtoms)(
            self.display,
            names.as_ptr() as *mut _,
            names.len() as c_int,
            ffi::False,
            atoms.as_mut_ptr(),
        );
        self.check_errors()?;
        atoms.set_len(names.len());
        /*println!(
            "XInternAtoms atoms:{:?}",
            atoms,
        );*/
        Ok(atoms)
    }
}
