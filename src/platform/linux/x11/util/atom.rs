use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::*;

use parking_lot::Mutex;

use super::*;

type AtomCache = HashMap<CString, ffi::Atom>;

lazy_static! {
    static ref ATOM_CACHE: Mutex<AtomCache> = Mutex::new(HashMap::with_capacity(2048));
}

pub unsafe fn get_atom(xconn: &Arc<XConnection>, name: &[u8]) -> Result<ffi::Atom, XError> {
    let name = CStr::from_bytes_with_nul_unchecked(name); // I trust you. Don't let me down.
    let mut atom_cache_lock = ATOM_CACHE.lock();
    let cached_atom = (*atom_cache_lock).get(name).cloned();
    if let Some(atom) = cached_atom {
        Ok(atom)
    } else {
        let atom = (xconn.xlib.XInternAtom)(
            xconn.display,
            name.as_ptr() as *const c_char,
            ffi::False,
        );
        /*println!(
            "XInternAtom name:{:?} atom:{:?}",
            name,
            atom,
        );*/
        xconn.check_errors()?;
        (*atom_cache_lock).insert(name.to_owned(), atom);
        Ok(atom)
    }
}

// Note: this doesn't use caching, for the sake of simplicity.
// If you're dealing with this many atoms, you'll usually want to cache them locally anyway.
pub unsafe fn get_atoms(
    xconn: &Arc<XConnection>,
    names: &[*mut c_char],
) -> Result<Vec<ffi::Atom>, XError> {
    let mut atoms = Vec::with_capacity(names.len());
    (xconn.xlib.XInternAtoms)(
        xconn.display,
        names.as_ptr() as *mut _,
        names.len() as c_int,
        ffi::False,
        atoms.as_mut_ptr(),
    );
    xconn.check_errors()?;
    atoms.set_len(names.len());
    /*println!(
        "XInternAtoms atoms:{:?}",
        atoms,
    );*/
    Ok(atoms)
}
