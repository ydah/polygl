use polygl_builtins::{BuiltinTable, Domain as BuiltinDomain};

use crate::graph::DependencyGraph;
use crate::symbol::{SymbolKind, SymbolTable};
use crate::{
    Block, CallTarget, Domain, Expr, ExprKind, Module, Place, PlaceKind, StatementKind, SymbolId,
};

#[derive(Clone, Debug)]
struct NodeFacts {
    allowed: DomainSet,
    dependencies: Vec<SymbolId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DomainSet(u8);

impl DomainSet {
    const NONE: Self = Self(0);
    const HOST: Self = Self(1);
    const GPU: Self = Self(2);
    const BOTH: Self = Self(Self::HOST.0 | Self::GPU.0);

    const fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

pub(crate) fn resolve_domains(module: &mut Module) {
    let symbols = SymbolTable::new(module);
    let mut facts = vec![
        NodeFacts {
            allowed: DomainSet::BOTH,
            dependencies: Vec::new(),
        };
        symbols.len()
    ];
    for function in &module.functions {
        let mut node = NodeFacts {
            allowed: domain_set(function.domain),
            dependencies: Vec::new(),
        };
        inspect_block(&function.body, &symbols, &mut node);
        let id = symbols.require(&function.name, SymbolKind::Function);
        facts[id.index()] = node;
    }
    for constant in &module.constants {
        let mut node = NodeFacts {
            allowed: DomainSet::BOTH,
            dependencies: Vec::new(),
        };
        inspect_expr(&constant.value, &symbols, &mut node);
        let id = symbols.require(&constant.name, SymbolKind::Constant);
        facts[id.index()] = node;
    }

    let mut graph = DependencyGraph::new(symbols.len());
    for (index, node) in facts.iter().enumerate() {
        graph.set_dependencies(
            SymbolId::from_index(index),
            node.dependencies.iter().copied(),
        );
    }
    let components = graph.strongly_connected_components();

    for component in &components {
        let mut allowed = DomainSet::BOTH;
        for symbol in component {
            allowed = allowed.intersect(facts[symbol.index()].allowed);
            for dependency in graph.dependencies(*symbol) {
                if component.binary_search(dependency).is_err() {
                    allowed = allowed.intersect(facts[dependency.index()].allowed);
                }
            }
        }
        for symbol in component {
            facts[symbol.index()].allowed = allowed;
        }
    }

    let mut usage = vec![DomainSet::NONE; symbols.len()];
    for entry in &module.entries {
        let mut entry_facts = NodeFacts {
            allowed: DomainSet::BOTH,
            dependencies: Vec::new(),
        };
        inspect_block(&entry.body, &symbols, &mut entry_facts);
        let entry_domain = domain_set(entry.domain);
        for dependency in entry_facts.dependencies {
            usage[dependency.index()] = usage[dependency.index()].union(entry_domain);
        }
    }

    for component in components.iter().rev() {
        let component_usage = component.iter().fold(DomainSet::NONE, |used, symbol| {
            used.union(usage[symbol.index()])
        });
        for symbol in component {
            usage[symbol.index()] = component_usage;
        }
        for symbol in component {
            for dependency in graph.dependencies(*symbol) {
                if component.binary_search(dependency).is_err() {
                    usage[dependency.index()] = usage[dependency.index()].union(component_usage);
                }
            }
        }
    }

    let function_domains = module.functions.iter().map(|function| {
        let symbol = symbols.require(&function.name, SymbolKind::Function);
        resolved_domain(
            facts[symbol.index()].allowed,
            usage[symbol.index()],
            function.domain,
        )
    });
    let constant_domains = module.constants.iter().map(|constant| {
        let symbol = symbols.require(&constant.name, SymbolKind::Constant);
        resolved_domain(
            facts[symbol.index()].allowed,
            usage[symbol.index()],
            Domain::Shared,
        )
    });
    let function_domains = function_domains.collect::<Vec<_>>();
    let constant_domains = constant_domains.collect::<Vec<_>>();
    drop(symbols);
    for (function, domain) in module.functions.iter_mut().zip(function_domains) {
        function.domain = domain;
    }
    for (constant, domain) in module.constants.iter_mut().zip(constant_domains) {
        constant.domain = domain;
    }
}

const fn resolved_domain(allowed: DomainSet, usage: DomainSet, fallback: Domain) -> Domain {
    match allowed {
        DomainSet::HOST => Domain::Host,
        DomainSet::GPU => Domain::Gpu,
        DomainSet::BOTH => match usage {
            DomainSet::HOST => Domain::Host,
            DomainSet::GPU => Domain::Gpu,
            DomainSet::NONE | DomainSet::BOTH => Domain::Shared,
            _ => fallback,
        },
        DomainSet::NONE => match usage {
            DomainSet::HOST => Domain::Host,
            DomainSet::GPU => Domain::Gpu,
            DomainSet::NONE | DomainSet::BOTH => fallback,
            _ => fallback,
        },
        _ => fallback,
    }
}

const fn domain_set(domain: Domain) -> DomainSet {
    match domain {
        Domain::Host => DomainSet::HOST,
        Domain::Gpu => DomainSet::GPU,
        Domain::Shared => DomainSet::BOTH,
    }
}

fn inspect_block(block: &Block, symbols: &SymbolTable<'_>, facts: &mut NodeFacts) {
    for statement in &block.statements {
        match &statement.kind {
            StatementKind::Let { init, .. } | StatementKind::Expr(init) => {
                inspect_expr(init, symbols, facts);
            }
            StatementKind::Assign { target, value } => {
                inspect_place(target, symbols, facts);
                inspect_expr(value, symbols, facts);
            }
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                inspect_expr(condition, symbols, facts);
                inspect_block(then_block, symbols, facts);
                if let Some(else_block) = else_block {
                    inspect_block(else_block, symbols, facts);
                }
            }
            StatementKind::While { condition, body } => {
                inspect_expr(condition, symbols, facts);
                inspect_block(body, symbols, facts);
            }
            StatementKind::For { range, body, .. } => {
                inspect_expr(&range.start, symbols, facts);
                inspect_expr(&range.end, symbols, facts);
                inspect_block(body, symbols, facts);
            }
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    inspect_expr(value, symbols, facts);
                }
            }
            StatementKind::Break | StatementKind::Continue => {}
        }
    }
}

