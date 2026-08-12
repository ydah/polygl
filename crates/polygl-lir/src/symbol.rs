use std::collections::HashMap;

use crate::Module;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(u32);

impl SymbolId {
    #[must_use]
    pub(crate) const fn from_index(index: usize) -> Self {
        assert!(
            index <= u32::MAX as usize,
            "symbol table exceeds u32 capacity"
        );
        Self(index as u32)
    }

    #[must_use]
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SymbolKind {
    Function,
    Constant,
}

pub(crate) struct SymbolTable<'module> {
    names: Vec<(&'module str, SymbolKind)>,
    ids: HashMap<(&'module str, SymbolKind), SymbolId>,
}

impl<'module> SymbolTable<'module> {
    pub(crate) fn new(module: &'module Module) -> Self {
        let mut table = Self {
            names: Vec::with_capacity(module.functions.len() + module.constants.len()),
            ids: HashMap::with_capacity(module.functions.len() + module.constants.len()),
        };
        for function in &module.functions {
            table.declare(&function.name, SymbolKind::Function);
        }
        for constant in &module.constants {
            table.declare(&constant.name, SymbolKind::Constant);
        }
        table
    }

    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.names.len()
    }

    #[must_use]
    pub(crate) fn get(&self, name: &str, kind: SymbolKind) -> Option<SymbolId> {
        self.ids.get(&(name, kind)).copied()
    }

    #[must_use]
    pub(crate) fn require(&self, name: &str, kind: SymbolKind) -> SymbolId {
        self.get(name, kind)
            .expect("typed LIR contains only declared dependencies")
    }

    fn declare(&mut self, name: &'module str, kind: SymbolKind) {
        let id = SymbolId::from_index(self.names.len());
        let previous = self.ids.insert((name, kind), id);
        assert!(previous.is_none(), "typed LIR declarations are unique");
        self.names.push((name, kind));
    }
}

#[cfg(test)]
mod tests {
    use polygl_span::{SourceFile, SourceId};

    use crate::{Block, Constant, Domain, Function, Literal, Module};

    use super::{SymbolKind, SymbolTable};

    #[test]
    fn assigns_stable_ids_in_declaration_order_and_separates_namespaces() {
        let source = SourceFile::new(SourceId::new(1), "symbols", "x");
        let span = source.span(0, 1).unwrap();
        let module = Module {
            functions: vec![Function {
                name: "same".to_owned(),
                params: Vec::new(),
                result: polygl_types::Type::Unit,
                body: Block {
                    statements: Vec::new(),
                    span,
                },
                domain: Domain::Shared,
                span,
            }],
            structs: Vec::new(),
            constants: vec![Constant {
                name: "same".to_owned(),
                ty: polygl_types::Type::Int,
                value: crate::Expr::new(
                    crate::ExprKind::Literal(Literal::Int(1)),
                    polygl_types::Type::Int,
                    span,
                ),
                domain: Domain::Shared,
                span,
            }],
            entries: Vec::new(),
            span,
        };

        let symbols = SymbolTable::new(&module);
        let function = symbols.require("same", SymbolKind::Function);
        let constant = symbols.require("same", SymbolKind::Constant);
        assert_eq!(function.index(), 0);
        assert_eq!(constant.index(), 1);
        assert_ne!(function, constant);
    }
}
