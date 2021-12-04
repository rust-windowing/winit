enum OsTarget {
    Other(String),
    Windows,
    Macos,
    Linux,
    Dragonfly,
    Freebsd,
    Netbsd,
    Openbsd,
}

impl OsTarget {
    fn from_env() -> Self {
        Self::from_string(std::env::var("CARGO_CFG_TARGET_OS").unwrap())
    }

    fn from_string(s: String) -> Self {
        use OsTarget::*;
        match &*s {
            "windows" => Windows,
            "macos" => Macos,
            "linux" => Linux,
            "dragonfly" => Dragonfly,
            "freebsd" => Freebsd,
            "netbsd" => Netbsd,
            "openbsd" => Openbsd,
            _ => Other(s),
        }
    }
}

fn main() {
    use OsTarget::*;

    let os_target = OsTarget::from_env();
    if matches!(
        os_target,
        Windows | Macos | Linux | Dragonfly | Freebsd | Netbsd | Openbsd
    ) {
        println!("cargo:rustc-cfg=have_mod_supplement");
    }
}