fn inspect_place(place: &Place, symbols: &SymbolTable<'_>, facts: &mut NodeFacts) {
    match &place.kind {
        PlaceKind::Variable(_) => {}
        PlaceKind::Index { base, index } => {
            inspect_expr(base, symbols, facts);
            inspect_expr(index, symbols, facts);
        }
        PlaceKind::Field { base, .. } => inspect_expr(base, symbols, facts),
    }
}

fn inspect_expr(expression: &Expr, symbols: &SymbolTable<'_>, facts: &mut NodeFacts) {
    match &expression.kind {
        ExprKind::Literal(_) | ExprKind::Variable(_) => {}
        ExprKind::Uniform(_) => {
            facts.allowed = facts.allowed.intersect(DomainSet::GPU);
        }
        ExprKind::Constant(name) => facts
            .dependencies
            .push(symbols.require(name, SymbolKind::Constant)),
        ExprKind::Binary { left, right, .. }
        | ExprKind::Index {
            base: left,
            index: right,
        } => {
            inspect_expr(left, symbols, facts);
            inspect_expr(right, symbols, facts);
        }
        ExprKind::Unary { operand, .. }
        | ExprKind::Field { base: operand, .. }
        | ExprKind::ArrayLength(operand)
        | ExprKind::IsNil(operand)
        | ExprKind::IsFalsy(operand) => inspect_expr(operand, symbols, facts),
        ExprKind::Call { target, args } => {
            match target {
                CallTarget::Function(name) => {
                    facts
                        .dependencies
                        .push(symbols.require(name, SymbolKind::Function));
                }
                CallTarget::Runtime(operation) => {
                    let builtin = BuiltinTable::all()
                        .iter()
                        .find(|builtin| builtin.runtime_op == *operation)
                        .expect("lowered runtime operations come from the builtin registry");
                    facts.allowed = facts.allowed.intersect(match builtin.domain {
                        BuiltinDomain::Host => DomainSet::HOST,
                        BuiltinDomain::Gpu => DomainSet::GPU,
                        BuiltinDomain::Both => DomainSet::BOTH,
                    });
                }
            }
            for argument in args {
                inspect_expr(argument, symbols, facts);
            }
        }
        ExprKind::Array(items) | ExprKind::Vector { args: items, .. } => {
            for item in items {
                inspect_expr(item, symbols, facts);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                inspect_expr(&entry.key, symbols, facts);
                inspect_expr(&entry.value, symbols, facts);
            }
        }
        ExprKind::Struct { fields, .. } => {
            for field in fields {
                inspect_expr(&field.value, symbols, facts);
            }
        }
    }
}
