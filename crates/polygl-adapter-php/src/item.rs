use mago_span::HasSpan;
use mago_syntax::cst::{
    Constant as PhpConstant, Function as PhpFunction, FunctionLikeParameterList,
    FunctionLikeReturnTypeHint,
};
use polygl_adapter_api::canonical_entry_kind;
use polygl_hir::{
    ConstDef, DomainHint, EntryPoint, Function, Item, Param, Symbol, TypeExpr, TypeKind,
};
use polygl_span::{Diagnostic, Severity, Suggestion};

use crate::lowerer::Lowerer;

impl Lowerer<'_, '_, '_> {
    pub(crate) fn lower_constants(&mut self, constant: &PhpConstant<'_>) -> Option<Vec<Item>> {
        if !constant.attribute_lists.is_empty() {
            self.unsupported(
                constant.span(),
                "attributes on PHP constants are outside Common Core",
                "remove the attribute and keep a plain top-level `const` declaration",
            );
            return None;
        }
        let mut result = Vec::new();
        for item in constant.items.iter() {
            let name = self.name(item.name.value);
            result.push(Item::Const(ConstDef {
                name: Symbol::new(name),
                ty: None,
                value: self.lower_expression(item.value)?,
                span: self.span(item.span()),
            }));
        }
        Some(result)
    }

    pub(crate) fn lower_function(&mut self, function: &PhpFunction<'_>) -> Option<Item> {
        if !function.attribute_lists.is_empty() || function.ampersand.is_some() {
            self.unsupported(
                function.span(),
                "attributes and reference returns are outside Common Core",
                "remove the attributes/reference marker",
            );
            return None;
        }
        let name = self.name(function.name.value);
        let params = self.lower_parameters(
            &function.parameter_list,
            function.span().start_offset() as usize,
        )?;
        let return_type = self.lower_return_hint(function.return_type_hint.as_ref())?;
        self.declared = params
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect();
        let body = self.lower_block(&function.body);
        self.declared.clear();
        let span = self.span(function.span());
        match canonical_entry_kind(&name) {
            Some(kind) => Some(Item::Entry(EntryPoint {
                kind,
                params,
                return_type,
                body,
                span,
            })),
            None => Some(Item::Function(Function {
                name: Symbol::new(name),
                params,
                return_type,
                body,
                span,
                domain: DomainHint::Auto,
            })),
        }
    }

    pub(crate) fn lower_parameters(
        &mut self,
        parameter_list: &FunctionLikeParameterList<'_>,
        declaration_offset: usize,
    ) -> Option<Vec<Param>> {
        let mut result = Vec::new();
        for parameter in parameter_list.parameters.iter() {
            if !parameter.attribute_lists.is_empty()
                || !parameter.modifiers.is_empty()
                || parameter.ampersand.is_some()
                || parameter.ellipsis.is_some()
                || parameter.default_value.is_some()
                || parameter.hooks.is_some()
            {
                self.unsupported(
                    parameter.span(),
                    "only required positional value parameters are supported",
                    "remove attributes, defaults, references, variadics, and promoted-property modifiers",
                );
                return None;
            }
            let name = self.variable_name(parameter.variable.name);
            let native = parameter
                .hint
                .as_ref()
                .and_then(|hint| self.lower_native_hint(hint));
            let annotation = self
                .annotations
                .take(&name, declaration_offset, self.source);
            let ty = match (native, annotation) {
                (Some(native), Some(annotation)) if native.kind != annotation.kind => {
                    self.diagnostics.push(
                        Diagnostic::new(
                            Severity::Error,
                            "E0303",
                            "the @pgl annotation conflicts with the native PHP type hint",
                            annotation.span,
                        )
                        .with_suggestion(Suggestion::new(
                            annotation.span,
                            "",
                            "remove the redundant conflicting @pgl annotation",
                        )),
                    );
                    Some(native)
                }
                (Some(native), _) => Some(native),
                (None, annotation) => annotation,
            };
            result.push(Param {
                name: Symbol::new(name),
                ty,
                span: self.span(parameter.span()),
            });
        }
        Some(result)
    }

    pub(crate) fn lower_native_hint(
        &mut self,
        hint: &mago_syntax::cst::Hint<'_>,
    ) -> Option<TypeExpr> {
        let kind = match hint {
            mago_syntax::cst::Hint::Float(_) => TypeKind::Float,
            mago_syntax::cst::Hint::Bool(_) => TypeKind::Bool,
            mago_syntax::cst::Hint::Integer(_) => TypeKind::Int,
            mago_syntax::cst::Hint::String(_) => TypeKind::Str,
            mago_syntax::cst::Hint::Identifier(identifier)
                if identifier
                    .last_segment()
                    .first()
                    .is_some_and(u8::is_ascii_uppercase) =>
            {
                TypeKind::Struct(Symbol::new(self.name(identifier.last_segment())))
            }
            _ => {
                self.unsupported(
                    hint.span(),
                    "this PHP type hint has no Common Core value type",
                    "use int, float, bool, string, or an @pgl annotation",
                );
                return None;
            }
        };
        Some(TypeExpr::new(kind, self.span(hint.span())))
    }

    pub(crate) fn lower_return_hint(
        &mut self,
        hint: Option<&FunctionLikeReturnTypeHint<'_>>,
    ) -> Option<Option<TypeExpr>> {
        hint.map_or(Some(None), |hint| {
            if matches!(hint.hint, mago_syntax::cst::Hint::Void(_)) {
                return Some(Some(TypeExpr::new(
                    TypeKind::Unit,
                    self.span(hint.hint.span()),
                )));
            }
            self.lower_native_hint(&hint.hint).map(Some)
        })
    }
}
