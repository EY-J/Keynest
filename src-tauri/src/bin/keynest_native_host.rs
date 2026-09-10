// Keep the console subsystem: Chromium owns redirected binary stdin/stdout.
// This executable never calls the desktop run() entry point or opens a vault.
fn main() {
    keynest_lib::run_native_host();
}
