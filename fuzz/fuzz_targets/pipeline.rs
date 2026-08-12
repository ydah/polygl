#![no_main]

use libfuzzer_sys::fuzz_target;
use polygl_adapter_api::{LanguageAdapter, LowerCtx};
use polygl_adapter_perl::PerlAdapter;
use polygl_adapter_php::PhpAdapter;
use polygl_adapter_ruby::RubyAdapter;
use polygl_backend_glsl::GlslBackend;
use polygl_backend_js::{BuildMode, JavaScriptBackend};
use polygl_core::BuiltinTable;
use polygl_span::{SourceFile, SourceId};

fuzz_target!(|input: &[u8]| {
    let Some((&language, source_bytes)) = input.split_first() else {
        return;
    };
    let adapter: &dyn LanguageAdapter = match language % 3 {
        0 => &RubyAdapter,
        1 => &PhpAdapter,
        _ => &PerlAdapter,
    };
    let Ok(source) = SourceFile::from_bytes(SourceId::new(0), "fuzz", source_bytes.to_vec()) else {
        return;
    };
    let mut context = LowerCtx::new(&BuiltinTable);
    let Ok(hir) = adapter.lower(&source, &mut context) else {
        return;
    };
    let Ok(typed) = polygl_types::analyze(&hir) else {
        return;
    };
    let lir = polygl_lir::lower(&typed);
    let Ok(split) = polygl_lir::split(&lir) else {
        return;
    };
    let _ = JavaScriptBackend::new(BuildMode::Release)
        .generate(&split.host, std::slice::from_ref(&source));
    let _ = GlslBackend::new().generate(&split.gpu);
});
