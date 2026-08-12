#![no_main]

mod common;

use libfuzzer_sys::fuzz_target;
use polygl_adapter_api::LanguageAdapter;
use polygl_adapter_perl::PerlAdapter;
use polygl_adapter_php::PhpAdapter;
use polygl_adapter_ruby::RubyAdapter;

fuzz_target!(|input: &[u8]| {
    let Some((&language, source_bytes)) = input.split_first() else {
        return;
    };
    let adapter: &dyn LanguageAdapter = match language % 3 {
        0 => &RubyAdapter,
        1 => &PhpAdapter,
        _ => &PerlAdapter,
    };
    common::fuzz_frontend(adapter, source_bytes);
});
