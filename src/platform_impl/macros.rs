//! TODO!
//!
//! Yeah... This is complex!

macro_rules! platform {
    // Declare enums
    {
        $(#[$m:meta])*
        $v:vis enum $($t:tt)*
    } => {
        __platform_parse_until_curly_bracket! {
            (__platform_enum_out)
            ($(#[$m])* $v enum)
            ()
            ($($t)*)
        }
    };

    // Match on those enums
    (
        $(use $import:ident::__Platform__;)?
        match $($t:tt)*
    ) => {
        __platform_parse_until_curly_bracket! {
            (__platform_match_parse_inner)
            ($(use $import::__Platform__;)? match)
            ()
            ($($t)*)
        }
    };
}

macro_rules! __platform_parse_until_curly_bracket {
    {($macro_out:ident) ($($prefix:tt)*) ($($item:tt)*) ({ $($body:tt)* })} => {
        $macro_out! {
            $($prefix)* ($($item)*) {
                $($body)*
            }
        }
    };
    {($macro_out:ident) ($($prefix:tt)*) ($($item:tt)*) ($t:tt $($rest:tt)*)} => {
        __platform_parse_until_curly_bracket! {
            ($macro_out) ($($prefix)*) ($($item)* $t) ($($rest)*)
        }
    };
}

macro_rules! __platform_parse_until_fat_arrow {
    {($macro_out:ident) ($($prefix:tt)*) ($($item:tt)*) (=> $($body:tt)*)} => {
        $macro_out! {
            $($prefix)* {
                ($($item)*) => $($body)*
            }
        }
    };
    {($macro_out:ident) ($($prefix:tt)*) ($($item:tt)*) ($t:tt $($rest:tt)*)} => {
        __platform_parse_until_fat_arrow! {
            ($macro_out) ($($prefix)*) ($($item)* $t) ($($rest)*)
        }
    };
}

macro_rules! __platform_enum_out {
    (
        $(#[$m:meta])* $v:vis enum ($($enum:tt)*) {
            __Platform__$(($($t:tt)+))? $(,)?
        }
    ) => {
        $(#[$m])*
        $v enum $($enum)* {
            #[cfg(target_os = "ios")]
            Ios$((__platform_impl_replace!((platform_impl) () ($($t)+))))?,

            #[cfg(target_os = "macos")]
            Macos$((__platform_impl_replace!((platform_impl) () ($($t)+))))?,

            #[cfg(all(
                feature = "x11",
                any(
                    target_os = "linux",
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "netbsd",
                    target_os = "openbsd",
                ),
            ))]
            X11$((__platform_impl_replace!((platform_impl::x11) () ($($t)+))))?,

            #[cfg(all(
                feature = "wayland",
                any(
                    target_os = "linux",
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "netbsd",
                    target_os = "openbsd",
                ),
            ))]
            Wayland$((__platform_impl_replace!((platform_impl::wayland) () ($($t)+))))?,

            #[cfg(target_os = "windows")]
            Windows$((__platform_impl_replace!((platform_impl) () ($($t)+))))?,

            #[cfg(target_arch = "wasm32")]
            Web$((__platform_impl_replace!((platform_impl) () ($($t)+))))?,

            #[cfg(target_os = "android")]
            Android$((__platform_impl_replace!((platform_impl) () ($($t)+))))?,
        }
    };
}

macro_rules! __platform_match_parse_inner {
    (
        $(use $import:ident::__Platform__;)?
        match ($item:expr) {
            $($t:tt)*
        }
    ) => {
        __platform_parse_until_fat_arrow! {
            (__platform_match_out)
            ($(use $import::__Platform__;)? match ($item))
            ()
            ($($t)*)
        }
    };
}

