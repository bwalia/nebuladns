#![no_main]

use libfuzzer_sys::fuzz_target;
use nebula_wire::Question;

fuzz_target!(|data: &[u8]| {
    let _ = Question::decode(data, 0);
});
