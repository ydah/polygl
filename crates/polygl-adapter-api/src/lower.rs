use polygl_hir::{BuiltinId, Module};
use polygl_span::{Diagnostics, SourceFile};

use crate::FeatureTag;

pub trait BuiltinResolver: Send + Sync {
    fn resolve_builtin(&self, canonical_name: &str) -> Option<BuiltinId>;
}

pub struct LowerCtx<'a> {
    builtins: &'a dyn BuiltinResolver,
}

impl<'a> LowerCtx<'a> {
    #[must_use]
    pub const fn new(builtins: &'a dyn BuiltinResolver) -> Self {
        Self { builtins }
    }

    #[must_use]
    pub fn resolve_builtin(&self, canonical_name: &str) -> Option<BuiltinId> {
        self.builtins.resolve_builtin(canonical_name)
    }
}

pub trait LanguageAdapter: Send + Sync {
    fn id(&self) -> &'static str;

    fn file_extensions(&self) -> &'static [&'static str];

    fn lower(&self, source: &SourceFile, context: &mut LowerCtx<'_>)
    -> Result<Module, Diagnostics>;

    fn capabilities(&self) -> &'static [FeatureTag];
}

#[cfg(test)]
mod tests {
    use polygl_hir::BuiltinId;

    use super::{BuiltinResolver, LowerCtx};

    struct Resolver;

    impl BuiltinResolver for Resolver {
        fn resolve_builtin(&self, canonical_name: &str) -> Option<BuiltinId> {
            (canonical_name == "triangle").then_some(BuiltinId::TRIANGLE)
        }
    }

    #[test]
    fn lower_context_resolves_only_canonical_builtins() {
        let context = LowerCtx::new(&Resolver);
        assert_eq!(
            context.resolve_builtin("triangle"),
            Some(BuiltinId::TRIANGLE)
        );
        assert_eq!(context.resolve_builtin("draw_triangle"), None);
    }
}
