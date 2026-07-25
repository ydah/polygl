use crate::{Item, Module};

impl Module {
    /// Returns a canonical clone without changing executable or evaluation order.
    #[must_use]
    pub fn normalized(&self) -> Self {
        let mut normalized = self.clone();
        normalized.normalize();
        normalized
    }

    /// Canonicalizes top-level declaration order for Neutral HIR comparison.
    pub fn normalize(&mut self) {
        let mut constant_order = 0_usize;
        let mut keyed = self
            .items
            .drain(..)
            .map(|item| {
                let key = item_key(&item, &mut constant_order);
                (key, item)
            })
            .collect::<Vec<_>>();
        keyed.sort_by(|left, right| left.0.cmp(&right.0));
        self.items = keyed.into_iter().map(|(_, item)| item).collect();
    }
}

fn item_key(item: &Item, constant_order: &mut usize) -> (u8, usize, String) {
    match item {
        Item::Const(_) => {
            let order = *constant_order;
            *constant_order += 1;
            (0, order, String::new())
        }
        Item::Struct(item) => (1, 0, item.name.as_str().to_owned()),
        Item::Function(item) => (2, 0, item.name.as_str().to_owned()),
        Item::Entry(item) => (3, 0, item.kind.canonical_name()),
    }
}

#[cfg(test)]
mod tests {
    use polygl_span::{SourceFile, SourceId};

    use crate::{ConstDef, EntryPointKind, HirBuilder, Item, Symbol};

    #[test]
    fn normalization_is_idempotent_and_preserves_statement_order_and_spans() {
        let source = SourceFile::new(SourceId::new(1), "main.rb", "x");
        let span = source.span(0, 1).unwrap();
        let builder = HirBuilder::new(span);
        let first = builder.expression(builder.int(1));
        let second = builder.expression(builder.int(2));
        let constant_z = Item::Const(ConstDef {
            name: Symbol::from("z"),
            ty: None,
            value: builder.int(1),
            span,
        });
        let constant_a = Item::Const(ConstDef {
            name: Symbol::from("a"),
            ty: None,
            value: builder.int(2),
            span,
        });
        let entry = builder.entry(
            EntryPointKind::Setup,
            builder.block(vec![first.clone(), second.clone()]),
        );
        let module = builder.module(vec![entry, constant_z, constant_a]);

        let once = module.normalized();
        let twice = once.normalized();
        assert_eq!(once, twice);
        assert_eq!(once.span, span);
        let constant_names = once.items[0..2]
            .iter()
            .map(|item| match item {
                Item::Const(item) => item.name.as_str(),
                _ => panic!("expected constant"),
            })
            .collect::<Vec<_>>();
        assert_eq!(constant_names, ["z", "a"]);
        let crate::Item::Entry(entry) = &once.items[2] else {
            panic!("expected entry");
        };
        assert_eq!(entry.body.statements, [first, second]);
    }
}
