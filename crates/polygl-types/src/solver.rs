use std::collections::{HashMap, HashSet};
use std::fmt;

use polygl_hir::{OpaqueType, Symbol};

use crate::Type;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InferType {
    Var(u32),
    Unit,
    Int,
    Float,
    Bool,
    Str,
    Array(Box<Self>),
    Map(Box<Self>),
    Option(Box<Self>),
    Struct(Symbol),
    Vector(u8),
    Matrix(u8),
    Opaque(OpaqueType),
    Error,
}

impl InferType {
    fn contains(&self, needle: u32) -> bool {
        match self {
            Self::Var(id) => *id == needle,
            Self::Array(value) | Self::Map(value) | Self::Option(value) => value.contains(needle),
            Self::Unit
            | Self::Int
            | Self::Float
            | Self::Bool
            | Self::Str
            | Self::Struct(_)
            | Self::Vector(_)
            | Self::Matrix(_)
            | Self::Opaque(_)
            | Self::Error => false,
        }
    }

    pub(crate) fn from_type(ty: &Type) -> Self {
        match ty {
            Type::Unit => Self::Unit,
            Type::Int => Self::Int,
            Type::Float => Self::Float,
            Type::Bool => Self::Bool,
            Type::Str => Self::Str,
            Type::Array(element) => Self::Array(Box::new(Self::from_type(element))),
            Type::Map(value) => Self::Map(Box::new(Self::from_type(value))),
            Type::Option(value) => Self::Option(Box::new(Self::from_type(value))),
            Type::Struct(name) => Self::Struct(name.clone()),
            Type::Vector(size) => Self::Vector(*size),
            Type::Matrix(size) => Self::Matrix(*size),
            Type::Opaque(kind) => Self::Opaque(*kind),
        }
    }
}

