#![no_main]

use libfuzzer_sys::fuzz_target;
use nebula_wire::Header;

fuzz_target!(|data: &[u8]| {
    // Must never panic on arbitrary bytes.
    let _ = Header::decode(data);
});
