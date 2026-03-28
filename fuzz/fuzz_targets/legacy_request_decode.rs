#![no_main]

use legacy_protocol::{
    decode_protocol_atomic_request, decode_protocol_count_request, decode_protocol_get_request,
    decode_protocol_search_continue, decode_protocol_search_start,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&selector, body)) = data.split_first() else {
        return;
    };

    match selector % 5 {
        0 => {
            let _ = decode_protocol_get_request(body);
        }
        1 => {
            let _ = decode_protocol_count_request(body);
        }
        2 => {
            let _ = decode_protocol_atomic_request(body);
        }
        3 => {
            let _ = decode_protocol_search_start(body);
        }
        _ => {
            let _ = decode_protocol_search_continue(body);
        }
    }
});