macro_rules! __platform_match_out {
    (
        $(use $import:ident::__Platform__;)?
        match ($item:expr) {
            ($($p:tt)*) => $x:expr
            $(, _ => $fallback:expr)? $(,)?
        }
    ) => {
        match $item {
            #[cfg(target_os = "ios")]
            __platform_pattern_replace!((Ios) () ($($p)+)) => {
                $(use $import::Ios as __Platform__;)?
                $x
            }

            #[cfg(target_os = "macos")]
            __platform_pattern_replace!((Macos) () ($($p)+)) => {
                $(use $import::Macos as __Platform__;)?
                $x
            }

            #[cfg(all(
                feature = "x11",
                any(
                    target_os = "linux",
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "netbsd",
                    target_os = "openbsd",
                ),
            ))]
            __platform_pattern_replace!((X11) () ($($p)+)) => {
                $(use $import::X11 as __Platform__;)?
                $x
            }

            #[cfg(all(
                feature = "wayland",
                any(
                    target_os = "linux",
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "netbsd",
                    target_os = "openbsd",
                ),
            ))]
            __platform_pattern_replace!((Wayland) () ($($p)+)) => {
                $(use $import::Wayland as __Platform__;)?
                $x
            }

            #[cfg(target_os = "windows")]
            __platform_pattern_replace!((Windows) () ($($p)+)) => {
                $(use $import::Windows as __Platform__;)?
                $x
            }

            #[cfg(target_arch = "wasm32")]
            __platform_pattern_replace!((Web) () ($($p)+)) => {
                $(use $import::Web as __Platform__;)?
                $x
            }

            #[cfg(target_os = "android")]
            __platform_pattern_replace!((Android) () ($($p)+)) => {
                $(use $import::Android as __Platform__;)?
                $x
            }

            $(
                #[allow(unreachable_patterns)]
                _ => $fallback
            )?
        }
    };
}

/// Replace __Platform__ with the specified path
macro_rules! __platform_pattern_replace {
    {($($path:tt)*) ($($output:tt)*) (__Platform__ $($rest:tt)*)} => {
        __platform_pattern_replace! {
            ($($path)*)
            ($($output)* $($path)*)
            ($($rest)*)
        }
    };
    // Recurse into parentheses
    {($($path:tt)*) ($($output:tt)*) (( $($rest:tt)* ))} => {
        __platform_pattern_replace_in_group! {
            ($($path)*)
            ($($output)*)
            ()
            ()
            ($($rest)*)
        }
    };
    {($($path:tt)*) ($($output:tt)*) ($t:tt $($rest:tt)*)} => {
        __platform_pattern_replace! {
            ($($path)*)
            ($($output)* $t)
            ($($rest)*)
        }
    };
    {($($path:tt)*) ($($output:tt)*) ()} => {
        $($output)*
    };
}

macro_rules! __platform_pattern_replace_in_group {
    {($($path:tt)*) ($($output:tt)*) ($($parsed:tt)*) ($($item:tt)*) ($(,)?)} => {
        $($output)* (
            $($parsed)*
            __platform_pattern_replace!(($($path)*) () ($($item)*))
        )
    };
    {($($path:tt)*) ($($output:tt)*) ($($parsed:tt)*) ($($item:tt)*) (, $($rest:tt)*)} => {
        __platform_pattern_replace_in_group! {
            ($($path)*) ($($output)*) ($($parsed)* __platform_pattern_replace!(($($path)*) () ($($item)*)), ) () ($($rest)*)
        }
    };
    {($($path:tt)*) ($($output:tt)*) ($($parsed:tt)*) ($($item:tt)*) ($t:tt $($rest:tt)*)} => {
        __platform_pattern_replace_in_group! {
            ($($path)*) ($($output)*) ($($parsed)*) ($($item)* $t) ($($rest)*)
        }
    };
}

/// Replace platform_impl::__platform__ with the specified path
macro_rules! __platform_impl_replace {
    {($($path:tt)*) ($($output:tt)*) (platform_impl::__platform__ $($rest:tt)*)} => {
        __platform_impl_replace! {
            ($($path)*)
            ($($output)* $($path)*)
            ($($rest)*)
        }
    };
    {($($path:tt)*) ($($output:tt)*) ($t:tt $($rest:tt)*)} => {
        __platform_impl_replace! {
            ($($path)*)
            ($($output)* $t)
            ($($rest)*)
        }
    };
    {($($path:tt)*) ($($output:tt)*) ()} => {
        $($output)*
    };
}
