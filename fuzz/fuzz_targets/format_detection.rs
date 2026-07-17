#![no_main]

use libfuzzer_sys::fuzz_target;
use zrimo_core::DocumentFormat;

fuzz_target!(|data: &[u8]| {
    if data.len() > 1_048_576 {
        return;
    }
    let extension = String::from_utf8_lossy(data);
    let _ = DocumentFormat::from_extension(&extension);
});
