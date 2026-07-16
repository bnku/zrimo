#![no_main]

use image_wasm::fuzz_decode_tiff;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() > 2 * 1024 * 1024 {
        return;
    }
    let _ = fuzz_decode_tiff(data, 4_000_000);
});
