use parking_lot::ReentrantMutex;
use std::ffi::OsString;

static ENV_LOCK: ReentrantMutex<()> = parking_lot::const_reentrant_mutex(());

pub fn reset_env() {
    for (var, _) in std::env::vars_os() {
        if let Some(s) = var.to_str() {
            if matches!(s, "HOME" | "PATH" | "X_PATH") {
                continue;
            }
        }
        std::env::remove_var(&var);
    }
}

pub fn set_env(var: &str, val: &str) -> impl Drop {
    let reset = Reset(var.to_string(), std::env::var_os(var), ENV_LOCK.lock());
    log::info!("Setting environment variable {} to {}", var, val);
    std::env::set_var(var, val);
    reset
}

struct Reset<T>(String, Option<OsString>, T);

impl<T> Drop for Reset<T> {
    fn drop(&mut self) {
        match &self.1 {
            Some(v) => {
                log::info!(
                    "Reset environment variable {} to {}",
                    self.0,
                    v.to_string_lossy()
                );
                std::env::set_var(&self.0, v);
            }
            _ => {
                log::info!("Unsetting environment variable {}", self.0);
                std::env::remove_var(&self.0);
            }
        }
    }
}
