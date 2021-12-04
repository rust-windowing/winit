1. Build the required X11 module:

    ```
    cd x11-module
    meson build
    meson install -C build
    ```

   (Installs in `x11-module/install`.)

2. Run with `cargo run`

3. Logs are in the `testruns` directory.

# Troubleshooting

The plain Xorg binary is expected to be at `/usr/lib/Xorg`. (`/usr/bin/Xorg` is usually
a symlink to `/usr/lib/Xorg.wrap`.) If this is not correct set the environment variable
`X_PATH` to the correct path or edit the source code.
