//! Application diagnostics never format panic payloads, paths or request values.
use std::any::Any;

fn panic_message(_: &(dyn Any + Send)) -> &'static str {
    "KeyNest encountered an internal failure. Restart the application."
}

pub(crate) fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        // Deliberately do not call the default hook: it prints arbitrary payloads.
        eprintln!("{}", panic_message(info.payload()));
    }));
}

#[cfg(test)]
mod tests {
    #[test]
    fn panic_diagnostic_omits_arbitrary_payloads() {
        let sentinel = String::from("sentinel-master-recovery-vault-secret");
        let output = super::panic_message(&sentinel);
        assert!(!output.contains(&sentinel));
        assert_eq!(output, super::panic_message(&42_u32));
    }
}
