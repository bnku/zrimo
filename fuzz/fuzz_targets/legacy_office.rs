#![no_main]

use legacy_office_wasm::convert_legacy_bytes;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > 2 * 1024 * 1024 {
        return;
    }
    let format = ["doc", "xls", "ppt"][usize::from(data[0]) % 3];
    let _ = convert_legacy_bytes(&data[1..], format);
});
