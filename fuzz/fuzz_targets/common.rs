use polygl_adapter_api::{LanguageAdapter, LowerCtx};
use polygl_core::BuiltinTable;
use polygl_span::{SourceFile, SourceId};

pub fn fuzz_frontend(adapter: &dyn LanguageAdapter, input: &[u8]) {
    let Ok(source) = SourceFile::from_bytes(SourceId::new(0), "fuzz", input.to_vec()) else {
        return;
    };
    let mut context = LowerCtx::new(&BuiltinTable);
    let _ = adapter.lower(&source, &mut context);
}
