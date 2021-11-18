use super::*;

impl XConnection {
    pub fn get_atom(&self, name: &str) -> ffi::xcb_atom_t {
        let mut atom_cache_lock = self.atom_cache.lock();
        let cached_atom = (*atom_cache_lock).get(name).cloned();
        if let Some(atom) = cached_atom {
            atom
        } else {
            let atom = self.get_atom_uncached(name);
            (*atom_cache_lock).insert(name.to_owned(), atom);
            atom
        }
    }

    pub fn get_atom_uncached(&self, name: &str) -> ffi::xcb_atom_t {
        unsafe {
            let cookie = self
                .xcb
                .xcb_intern_atom(self.c, 0, name.len() as _, name.as_ptr() as _);
            let mut err = ptr::null_mut();
            let reply = self.xcb.xcb_intern_atom_reply(self.c, cookie, &mut err);
            match self.check(reply, err) {
                Ok(r) => r.atom,
                Err(e) => panic!("Could not intern the atom `{}`: {}", name, e),
            }
        }
    }
}
