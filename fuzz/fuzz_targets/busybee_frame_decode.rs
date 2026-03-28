#![no_main]

use hyperdex_admin_protocol::BusyBeeFrame;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(frame) = BusyBeeFrame::decode(data) {
        let _ = frame.encode();
    }
});
