//! `record.errors&.full_messages` → `record.errors.full_messages`.
//!
//! `&.` binds a receiver that is not a plain read to a local first
//! (`(__safe_nav1 = record.errors).nil? ? nil : __safe_nav1.full_messages`, see
//! `ingest::expr`), and every `errors` lowering matches its receiver as
//! an `errors` send, so the local hid the site from all of them:
//! `full_messages` stayed a call on the Array of full messages. Rails'
//! `errors` is never nil, so the guard never fires, and the send reads
//! `errors` directly. `record&.errors&.m` keeps the guard on `record`:
//! `record.nil? ? nil : record.errors.m`.
//!
//! Typed: only an `errors` of `ActiveModel::Errors` folds, never an
//! app's own `errors` that may answer nil.

use crate::app::App;
use crate::expr::{Expr, ExprNode, LValue, Literal};
use crate::ty::Ty;

pub fn apply_errors_safe_nav_lowering(app: &mut App) {
    super::for_each_hook_body(app, &mut rewrite);
    super::for_each_test_body(app, &mut rewrite);
    for view in &mut app.views {
        rewrite(&mut view.body);
    }
}

fn is_model_errors(e: &Expr) -> bool {
    matches!(&*e.node, ExprNode::Send { method, args, block: None, .. } if method.as_str() == "errors" && args.is_empty())
        && matches!(&e.ty, Some(Ty::Class { id, .. }) if id.0.as_str() == "ActiveModel::Errors")
}

/// `cond` and the call branch of a `&.` site: `<subject>.nil? ? nil : <call>`.
fn safe_nav_parts(e: &mut Expr) -> Option<(&mut Expr, &mut Expr)> {
    let ExprNode::If { cond, then_branch, else_branch } = &mut *e.node else { return None };
    if !matches!(&*then_branch.node, ExprNode::Lit { value: Literal::Nil }) {
        return None;
    }
    let ExprNode::Send { recv: Some(_), method, args, block: None, .. } = &*cond.node else { return None };
    if method.as_str() != "nil?" || !args.is_empty() {
        return None;
    }
    Some((cond, else_branch))
}

fn rewrite(expr: &mut Expr) {
    expr.node.for_each_child_mut(&mut rewrite);
    let Some((cond, call)) = safe_nav_parts(expr) else { return };
    // `(t = <value>).nil? ? nil : t.m(...)`: the receiver was bound.
    let ExprNode::Send { recv: Some(subject), .. } = &*cond.node else { return };
    let ExprNode::Assign { target: LValue::Var { name: temp, .. }, value } = &*subject.node else { return };
    let ExprNode::Send { recv: Some(read), .. } = &*call.node else { return };
    if !matches!(&*read.node, ExprNode::Var { name, .. } if name == temp) {
        return;
    }
    let mut value = value.clone();
    let call = call.clone();
    if is_model_errors(&value) {
        let mut send = call;
        let ExprNode::Send { recv, .. } = &mut *send.node else { unreachable!() };
        *recv = Some(value);
        *expr = send;
        return;
    }
    // `record&.errors` is `record.nil? ? nil : record.errors`; the guard
    // stays on `record`.
    let Some((guard, errors)) = safe_nav_parts(&mut value) else { return };
    if !is_model_errors(errors) {
        return;
    }
    let (guard, errors) = (guard.clone(), errors.clone());
    let mut send = call;
    let ExprNode::Send { recv, .. } = &mut *send.node else { unreachable!() };
    *recv = Some(errors);
    let ExprNode::If { cond, else_branch, .. } = &mut *expr.node else { unreachable!() };
    *cond = guard;
    *else_branch = send;
}
