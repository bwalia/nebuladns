#![no_main]

use libfuzzer_sys::fuzz_target;
use nebula_wire::Name;

fuzz_target!(|data: &[u8]| {
    let _ = Name::decode(data, 0);
});
