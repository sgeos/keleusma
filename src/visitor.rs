//! AST visitor traits with default-implemented structural walks.
//!
//! Two traits cover the practical need across the runtime crate:
//!
//! - [`MutVisitor`] takes `&mut` references and is used by passes
//!   that mutate the AST in place. Examples: monomorphization
//!   rewrites, closure hoisting.
//! - [`Visitor`] takes `&` references and is used by passes that
//!   only inspect the AST. Examples: free-variable collection,
//!   target-feature validation.
//!
//! Each trait defines a `visit_*` hook for each AST node kind and a
//! corresponding `walk_*` default method that recurses into the
//! node's children. The default `visit_*` simply calls `walk_*`, so
//! a pass that does nothing structural produces a no-op traversal.
//! A pass overrides `visit_*` to insert its own logic at that node
//! kind. Inside an override, the pass calls `self.walk_*(node)` to
//! recurse, possibly before or after its own logic depending on
//! whether the pass needs pre-order or post-order semantics.
//!
//! Each pass typically implements the trait on a state struct that
//! holds the per-pass context (locals, specialization caches,
//! validation state, and so on). Method dispatch through the trait
//! lets each pass override only the node kinds it needs to act on,
//! keeping the structural recursion in a single place.

use crate::ast::*;

/// Mutable AST visitor. Implementors override the `visit_*` hooks
/// they need; the `walk_*` defaults handle structural recursion.
pub trait MutVisitor {
    fn visit_block(&mut self, block: &mut Block) {
        self.walk_block(block);
    }
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        self.walk_stmt(stmt);
    }
    fn visit_iterable(&mut self, it: &mut Iterable) {
        self.walk_iterable(it);
    }
    fn visit_expr(&mut self, expr: &mut Expr) {
        self.walk_expr(expr);
    }

    fn walk_block(&mut self, block: &mut Block) {
        for stmt in block.stmts.iter_mut() {
            self.visit_stmt(stmt);
        }
        if let Some(e) = block.tail_expr.as_mut() {
            self.visit_expr(e);
        }
    }

    fn walk_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Let(l) => self.visit_expr(&mut l.value),
            Stmt::For(f) => {
                self.visit_iterable(&mut f.iterable);
                self.visit_block(&mut f.body);
            }
            Stmt::Break(_) => {}
            Stmt::DataFieldAssign { value, .. } => self.visit_expr(value),
            Stmt::Expr(e) => self.visit_expr(e),
        }
    }

    fn walk_iterable(&mut self, it: &mut Iterable) {
        match it {
            Iterable::Range(s, e) => {
                self.visit_expr(s);
                self.visit_expr(e);
            }
            Iterable::Expr(e) => self.visit_expr(e),
        }
    }

    fn walk_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Literal { .. }
            | Expr::Ident { .. }
            | Expr::Placeholder { .. }
            | Expr::ClosureRef { .. } => {}
            Expr::BinOp { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::UnaryOp { operand, .. } => {
                self.visit_expr(operand);
            }
            Expr::Call { args, .. } => {
                for a in args.iter_mut() {
                    self.visit_expr(a);
                }
            }
            Expr::Pipeline { left, args, .. } => {
                self.visit_expr(left);
                for a in args.iter_mut() {
                    self.visit_expr(a);
                }
            }
            Expr::Yield { value, .. } => {
                self.visit_expr(value);
            }
            Expr::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                self.visit_expr(condition);
                self.visit_block(then_block);
                if let Some(b) = else_block.as_mut() {
                    self.visit_block(b);
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                self.visit_expr(scrutinee);
                for arm in arms.iter_mut() {
                    self.visit_expr(&mut arm.expr);
                }
            }
            Expr::Loop { body, .. } => {
                self.visit_block(body);
            }
            Expr::FieldAccess { object, .. } => {
                self.visit_expr(object);
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.visit_expr(receiver);
                for a in args.iter_mut() {
                    self.visit_expr(a);
                }
            }
            Expr::TupleIndex { object, .. } => {
                self.visit_expr(object);
            }
            Expr::ArrayIndex { object, index, .. } => {
                self.visit_expr(object);
                self.visit_expr(index);
            }
            Expr::StructInit { fields, .. } => {
                for f in fields.iter_mut() {
                    self.visit_expr(&mut f.value);
                }
            }
            Expr::EnumVariant { args, .. } => {
                for a in args.iter_mut() {
                    self.visit_expr(a);
                }
            }
            Expr::ArrayLiteral { elements, .. } | Expr::TupleLiteral { elements, .. } => {
                for e in elements.iter_mut() {
                    self.visit_expr(e);
                }
            }
            Expr::Cast { expr, .. } => {
                self.visit_expr(expr);
            }
            Expr::Closure { body, .. } => {
                self.visit_block(body);
            }
        }
    }
}

