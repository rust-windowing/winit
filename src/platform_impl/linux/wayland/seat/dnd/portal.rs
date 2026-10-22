//! Resolve portal capabilities only after a drop, without blocking Wayland dispatch.
use crate::platform_impl::wayland::state::WinitState;
use sctk::reexports::calloop::channel;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

// Independent of UI deadlines: a slow or broken bus cannot accumulate background workers.
static WORKERS: AtomicUsize = AtomicUsize::new(0);
struct Worker;
impl Drop for Worker {
    fn drop(&mut self) {
        WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

fn key(bytes: &[u8]) -> Option<&str> {
    if bytes.is_empty() || bytes.len() > 4096 {
        return None;
    }
    // GTK serializes a string including its trailing C terminator; other sources omit it.
    let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    let key = std::str::from_utf8(bytes).ok()?;
    // GTK sends the key as UTF-8, with no URI decoding or shell interpretation.
    (!key.is_empty() && !key.contains('\0') && !key.chars().any(char::is_control)).then_some(key)
}

fn retrieve(key: &str) -> Option<Vec<PathBuf>> {
    let connection = zbus::blocking::connection::Builder::session()
        .ok()?
        .method_timeout(Duration::from_secs(5))
        .build()
        .ok()?;
    let proxy = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.portal.Documents",
        "/org/freedesktop/portal/documents",
        "org.freedesktop.portal.FileTransfer",
    )
    .ok()?;
    let options: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();
    let paths: Vec<String> = proxy.call("RetrieveFiles", &(key, options)).ok()?;
    if paths.is_empty()
        || paths.len() > 256
        || paths.iter().map(String::len).sum::<usize>() > super::uris::MAX_BYTES
    {
        return None;
    }
    paths
        .into_iter()
        .map(|p| {
            let path = PathBuf::from(&p);
            (path.is_absolute() && !p.contains('\0')).then_some(path)
        })
        .collect()
}

impl WinitState {
    pub(super) fn resolve_portal_drop(&mut self, id: u64, bytes: &[u8]) {
        let Some(key) = key(bytes).map(str::to_owned) else {
            self.complete_drop(id, None);
            return;
        };
        if WORKERS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < super::MAX_PENDING).then_some(n + 1)
            })
            .is_err()
        {
            self.complete_drop(id, None);
            return;
        }
        let worker = Worker;
        if let Some(pending) = self.dnd.pending.get_mut(&id) {
            pending.pipe = None;
        }
        let (sender, receiver) = channel::sync_channel(1);
        let Ok(token) = self.loop_handle.insert_source(receiver, move |event, _, state| {
            if let channel::Event::Msg(paths) = event {
                state.complete_drop(id, paths);
            }
        }) else {
            drop(worker);
            self.complete_drop(id, None);
            return;
        };
        // The existing deadline removes this channel even if the bus is unresponsive.
        if let Some(pending) = self.dnd.pending.get_mut(&id) {
            pending.pipe = Some(token);
        }
        if std::thread::Builder::new()
            .name("wayland-file-portal".into())
            .spawn(move || {
                let _worker = worker;
                let paths = retrieve(&key);
                if paths.is_none() {
                    tracing::warn!("Wayland file drop portal resolution failed");
                }
                let _ = sender.send(paths);
            })
            .is_err()
        {
            self.complete_drop(id, None);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn capability_keys_are_bounded_and_literal() {
        assert_eq!(super::key(b"123-abc"), Some("123-abc"));
        assert_eq!(super::key(b"123-abc\0"), Some("123-abc"));
        assert!(super::key(b"\0").is_none());
        assert!(super::key(b"abc\0\0").is_none());
        for invalid in [b"".as_slice(), b"a\0b", b"a\nb", &[255]] {
            assert!(super::key(invalid).is_none());
        }
        assert!(super::key(&vec![b'a'; 4097]).is_none());
    }
}