impl fmt::Display for InferType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Var(id) => write!(formatter, "?{id}"),
            Self::Unit => formatter.write_str("void"),
            Self::Int => formatter.write_str("int"),
            Self::Float => formatter.write_str("float"),
            Self::Bool => formatter.write_str("bool"),
            Self::Str => formatter.write_str("str"),
            Self::Array(element) => write!(formatter, "{element}[]"),
            Self::Map(value) => write!(formatter, "Map<str, {value}>"),
            Self::Option(value) => write!(formatter, "Option<{value}>"),
            Self::Struct(name) => name.fmt(formatter),
            Self::Vector(size) => write!(formatter, "vec{size}"),
            Self::Matrix(size) => write!(formatter, "mat{size}"),
            Self::Opaque(kind) => write!(formatter, "{kind:?}"),
            Self::Error => formatter.write_str("<error>"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SolveError {
    Mismatch {
        expected: InferType,
        actual: InferType,
    },
    Infinite(u32),
    Unresolved(InferType),
}

#[derive(Default)]
pub(crate) struct Solver {
    substitutions: HashMap<u32, InferType>,
    fixed_variables: HashSet<u32>,
    numeric_variables: HashSet<u32>,
    next_variable: u32,
}

impl Solver {
    pub(crate) fn fresh(&mut self) -> InferType {
        let id = self.next_variable;
        self.next_variable += 1;
        InferType::Var(id)
    }

    pub(crate) fn resolve(&self, ty: &InferType) -> InferType {
        match ty {
            InferType::Var(id) => self
                .substitutions
                .get(id)
                .map_or_else(|| ty.clone(), |bound| self.resolve(bound)),
            InferType::Array(value) => InferType::Array(Box::new(self.resolve(value))),
            InferType::Map(value) => InferType::Map(Box::new(self.resolve(value))),
            InferType::Option(value) => InferType::Option(Box::new(self.resolve(value))),
            _ => ty.clone(),
        }
    }

    pub(crate) fn join(
        &mut self,
        left: InferType,
        right: InferType,
    ) -> Result<InferType, SolveError> {
        match (left, right) {
            (InferType::Error, _) | (_, InferType::Error) => Ok(InferType::Error),
            (InferType::Var(id), right) => self.bind_join(id, right),
            (left, InferType::Var(id)) => self.bind_join(id, left),
            (InferType::Int, InferType::Float) | (InferType::Float, InferType::Int) => {
                Ok(InferType::Float)
            }
            (InferType::Array(left), InferType::Array(right)) => {
                Ok(InferType::Array(Box::new(self.equal(*left, *right)?)))
            }
            (InferType::Map(left), InferType::Map(right)) => {
                Ok(InferType::Map(Box::new(self.equal(*left, *right)?)))
            }
            (InferType::Option(left), InferType::Option(right)) => {
                Ok(InferType::Option(Box::new(self.equal(*left, *right)?)))
            }
            (InferType::Option(inner), value) | (value, InferType::Option(inner)) => {
                Ok(InferType::Option(Box::new(self.equal(*inner, value)?)))
            }
            (left, right) if left == right => Ok(left),
            (expected, actual) => Err(SolveError::Mismatch { expected, actual }),
        }
    }

    pub(crate) fn assign(
        &mut self,
        expected: InferType,
        actual: InferType,
    ) -> Result<InferType, SolveError> {
        match (expected, actual) {
            (InferType::Error, _) | (_, InferType::Error) => Ok(InferType::Error),
            (InferType::Var(id), actual) => self.bind_join(id, actual),
            (expected, InferType::Var(id)) => self.constrain_variable(id, expected),
            (InferType::Float, InferType::Int) => Ok(InferType::Float),
            (InferType::Array(expected), InferType::Array(actual)) => {
                Ok(InferType::Array(Box::new(self.equal(*expected, *actual)?)))
            }
            (InferType::Map(expected), InferType::Map(actual)) => {
                Ok(InferType::Map(Box::new(self.equal(*expected, *actual)?)))
            }
            (InferType::Option(expected), InferType::Option(actual)) => {
                Ok(InferType::Option(Box::new(self.equal(*expected, *actual)?)))
            }
            (InferType::Option(expected), actual) => {
                Ok(InferType::Option(Box::new(self.assign(*expected, actual)?)))
            }
            (expected, actual) if expected == actual => Ok(expected),
            (expected, actual) => Err(SolveError::Mismatch { expected, actual }),
        }
    }

    pub(crate) fn equal(
        &mut self,
        left: InferType,
        right: InferType,
    ) -> Result<InferType, SolveError> {
        let left = self.resolve(&left);
        let right = self.resolve(&right);
        match (left, right) {
            (InferType::Error, _) | (_, InferType::Error) => Ok(InferType::Error),
            (InferType::Var(id), right) => self.bind_equal(id, right),
            (left, InferType::Var(id)) => self.bind_equal(id, left),
            (InferType::Array(left), InferType::Array(right)) => {
                Ok(InferType::Array(Box::new(self.equal(*left, *right)?)))
            }
            (InferType::Map(left), InferType::Map(right)) => {
                Ok(InferType::Map(Box::new(self.equal(*left, *right)?)))
            }
            (InferType::Option(left), InferType::Option(right)) => {
                Ok(InferType::Option(Box::new(self.equal(*left, *right)?)))
            }
            (left, right) if left == right => Ok(left),
            (expected, actual) => Err(SolveError::Mismatch { expected, actual }),
        }
    }

    pub(crate) fn reassign(
        &mut self,
        expected: InferType,
        actual: InferType,
    ) -> Result<InferType, SolveError> {
        if let InferType::Var(id) = expected
            && self.fixed_variables.contains(&id)
        {
            return self.assign(self.resolve(&InferType::Var(id)), actual);
        }
        let resolved_expected = self.resolve(&expected);
        let resolved_actual = self.resolve(&actual);
        if matches!(
            (&resolved_expected, &resolved_actual),
            (InferType::Int, InferType::Float)
                | (InferType::Float, InferType::Int)
                | (InferType::Var(_), InferType::Int | InferType::Float)
                | (InferType::Int | InferType::Float, InferType::Var(_))
        ) {
            self.join(expected, actual)
        } else if matches!(resolved_expected, InferType::Option(_))
            && !matches!(resolved_actual, InferType::Option(_))
        {
            self.assign(expected, actual)
        } else {
            self.equal(expected, actual)
        }
    }

    pub(crate) fn require_numeric(&mut self, ty: &InferType) -> Result<(), SolveError> {
        match self.resolve(ty) {
            InferType::Int | InferType::Float | InferType::Error => Ok(()),
            InferType::Var(id) => {
                self.numeric_variables.insert(id);
                Ok(())
            }
            actual => Err(SolveError::Mismatch {
                expected: InferType::Float,
                actual,
            }),
        }
    }

    pub(crate) fn mark_fixed(&mut self, ty: &InferType) {
        if let InferType::Var(id) = ty {
            self.fixed_variables.insert(*id);
        }
    }

    pub(crate) fn resolve_expression(&self, ty: &InferType) -> Result<Type, SolveError> {
        self.to_type(self.resolve(ty))
    }

    fn to_type(&self, ty: InferType) -> Result<Type, SolveError> {
        let _ = self.next_variable;
        match ty {
            InferType::Var(_) => Err(SolveError::Unresolved(ty)),
            InferType::Unit => Ok(Type::Unit),
            InferType::Int => Ok(Type::Int),
            InferType::Float => Ok(Type::Float),
            InferType::Bool => Ok(Type::Bool),
            InferType::Str => Ok(Type::Str),
            InferType::Array(element) => Ok(Type::Array(Box::new(self.to_type(*element)?))),
            InferType::Map(value) => Ok(Type::Map(Box::new(self.to_type(*value)?))),
            InferType::Option(value) => Ok(Type::Option(Box::new(self.to_type(*value)?))),
            InferType::Struct(name) => Ok(Type::Struct(name)),
            InferType::Vector(size) => Ok(Type::Vector(size)),
            InferType::Matrix(size) => Ok(Type::Matrix(size)),
            InferType::Opaque(kind) => Ok(Type::Opaque(kind)),
            InferType::Error => Ok(Type::Unit),
        }
    }

    fn bind_join(&mut self, id: u32, ty: InferType) -> Result<InferType, SolveError> {
        let ty = self.resolve(&ty);
        if ty == InferType::Var(id) {
            return Ok(ty);
        }
        if ty.contains(id) {
            return Err(SolveError::Infinite(id));
        }
        self.enforce_numeric_binding(id, &ty)?;
        let joined = if let Some(current) = self.substitutions.get(&id).cloned() {
            self.join(current, ty)?
        } else {
            ty
        };
        self.substitutions.insert(id, joined.clone());
        Ok(joined)
    }

    fn constrain_variable(
        &mut self,
        id: u32,
        expected: InferType,
    ) -> Result<InferType, SolveError> {
        let expected = self.resolve(&expected);
        self.enforce_numeric_binding(id, &expected)?;
        if self.fixed_variables.contains(&id)
            && let Some(current) = self.substitutions.get(&id).cloned()
        {
            return self.assign(expected, current);
        }
        let constrained = if let Some(current) = self.substitutions.get(&id).cloned() {
            self.assign(expected, current)?
        } else {
            expected
        };
        self.substitutions.insert(id, constrained.clone());
        Ok(constrained)
    }

    fn bind_equal(&mut self, id: u32, ty: InferType) -> Result<InferType, SolveError> {
        if ty == InferType::Var(id) {
            return Ok(ty);
        }
        if ty.contains(id) {
            return Err(SolveError::Infinite(id));
        }
        self.enforce_numeric_binding(id, &ty)?;
        if let Some(current) = self.substitutions.get(&id).cloned() {
            return self.equal(current, ty);
        }
        self.substitutions.insert(id, ty.clone());
        Ok(ty)
    }

    fn enforce_numeric_binding(&mut self, id: u32, ty: &InferType) -> Result<(), SolveError> {
        if !self.numeric_variables.contains(&id) {
            return Ok(());
        }
        match ty {
            InferType::Var(other) => {
                self.numeric_variables.insert(*other);
                Ok(())
            }
            InferType::Int | InferType::Float | InferType::Error => Ok(()),
            actual => Err(SolveError::Mismatch {
                expected: InferType::Float,
                actual: actual.clone(),
            }),
        }
    }
}
