use std::collections::{HashMap, HashSet};
use std::fmt;

const DEFAULT_INSTANCE_LIMIT: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Unknown(u32),
    Int,
    Float,
    Bool,
    Str,
    Array(Box<Type>),
}

impl Type {
    fn contains(&self, needle: u32) -> bool {
        match self {
            Self::Unknown(id) => *id == needle,
            Self::Array(element) => element.contains(needle),
            Self::Int | Self::Float | Self::Bool | Self::Str => false,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(id) => write!(formatter, "?{id}"),
            Self::Int => formatter.write_str("int"),
            Self::Float => formatter.write_str("float"),
            Self::Bool => formatter.write_str("bool"),
            Self::Str => formatter.write_str("str"),
            Self::Array(element) => write!(formatter, "{element}[]"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Expr {
    Int,
    Float,
    Bool,
    Str,
    Var(&'static str),
    Add(Box<Expr>, Box<Expr>),
    Array(Vec<Expr>),
    Append {
        array: &'static str,
        value: Box<Expr>,
    },
    Call {
        function: &'static str,
        arguments: Vec<Expr>,
    },
}

#[derive(Clone, Debug)]
pub enum Statement {
    Let {
        name: &'static str,
        annotation: Option<Type>,
        value: Expr,
    },
    Assign {
        name: &'static str,
        value: Expr,
    },
    Evaluate(Expr),
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: &'static str,
    pub parameters: Vec<&'static str>,
    pub body: Expr,
}

#[derive(Clone, Debug)]
pub struct Builtin {
    pub name: &'static str,
    pub parameters: Vec<Type>,
    pub result: Type,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InferenceError {
    UnknownVariable(&'static str),
    UnknownFunction(&'static str),
    Arity {
        function: &'static str,
        expected: usize,
        actual: usize,
    },
    Incompatible {
        expected: Type,
        actual: Type,
    },
    InfiniteType(u32),
    InstanceLimit {
        function: &'static str,
        limit: usize,
    },
    RecursiveCycle(&'static str),
    Unresolved(Type),
}

pub struct Inference {
    functions: HashMap<&'static str, Function>,
    builtins: HashMap<&'static str, Builtin>,
    locals: HashMap<&'static str, Type>,
    substitutions: HashMap<u32, Type>,
    instances: HashMap<&'static str, HashMap<Vec<Type>, Type>>,
    active_instances: HashSet<(&'static str, Vec<Type>)>,
    next_unknown: u32,
    instance_limit: usize,
}

impl Inference {
    pub fn new(functions: impl IntoIterator<Item = Function>) -> Self {
        Self {
            functions: functions
                .into_iter()
                .map(|function| (function.name, function))
                .collect(),
            builtins: HashMap::new(),
            locals: HashMap::new(),
            substitutions: HashMap::new(),
            instances: HashMap::new(),
            active_instances: HashSet::new(),
            next_unknown: 0,
            instance_limit: DEFAULT_INSTANCE_LIMIT,
        }
    }

    pub fn with_builtins(mut self, builtins: impl IntoIterator<Item = Builtin>) -> Self {
        self.builtins = builtins
            .into_iter()
            .map(|builtin| (builtin.name, builtin))
            .collect();
        self
    }

    pub fn infer_program(
        &mut self,
        statements: &[Statement],
    ) -> Result<HashMap<&'static str, Type>, InferenceError> {
        for statement in statements {
            self.infer_statement(statement)?;
        }

        let locals = self.locals.clone();
        locals
            .into_iter()
            .map(|(name, ty)| Ok((name, self.require_resolved(&ty)?)))
            .collect()
    }

    pub fn instance_count(&self, function: &'static str) -> usize {
        self.instances.get(function).map_or(0, HashMap::len)
    }

    fn infer_statement(&mut self, statement: &Statement) -> Result<(), InferenceError> {
        match statement {
            Statement::Let {
                name,
                annotation,
                value,
            } => {
                let inferred = self.infer_expr(value)?;
                let ty = if let Some(expected) = annotation {
                    self.accept_assignment(expected.clone(), inferred)?
                } else {
                    inferred
                };
                self.locals.insert(name, ty);
                Ok(())
            }
            Statement::Assign { name, value } => {
                let current = self
                    .locals
                    .get(name)
                    .cloned()
                    .ok_or(InferenceError::UnknownVariable(name))?;
                let value = self.infer_expr(value)?;
                let widened = self.join_types(current, value)?;
                self.locals.insert(name, widened);
                Ok(())
            }
            Statement::Evaluate(expression) => {
                self.infer_expr(expression)?;
                Ok(())
            }
        }
    }

    fn infer_expr(&mut self, expression: &Expr) -> Result<Type, InferenceError> {
        match expression {
            Expr::Int => Ok(Type::Int),
            Expr::Float => Ok(Type::Float),
            Expr::Bool => Ok(Type::Bool),
            Expr::Str => Ok(Type::Str),
            Expr::Var(name) => self
                .locals
                .get(name)
                .cloned()
                .ok_or(InferenceError::UnknownVariable(name)),
            Expr::Add(left, right) => {
                let inferred_left = self.infer_expr(left)?;
                let inferred_right = self.infer_expr(right)?;
                let left = self.resolve_fully(&inferred_left);
                let right = self.resolve_fully(&inferred_right);
                self.numeric_result(left, right)
            }
            Expr::Array(elements) => {
                let mut element_type = self.fresh_unknown();
                for element in elements {
                    let inferred = self.infer_expr(element)?;
                    element_type = self.join_types(element_type, inferred)?;
                }
                Ok(Type::Array(Box::new(self.resolve_fully(&element_type))))
            }
            Expr::Append { array, value } => {
                let array_type = self
                    .locals
                    .get(array)
                    .cloned()
                    .ok_or(InferenceError::UnknownVariable(array))?;
                let value_type = self.infer_expr(value)?;
                let expected_array = Type::Array(Box::new(value_type));
                let widened = self.join_types(array_type, expected_array)?;
                self.locals.insert(array, widened.clone());
                Ok(widened)
            }
            Expr::Call {
                function,
                arguments,
            } => self.infer_call(function, arguments),
        }
    }

    fn infer_call(
        &mut self,
        function_name: &'static str,
        arguments: &[Expr],
    ) -> Result<Type, InferenceError> {
        if let Some(builtin) = self.builtins.get(function_name).cloned() {
            return self.infer_builtin_call(&builtin, arguments);
        }

        let function = self
            .functions
            .get(function_name)
            .cloned()
            .ok_or(InferenceError::UnknownFunction(function_name))?;
        if function.parameters.len() != arguments.len() {
            return Err(InferenceError::Arity {
                function: function_name,
                expected: function.parameters.len(),
                actual: arguments.len(),
            });
        }

        let argument_types = arguments
            .iter()
            .map(|argument| self.infer_expr(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let key = argument_types
            .iter()
            .map(|ty| self.require_resolved(ty))
            .collect::<Result<Vec<_>, _>>()?;

        if let Some(result) = self
            .instances
            .get(function_name)
            .and_then(|instances| instances.get(&key))
        {
            return Ok(result.clone());
        }

        if self.instance_count(function_name) >= self.instance_limit {
            return Err(InferenceError::InstanceLimit {
                function: function_name,
                limit: self.instance_limit,
            });
        }

        let active_key = (function_name, key.clone());
        if !self.active_instances.insert(active_key.clone()) {
            return Err(InferenceError::RecursiveCycle(function_name));
        }

        let saved_locals = std::mem::take(&mut self.locals);
        self.locals = function
            .parameters
            .iter()
            .copied()
            .zip(argument_types)
            .collect();
        let result = self.infer_expr(&function.body);
        self.locals = saved_locals;
        self.active_instances.remove(&active_key);
        let result = self.require_resolved(&result?)?;

        self.instances
            .entry(function_name)
            .or_default()
            .insert(key, result.clone());
        Ok(result)
    }

    fn infer_builtin_call(
        &mut self,
        builtin: &Builtin,
        arguments: &[Expr],
    ) -> Result<Type, InferenceError> {
        if builtin.parameters.len() != arguments.len() {
            return Err(InferenceError::Arity {
                function: builtin.name,
                expected: builtin.parameters.len(),
                actual: arguments.len(),
            });
        }

        for (expected, argument) in builtin.parameters.iter().zip(arguments) {
            let actual = self.infer_expr(argument)?;
            self.accept_assignment(expected.clone(), actual)?;
        }
        Ok(builtin.result.clone())
    }

    fn numeric_result(&mut self, left: Type, right: Type) -> Result<Type, InferenceError> {
        match (left, right) {
            (Type::Int, Type::Int) => Ok(Type::Int),
            (Type::Int | Type::Float, Type::Int | Type::Float) => Ok(Type::Float),
            (Type::Unknown(id), numeric @ (Type::Int | Type::Float))
            | (numeric @ (Type::Int | Type::Float), Type::Unknown(id)) => {
                self.bind(id, numeric.clone())?;
                Ok(numeric)
            }
            (expected, actual) => Err(InferenceError::Incompatible { expected, actual }),
        }
    }

    fn accept_assignment(&mut self, expected: Type, actual: Type) -> Result<Type, InferenceError> {
        let expected = self.resolve_fully(&expected);
        let actual = self.resolve_fully(&actual);
        match (expected, actual) {
            (Type::Unknown(id), actual) => {
                self.bind(id, actual.clone())?;
                Ok(actual)
            }
            (expected, Type::Unknown(id)) => {
                self.bind(id, expected.clone())?;
                Ok(expected)
            }
            (Type::Float, Type::Int) => Ok(Type::Float),
            (Type::Array(expected), Type::Array(actual)) => Ok(Type::Array(Box::new(
                self.accept_assignment(*expected, *actual)?,
            ))),
            (expected, actual) if expected == actual => Ok(expected),
            (expected, actual) => Err(InferenceError::Incompatible { expected, actual }),
        }
    }

    fn join_types(&mut self, left: Type, right: Type) -> Result<Type, InferenceError> {
        let left = self.resolve_fully(&left);
        let right = self.resolve_fully(&right);
        match (left, right) {
            (Type::Unknown(id), right) => {
                self.bind(id, right.clone())?;
                Ok(right)
            }
            (left, Type::Unknown(id)) => {
                self.bind(id, left.clone())?;
                Ok(left)
            }
            (Type::Int, Type::Float) | (Type::Float, Type::Int) => Ok(Type::Float),
            (Type::Array(left), Type::Array(right)) => {
                Ok(Type::Array(Box::new(self.join_types(*left, *right)?)))
            }
            (left, right) if left == right => Ok(left),
            (expected, actual) => Err(InferenceError::Incompatible { expected, actual }),
        }
    }

    fn bind(&mut self, id: u32, ty: Type) -> Result<(), InferenceError> {
        let ty = self.resolve_fully(&ty);
        if ty == Type::Unknown(id) {
            return Ok(());
        }
        if ty.contains(id) {
            return Err(InferenceError::InfiniteType(id));
        }
        self.substitutions.insert(id, ty);
        Ok(())
    }

    fn require_resolved(&self, ty: &Type) -> Result<Type, InferenceError> {
        let resolved = self.resolve_fully(ty);
        if Self::is_resolved(&resolved) {
            Ok(resolved)
        } else {
            Err(InferenceError::Unresolved(resolved))
        }
    }

    fn is_resolved(ty: &Type) -> bool {
        match ty {
            Type::Unknown(_) => false,
            Type::Array(element) => Self::is_resolved(element),
            Type::Int | Type::Float | Type::Bool | Type::Str => true,
        }
    }

    fn resolve_fully(&self, ty: &Type) -> Type {
        match ty {
            Type::Unknown(id) => self
                .substitutions
                .get(id)
                .map_or_else(|| ty.clone(), |bound| self.resolve_fully(bound)),
            Type::Array(element) => Type::Array(Box::new(self.resolve_fully(element))),
            Type::Int | Type::Float | Type::Bool | Type::Str => ty.clone(),
        }
    }

    fn fresh_unknown(&mut self) -> Type {
        let id = self.next_unknown;
        self.next_unknown += 1;
        Type::Unknown(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> Function {
        Function {
            name: "identity",
            parameters: vec!["value"],
            body: Expr::Var("value"),
        }
    }

    #[test]
    fn infers_literal_binding() {
        let locals = Inference::new([]).infer_program(&[Statement::Let {
            name: "value",
            annotation: None,
            value: Expr::Int,
        }]);
        assert_eq!(locals.unwrap()["value"], Type::Int);
    }

    #[test]
    fn widens_int_binding_on_float_reassignment() {
        let locals = Inference::new([]).infer_program(&[
            Statement::Let {
                name: "value",
                annotation: None,
                value: Expr::Int,
            },
            Statement::Assign {
                name: "value",
                value: Expr::Float,
            },
        ]);
        assert_eq!(locals.unwrap()["value"], Type::Float);
    }

    #[test]
    fn rejects_non_numeric_reassignment() {
        let result = Inference::new([]).infer_program(&[
            Statement::Let {
                name: "value",
                annotation: None,
                value: Expr::Bool,
            },
            Statement::Assign {
                name: "value",
                value: Expr::Int,
            },
        ]);
        assert!(matches!(
            result,
            Err(InferenceError::Incompatible {
                expected: Type::Bool,
                actual: Type::Int
            })
        ));
    }

    #[test]
    fn infers_mixed_numeric_addition_as_float() {
        let locals = Inference::new([]).infer_program(&[Statement::Let {
            name: "value",
            annotation: None,
            value: Expr::Add(Box::new(Expr::Int), Box::new(Expr::Float)),
        }]);
        assert_eq!(locals.unwrap()["value"], Type::Float);
    }

    #[test]
    fn rejects_float_for_int_annotation() {
        let result = Inference::new([]).infer_program(&[Statement::Let {
            name: "value",
            annotation: Some(Type::Int),
            value: Expr::Float,
        }]);
        assert_eq!(
            result,
            Err(InferenceError::Incompatible {
                expected: Type::Int,
                actual: Type::Float
            })
        );
    }

    #[test]
    fn infers_numeric_function_body_for_each_instance() {
        let add_half = Function {
            name: "add_half",
            parameters: vec!["value"],
            body: Expr::Add(Box::new(Expr::Var("value")), Box::new(Expr::Float)),
        };
        let locals = Inference::new([add_half]).infer_program(&[Statement::Let {
            name: "result",
            annotation: None,
            value: Expr::Call {
                function: "add_half",
                arguments: vec![Expr::Int],
            },
        }]);
        assert_eq!(locals.unwrap()["result"], Type::Float);
    }

    #[test]
    fn creates_one_instance_for_repeated_argument_tuple() {
        let mut inference = Inference::new([identity()]);
        inference
            .infer_program(&[
                Statement::Evaluate(Expr::Call {
                    function: "identity",
                    arguments: vec![Expr::Int],
                }),
                Statement::Evaluate(Expr::Call {
                    function: "identity",
                    arguments: vec![Expr::Int],
                }),
            ])
            .unwrap();
        assert_eq!(inference.instance_count("identity"), 1);
    }

    #[test]
    fn monomorphizes_polymorphic_calls_per_argument_tuple() {
        let mut inference = Inference::new([identity()]);
        inference
            .infer_program(&[
                Statement::Evaluate(Expr::Call {
                    function: "identity",
                    arguments: vec![Expr::Int],
                }),
                Statement::Evaluate(Expr::Call {
                    function: "identity",
                    arguments: vec![Expr::Float],
                }),
            ])
            .unwrap();
        assert_eq!(inference.instance_count("identity"), 2);
    }

    #[test]
    fn rejects_ninth_monomorphization() {
        let function = Function {
            name: "identity",
            parameters: vec!["value"],
            body: Expr::Var("value"),
        };
        let mut inference = Inference::new([function]);
        let types = [
            Expr::Int,
            Expr::Float,
            Expr::Bool,
            Expr::Str,
            Expr::Array(vec![Expr::Int]),
            Expr::Array(vec![Expr::Float]),
            Expr::Array(vec![Expr::Bool]),
            Expr::Array(vec![Expr::Str]),
        ];
        for expression in types {
            inference
                .infer_program(&[Statement::Evaluate(Expr::Call {
                    function: "identity",
                    arguments: vec![expression],
                })])
                .unwrap();
        }
        let result = inference.infer_program(&[Statement::Evaluate(Expr::Call {
            function: "identity",
            arguments: vec![Expr::Array(vec![Expr::Array(vec![Expr::Int])])],
        })]);
        assert_eq!(
            result,
            Err(InferenceError::InstanceLimit {
                function: "identity",
                limit: 8
            })
        );
    }

    #[test]
    fn detects_recursive_inference_cycle() {
        let recursive = Function {
            name: "recursive",
            parameters: vec!["value"],
            body: Expr::Call {
                function: "recursive",
                arguments: vec![Expr::Var("value")],
            },
        };
        let result =
            Inference::new([recursive]).infer_program(&[Statement::Evaluate(Expr::Call {
                function: "recursive",
                arguments: vec![Expr::Int],
            })]);
        assert_eq!(result, Err(InferenceError::RecursiveCycle("recursive")));
    }

    #[test]
    fn infers_empty_array_from_annotation() {
        let locals = Inference::new([]).infer_program(&[Statement::Let {
            name: "items",
            annotation: Some(Type::Array(Box::new(Type::Int))),
            value: Expr::Array(vec![]),
        }]);
        assert_eq!(locals.unwrap()["items"], Type::Array(Box::new(Type::Int)));
    }

    #[test]
    fn infers_empty_array_from_later_append() {
        let locals = Inference::new([]).infer_program(&[
            Statement::Let {
                name: "items",
                annotation: None,
                value: Expr::Array(vec![]),
            },
            Statement::Evaluate(Expr::Append {
                array: "items",
                value: Box::new(Expr::Str),
            }),
        ]);
        assert_eq!(locals.unwrap()["items"], Type::Array(Box::new(Type::Str)));
    }

    #[test]
    fn infers_mixed_numeric_array_independent_of_order() {
        for elements in [vec![Expr::Int, Expr::Float], vec![Expr::Float, Expr::Int]] {
            let locals = Inference::new([]).infer_program(&[Statement::Let {
                name: "items",
                annotation: None,
                value: Expr::Array(elements),
            }]);
            assert_eq!(locals.unwrap()["items"], Type::Array(Box::new(Type::Float)));
        }
    }

    #[test]
    fn rejects_heterogeneous_array() {
        let result = Inference::new([]).infer_program(&[Statement::Let {
            name: "items",
            annotation: None,
            value: Expr::Array(vec![Expr::Int, Expr::Str]),
        }]);
        assert_eq!(
            result,
            Err(InferenceError::Incompatible {
                expected: Type::Int,
                actual: Type::Str
            })
        );
    }

    #[test]
    fn propagates_builtin_array_element_type_backward() {
        let consume_points = Builtin {
            name: "consume_points",
            parameters: vec![Type::Array(Box::new(Type::Float))],
            result: Type::Bool,
        };
        let locals = Inference::new([])
            .with_builtins([consume_points])
            .infer_program(&[
                Statement::Let {
                    name: "points",
                    annotation: None,
                    value: Expr::Array(vec![]),
                },
                Statement::Evaluate(Expr::Call {
                    function: "consume_points",
                    arguments: vec![Expr::Var("points")],
                }),
            ]);
        assert_eq!(
            locals.unwrap()["points"],
            Type::Array(Box::new(Type::Float))
        );
    }

    #[test]
    fn requires_annotation_for_unconstrained_empty_array() {
        let result = Inference::new([]).infer_program(&[Statement::Let {
            name: "items",
            annotation: None,
            value: Expr::Array(vec![]),
        }]);
        assert!(matches!(
            result,
            Err(InferenceError::Unresolved(Type::Array(_)))
        ));
    }
}
