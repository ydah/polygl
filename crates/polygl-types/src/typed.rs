use std::collections::HashMap;

use polygl_hir::Module;

use crate::Type;

#[derive(Clone, Debug)]
pub struct TypedModule {
    hir: Module,
    instance_counts: HashMap<String, usize>,
    instance_returns: HashMap<String, Type>,
}

impl TypedModule {
    pub(crate) fn new(
        hir: Module,
        instance_counts: HashMap<String, usize>,
        instance_returns: HashMap<String, Type>,
    ) -> Self {
        Self {
            hir,
            instance_counts,
            instance_returns,
        }
    }

    #[must_use]
    pub const fn as_hir(&self) -> &Module {
        &self.hir
    }

    #[must_use]
    pub fn into_hir(self) -> Module {
        self.hir
    }

    #[must_use]
    pub fn instance_count(&self, source_name: &str) -> usize {
        self.instance_counts.get(source_name).copied().unwrap_or(0)
    }

    #[must_use]
    pub fn instance_return_type(&self, instance_name: &str) -> Option<&Type> {
        self.instance_returns.get(instance_name)
    }
}
