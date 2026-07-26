use std::collections::HashMap;

use polygl_builtins::{BuiltinTable, Domain as BuiltinDomain};

use crate::{Block, CallTarget, Domain, Expr, ExprKind, Module, Place, PlaceKind, StatementKind};

#[derive(Clone, Debug)]
struct NodeFacts {
    allowed: DomainSet,
    dependencies: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Node {
    Function(String),
    Constant(String),
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
    let mut facts = HashMap::new();
    for function in &module.functions {
        let mut node = NodeFacts {
            allowed: domain_set(function.domain),
            dependencies: Vec::new(),
        };
        inspect_block(&function.body, &mut node);
        facts.insert(Node::Function(function.name.clone()), node);
    }
    for constant in &module.constants {
        let mut node = NodeFacts {
            allowed: DomainSet::BOTH,
            dependencies: Vec::new(),
        };
        inspect_expr(&constant.value, &mut node);
        facts.insert(Node::Constant(constant.name.clone()), node);
    }

    loop {
        let previous = facts
            .iter()
            .map(|(node, facts)| (node.clone(), facts.allowed))
            .collect::<HashMap<_, _>>();
        let mut changed = false;
        for node in facts.values_mut() {
            let mut allowed = node.allowed;
            for dependency in &node.dependencies {
                allowed = allowed.intersect(
                    *previous
                        .get(dependency)
                        .expect("typed LIR contains only declared dependencies"),
                );
            }
            changed |= allowed != node.allowed;
            node.allowed = allowed;
        }
        if !changed {
            break;
        }
    }

    let mut usage = facts
        .keys()
        .map(|node| (node.clone(), DomainSet::NONE))
        .collect::<HashMap<_, _>>();
    for entry in &module.entries {
        let mut entry_facts = NodeFacts {
            allowed: DomainSet::BOTH,
            dependencies: Vec::new(),
        };
        inspect_block(&entry.body, &mut entry_facts);
        let entry_domain = domain_set(entry.domain);
        for dependency in entry_facts.dependencies {
            let used = usage
                .get_mut(&dependency)
                .expect("typed LIR contains only declared dependencies");
            *used = used.union(entry_domain);
        }
    }

    loop {
        let previous = usage.clone();
        let mut changed = false;
        for (node, node_facts) in &facts {
            let node_usage = previous[node];
            for dependency in &node_facts.dependencies {
                let used = usage
                    .get_mut(dependency)
                    .expect("typed LIR contains only declared dependencies");
                let next = used.union(node_usage);
                changed |= next != *used;
                *used = next;
            }
        }
        if !changed {
            break;
        }
    }

    for function in &mut module.functions {
        let node = Node::Function(function.name.clone());
        function.domain = resolved_domain(facts[&node].allowed, usage[&node], function.domain);
    }
    for constant in &mut module.constants {
        let node = Node::Constant(constant.name.clone());
        constant.domain = resolved_domain(facts[&node].allowed, usage[&node], Domain::Shared);
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

fn inspect_block(block: &Block, facts: &mut NodeFacts) {
    for statement in &block.statements {
        match &statement.kind {
            StatementKind::Let { init, .. } | StatementKind::Expr(init) => {
                inspect_expr(init, facts);
            }
            StatementKind::Assign { target, value } => {
                inspect_place(target, facts);
                inspect_expr(value, facts);
            }
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                inspect_expr(condition, facts);
                inspect_block(then_block, facts);
                if let Some(else_block) = else_block {
                    inspect_block(else_block, facts);
                }
            }
            StatementKind::While { condition, body } => {
                inspect_expr(condition, facts);
                inspect_block(body, facts);
            }
            StatementKind::For { range, body, .. } => {
                inspect_expr(&range.start, facts);
                inspect_expr(&range.end, facts);
                inspect_block(body, facts);
            }
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    inspect_expr(value, facts);
                }
            }
            StatementKind::Break | StatementKind::Continue => {}
        }
    }
}

fn inspect_place(place: &Place, facts: &mut NodeFacts) {
    match &place.kind {
        PlaceKind::Variable(_) => {}
        PlaceKind::Index { base, index } => {
            inspect_expr(base, facts);
            inspect_expr(index, facts);
        }
        PlaceKind::Field { base, .. } => inspect_expr(base, facts),
    }
}

fn inspect_expr(expression: &Expr, facts: &mut NodeFacts) {
    match &expression.kind {
        ExprKind::Literal(_) | ExprKind::Variable(_) => {}
        ExprKind::Uniform(_) => {
            facts.allowed = facts.allowed.intersect(DomainSet::GPU);
        }
        ExprKind::Constant(name) => facts.dependencies.push(Node::Constant(name.clone())),
        ExprKind::Binary { left, right, .. }
        | ExprKind::Index {
            base: left,
            index: right,
        } => {
            inspect_expr(left, facts);
            inspect_expr(right, facts);
        }
        ExprKind::Unary { operand, .. }
        | ExprKind::Field { base: operand, .. }
        | ExprKind::ArrayLength(operand)
        | ExprKind::IsNil(operand)
        | ExprKind::IsFalsy(operand) => inspect_expr(operand, facts),
        ExprKind::Call { target, args } => {
            match target {
                CallTarget::Function(name) => {
                    facts.dependencies.push(Node::Function(name.clone()));
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
                inspect_expr(argument, facts);
            }
        }
        ExprKind::Array(items) | ExprKind::Vector { args: items, .. } => {
            for item in items {
                inspect_expr(item, facts);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                inspect_expr(&entry.key, facts);
                inspect_expr(&entry.value, facts);
            }
        }
        ExprKind::Struct { fields, .. } => {
            for field in fields {
                inspect_expr(&field.value, facts);
            }
        }
    }
}