/// Immutable AST visitor. Same shape as [`MutVisitor`] but operates
/// on `&` references, suitable for passes that only inspect the AST.
pub trait Visitor {
    fn visit_block(&mut self, block: &Block) {
        self.walk_block(block);
    }
    fn visit_stmt(&mut self, stmt: &Stmt) {
        self.walk_stmt(stmt);
    }
    fn visit_iterable(&mut self, it: &Iterable) {
        self.walk_iterable(it);
    }
    fn visit_expr(&mut self, expr: &Expr) {
        self.walk_expr(expr);
    }

    fn walk_block(&mut self, block: &Block) {
        for stmt in block.stmts.iter() {
            self.visit_stmt(stmt);
        }
        if let Some(e) = block.tail_expr.as_ref() {
            self.visit_expr(e);
        }
    }

    fn walk_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(l) => self.visit_expr(&l.value),
            Stmt::For(f) => {
                self.visit_iterable(&f.iterable);
                self.visit_block(&f.body);
            }
            Stmt::Break(_) => {}
            Stmt::DataFieldAssign { value, .. } => self.visit_expr(value),
            Stmt::Expr(e) => self.visit_expr(e),
        }
    }

    fn walk_iterable(&mut self, it: &Iterable) {
        match it {
            Iterable::Range(s, e) => {
                self.visit_expr(s);
                self.visit_expr(e);
            }
            Iterable::Expr(e) => self.visit_expr(e),
        }
    }

    fn walk_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal { .. }
            | Expr::Ident { .. }
            | Expr::Placeholder { .. }
            | Expr::ClosureRef { .. } => {}
            Expr::BinOp { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::UnaryOp { operand, .. } => {
                self.visit_expr(operand);
            }
            Expr::Call { args, .. } => {
                for a in args.iter() {
                    self.visit_expr(a);
                }
            }
            Expr::Pipeline { left, args, .. } => {
                self.visit_expr(left);
                for a in args.iter() {
                    self.visit_expr(a);
                }
            }
            Expr::Yield { value, .. } => {
                self.visit_expr(value);
            }
            Expr::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                self.visit_expr(condition);
                self.visit_block(then_block);
                if let Some(b) = else_block.as_ref() {
                    self.visit_block(b);
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                self.visit_expr(scrutinee);
                for arm in arms.iter() {
                    self.visit_expr(&arm.expr);
                }
            }
            Expr::Loop { body, .. } => {
                self.visit_block(body);
            }
            Expr::FieldAccess { object, .. } => {
                self.visit_expr(object);
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.visit_expr(receiver);
                for a in args.iter() {
                    self.visit_expr(a);
                }
            }
            Expr::TupleIndex { object, .. } => {
                self.visit_expr(object);
            }
            Expr::ArrayIndex { object, index, .. } => {
                self.visit_expr(object);
                self.visit_expr(index);
            }
            Expr::StructInit { fields, .. } => {
                for f in fields.iter() {
                    self.visit_expr(&f.value);
                }
            }
            Expr::EnumVariant { args, .. } => {
                for a in args.iter() {
                    self.visit_expr(a);
                }
            }
            Expr::ArrayLiteral { elements, .. } | Expr::TupleLiteral { elements, .. } => {
                for e in elements.iter() {
                    self.visit_expr(e);
                }
            }
            Expr::Cast { expr, .. } => {
                self.visit_expr(expr);
            }
            Expr::Closure { body, .. } => {
                self.visit_block(body);
            }
        }
    }
}
