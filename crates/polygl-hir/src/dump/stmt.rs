use crate::{Block, Place, PlaceKind, Stmt, StmtKind};

use super::{Dumper, type_name};

impl Dumper {
    pub(super) fn block(&mut self, block: &Block) {
        self.open();
        for statement in &block.statements {
            self.statement(statement);
        }
        self.close();
    }

    fn statement(&mut self, statement: &Stmt) {
        match &statement.kind {
            StmtKind::Let { name, ty, init } => {
                let ty = ty
                    .as_ref()
                    .map(|ty| format!(": {}", type_name(ty)))
                    .unwrap_or_default();
                let init = self.expression(init);
                self.line(format!("let {name}{ty} = {init};"));
            }
            StmtKind::Assign { target, value } => {
                let target = self.place(target);
                let value = self.expression(value);
                self.line(format!("{target} = {value};"));
            }
            StmtKind::Expr(expression) => {
                let expression = self.expression(expression);
                self.line(format!("{expression};"));
            }
            StmtKind::If {
                condition,
                then_block,
                else_block,
            } => {
                let condition = self.expression(condition);
                self.line(format!("if {condition}"));
                self.block(then_block);
                if let Some(else_block) = else_block {
                    self.line("else");
                    self.block(else_block);
                }
            }
            StmtKind::While { condition, body } => {
                let condition = self.expression(condition);
                self.line(format!("while {condition}"));
                self.block(body);
            }
            StmtKind::For {
                variable,
                range,
                body,
            } => {
                let delimiter = if range.inclusive { "..=" } else { ".." };
                let start = self.expression(&range.start);
                let end = self.expression(&range.end);
                self.line(format!("for {variable} in {start}{delimiter}{end}"));
                self.block(body);
            }
            StmtKind::Return(value) => {
                let value = value
                    .as_ref()
                    .map(|value| format!(" {}", self.expression(value)))
                    .unwrap_or_default();
                self.line(format!("return{value};"));
            }
            StmtKind::Break => self.line("break;"),
            StmtKind::Continue => self.line("continue;"),
        }
    }

    fn place(&self, place: &Place) -> String {
        match &place.kind {
            PlaceKind::Var(name) => name.to_string(),
            PlaceKind::Index { base, index } => {
                format!("{}[{}]", self.expression(base), self.expression(index))
            }
            PlaceKind::Field { base, field } => {
                format!("{}.{}", self.expression(base), field)
            }
        }
    }
}
