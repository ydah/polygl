#![no_main]

mod common;

use libfuzzer_sys::fuzz_target;
use polygl_adapter_php::PhpAdapter;

fuzz_target!(|input: &[u8]| common::fuzz_frontend(&PhpAdapter, input));
