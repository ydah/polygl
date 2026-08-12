use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use polygl_adapter_api::LanguageAdapter;

#[derive(Default)]
pub struct AdapterRegistry<'adapter> {
    by_id: BTreeMap<&'static str, &'adapter dyn LanguageAdapter>,
    extensions: BTreeMap<&'static str, &'static str>,
    order: Vec<&'static str>,
}

impl<'adapter> AdapterRegistry<'adapter> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            by_id: BTreeMap::new(),
            extensions: BTreeMap::new(),
            order: Vec::new(),
        }
    }

    pub fn from_adapters(
        adapters: impl IntoIterator<Item = &'adapter dyn LanguageAdapter>,
    ) -> Result<Self, RegistryError> {
        let mut registry = Self::new();
        for adapter in adapters {
            registry.register(adapter)?;
        }
        Ok(registry)
    }

    pub fn register(
        &mut self,
        adapter: &'adapter dyn LanguageAdapter,
    ) -> Result<(), RegistryError> {
        let id = adapter.id();
        if !valid_component(id) {
            return Err(RegistryError::InvalidId(id));
        }
        if self.by_id.contains_key(id) {
            return Err(RegistryError::DuplicateId(id));
        }

        let mut claimed_extensions = BTreeSet::new();
        for extension in adapter.file_extensions() {
            if !valid_component(extension) {
                return Err(RegistryError::InvalidExtension {
                    adapter: id,
                    extension,
                });
            }
            if !claimed_extensions.insert(*extension) {
                return Err(RegistryError::DuplicateAdapterExtension {
                    adapter: id,
                    extension,
                });
            }
            if let Some(existing) = self.extensions.get(extension) {
                return Err(RegistryError::DuplicateExtension {
                    extension,
                    first: existing,
                    second: id,
                });
            }
        }

        self.by_id.insert(id, adapter);
        self.order.push(id);
        for extension in adapter.file_extensions() {
            self.extensions.insert(extension, id);
        }
        Ok(())
    }

    #[must_use]
    pub fn by_id(&self, id: &str) -> Option<&'adapter dyn LanguageAdapter> {
        self.by_id.get(id).copied()
    }

    #[must_use]
    pub fn for_extension(&self, extension: &str) -> Option<&'adapter dyn LanguageAdapter> {
        let extension = extension.strip_prefix('.').unwrap_or(extension);
        self.extensions
            .get(extension)
            .and_then(|id| self.by_id.get(id))
            .copied()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &'adapter dyn LanguageAdapter> + '_ {
        self.order.iter().map(|id| self.by_id[id])
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }
}

fn valid_component(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryError {
    InvalidId(&'static str),
    InvalidExtension {
        adapter: &'static str,
        extension: &'static str,
    },
    DuplicateId(&'static str),
    DuplicateAdapterExtension {
        adapter: &'static str,
        extension: &'static str,
    },
    DuplicateExtension {
        extension: &'static str,
        first: &'static str,
        second: &'static str,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(id) => write!(
                formatter,
                "adapter id `{id}` must contain lowercase ASCII letters and digits and start with a letter"
            ),
            Self::InvalidExtension { adapter, extension } => write!(
                formatter,
                "adapter `{adapter}` has invalid extension `{extension}`; extensions must not include a dot and must contain lowercase ASCII letters and digits"
            ),
            Self::DuplicateId(id) => write!(formatter, "adapter id `{id}` is registered twice"),
            Self::DuplicateAdapterExtension { adapter, extension } => write!(
                formatter,
                "adapter `{adapter}` lists extension `.{extension}` more than once"
            ),
            Self::DuplicateExtension {
                extension,
                first,
                second,
            } => write!(
                formatter,
                "extension `.{extension}` is claimed by both `{first}` and `{second}`"
            ),
        }
    }
}

impl Error for RegistryError {}

#[cfg(test)]
mod tests {
    use polygl_adapter_api::{FeatureTag, LanguageAdapter, LowerCtx};
    use polygl_hir::Module;
    use polygl_span::{Diagnostics, SourceFile};

    use super::{AdapterRegistry, RegistryError};

    struct TestAdapter {
        id: &'static str,
        extensions: &'static [&'static str],
    }

    impl LanguageAdapter for TestAdapter {
        fn id(&self) -> &'static str {
            self.id
        }

        fn file_extensions(&self) -> &'static [&'static str] {
            self.extensions
        }

        fn lower(
            &self,
            _source: &SourceFile,
            _context: &mut LowerCtx<'_>,
        ) -> Result<Module, Diagnostics> {
            unreachable!("registry tests do not lower source")
        }

        fn capabilities(&self) -> &'static [FeatureTag] {
            &[]
        }
    }

    static RUBY: TestAdapter = TestAdapter {
        id: "ruby",
        extensions: &["rb"],
    };
    static PERL: TestAdapter = TestAdapter {
        id: "perl",
        extensions: &["pl"],
    };
    static CONFLICT: TestAdapter = TestAdapter {
        id: "other",
        extensions: &["rb"],
    };
    static REPEATED: TestAdapter = TestAdapter {
        id: "repeated",
        extensions: &["repeat", "repeat"],
    };

    #[test]
    fn resolves_ids_and_extensions_in_stable_registration_order() {
        let registry = AdapterRegistry::from_adapters([
            &RUBY as &dyn LanguageAdapter,
            &PERL as &dyn LanguageAdapter,
        ])
        .unwrap();

        assert_eq!(registry.by_id("ruby").unwrap().id(), "ruby");
        assert_eq!(registry.for_extension(".pl").unwrap().id(), "perl");
        assert_eq!(
            registry.iter().map(LanguageAdapter::id).collect::<Vec<_>>(),
            ["ruby", "perl"]
        );
    }

    #[test]
    fn rejects_ambiguous_extensions_without_mutating_the_registry() {
        let mut registry = AdapterRegistry::from_adapters([
            &RUBY as &dyn LanguageAdapter,
            &PERL as &dyn LanguageAdapter,
        ])
        .unwrap();

        assert_eq!(
            registry.register(&CONFLICT),
            Err(RegistryError::DuplicateExtension {
                extension: "rb",
                first: "ruby",
                second: "other",
            })
        );
        assert!(registry.by_id("other").is_none());
        assert_eq!(registry.for_extension("rb").unwrap().id(), "ruby");

        assert_eq!(
            registry.register(&REPEATED),
            Err(RegistryError::DuplicateAdapterExtension {
                adapter: "repeated",
                extension: "repeat",
            })
        );
        assert!(registry.by_id("repeated").is_none());
    }
}
