//! Model class-body statements that no pass claims.
//!
//! `ModelBodyItem::Unknown` is a holding pen, not a verdict: broadcasts
//! (`broadcasts_to`), markers (`primary_abstract_class`), block-form
//! callbacks and class-scope constants are all fished out of it by later
//! recognizers. Whatever nobody claims never reaches the lowered output.
//! That used to be an emit-time warning, which `check` never prints: a
//! macro roundhouse does not model (a gem DSL, an app class method called
//! from the body) or an option a known macro declines checked with zero
//! errors while its behaviour was missing from the program (AGENTS.md
//! invariant 6).
//!
//! Every receiverless call (and `self.x = …` setting) left here is an
//! error now, unless it is
//! - claimed: a pass lowers exactly that shape ([`claimed`] names the
//!   pass for each entry and asks it by span where it declines some
//!   forms), or
//! - a known no-op on every target ([`NO_OP_CALLS`], with the reason).
//!
//! A call that already carries its own diagnostic (an unresolved
//! `include`) or a project extension's attribute macro (refused by
//! `analyze::diagnostics::attribute_macro_refusals`) is not reported a
//! second time.

use crate::app::App;
use crate::diagnostic::Diagnostic;
use crate::dialect::{Model, ModelBodyItem};
use crate::expr::{Expr, ExprNode, LValue, Literal};
use crate::schema::Schema;
use crate::span::Span;

/// Receiverless class-body calls with no effect on any target, each
/// with the reason it is safe to leave out.
pub(crate) const NO_OP_CALLS: &[(&str, &str)] = &[
    ("private", "a visibility marker for the `def`s after it; the method walk reads it"),
    ("protected", "a visibility marker for the `def`s after it; the method walk reads it"),
    ("public", "a visibility marker for the `def`s after it; the method walk reads it"),
    (
        "private_constant",
        "restricts constant access from outside the class; a program that runs on Rails never \
         makes that access, and no target has constant visibility",
    ),
    ("abstract!", "a sorbet annotation: types only, and the emitted tree has no sorbet-runtime"),
    ("interface!", "a sorbet annotation: types only, and the emitted tree has no sorbet-runtime"),
    ("final!", "a sorbet annotation: types only, and the emitted tree has no sorbet-runtime"),
    ("sealed!", "a sorbet annotation: types only, and the emitted tree has no sorbet-runtime"),
    ("sig", "a sorbet annotation: types only, and the emitted tree has no sorbet-runtime"),
    ("type_parameters", "a sorbet annotation: types only, and the emitted tree has no sorbet-runtime"),
];

/// `extend` of a module that only carries sorbet annotations.
const NO_OP_EXTENDS: &[&str] = &["T::Sig", "T::Helpers", "T::Generic"];

/// One unclaimed class-body statement.
pub(crate) struct Unclaimed {
    pub span: Span,
    pub name: String,
    /// Why a pass that knows the macro declined this form, when it says.
    pub reason: Option<String>,
}

/// The model's class-body statements no pass claims, in body order.
pub(crate) fn unclaimed_calls(model: &Model, schema: &Schema) -> Vec<Unclaimed> {
    let mut out = Vec::new();
    for item in &model.body {
        let ModelBodyItem::Unknown { expr, .. } = item else { continue };
        // Replayed where the source has it (`ingest::app_extension_calls`),
        // applied by `apply_inherited_visibility` and `analyze::private_calls`,
        // or a class macro `ingest::class_macros` refused and reports itself.
        if expr.decisions & (crate::expr::REPLAYED_CLASS_BODY | crate::expr::INHERITED_VISIBILITY | crate::expr::REFUSED_CLASS_MACRO) != 0 {
            continue;
        }
        let Some(name) = reported_name(expr) else { continue };
        if expr.diagnostic().is_some() || is_no_op(name, expr) || claimed(model, schema, name, expr) {
            continue;
        }
        let reason = match name {
            "friendly_id" => crate::lower::friendly_id::decline_reason(model),
            _ => None,
        };
        out.push(Unclaimed { span: expr.span, name: name.to_string(), reason });
    }
    out
}

/// One error per unclaimed statement of every model.
pub(crate) fn diagnostics(app: &App) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for model in &app.models {
        for call in unclaimed_calls(model, &app.schema) {
            if app.extensions.attribute_macro(&call.name).is_some() {
                continue;
            }
            let mut detail = format!(
                "`{}` in the body of `{}` is not modelled, so its effect would be dropped from the output",
                call.name,
                model.name.0.as_str()
            );
            if let Some(reason) = &call.reason {
                detail.push_str(": ");
                detail.push_str(reason);
            }
            out.push(Diagnostic::unsupported(call.span, None, call.name.as_str(), detail));
        }
    }
    out
}

/// The name a statement is reported under: a receiverless call, or a
/// `self.x = …` setting. Constants (claimed by
/// `analyze::extract_const_assignments`) and other statements are not
/// class-body calls.
fn reported_name(expr: &Expr) -> Option<&str> {
    if matches!(&*expr.node, ExprNode::Assign { target: LValue::Const { .. }, .. }) {
        return None;
    }
    let ExprNode::Send { recv, method, args, .. } = &*expr.node else { return None };
    let name = method.as_str();
    if let Some(recv) = recv {
        if !matches!(&*recv.node, ExprNode::SelfRef)
            || !name.ends_with('=')
            || matches!(name, "==" | "!=" | "<=" | ">=" | "===")
        {
            return None;
        }
        // Literal table settings are consumed by ingest::model's
        // explicit_class_setting; dynamic values remain unsupported.
        if matches!(name, "table_name=" | "table_name_prefix=")
            && args.len() == 1
            && matches!(&*args[0].node, ExprNode::Lit { value: Literal::Str { .. } | Literal::Sym { .. } })
        {
            return None;
        }
    }
    Some(name)
}

fn is_no_op(name: &str, expr: &Expr) -> bool {
    if NO_OP_CALLS.iter().any(|(n, _)| *n == name) {
        return true;
    }
    if name != "extend" {
        return false;
    }
    let ExprNode::Send { args, .. } = &*expr.node else { return false };
    let [arg] = args.as_slice() else { return false };
    let ExprNode::Const { path } = &*arg.node else { return false };
    let written = path.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("::");
    NO_OP_EXTENDS.contains(&written.as_str())
}

/// Whether a pass lowers this statement. Each entry names the pass that
/// consumes the shape and asks that pass's own recognizer, so the claim
/// follows the lowering instead of a copy of its shape.
fn claimed(model: &Model, schema: &Schema, name: &str, expr: &Expr) -> bool {
    let ExprNode::Send { args, .. } = &*expr.node else { return false };
    match name {
        // markers::push_attribute_api_methods synthesizes the accessors.
        "attr_accessor" | "attr_reader" | "attr_writer" => true,
        // lower::attached, a named attachment with only `dependent:`.
        "has_one_attached" | "has_many_attached" => crate::lower::attached::claims(expr),
        // lower::broadcasts, on the stream and option forms it reads.
        "broadcasts_to" => crate::lower::broadcasts::claims_broadcasts_to(args),
        // lower::typed_store's shared method synthesis.
        "typed_store" => true,
        // lower::secure_password's shared method synthesis.
        "has_secure_password" => true,
        // lower::secure_token, on the forms it expands.
        "has_secure_token" => crate::lower::secure_token::secure_token_decls(&model.body)
            .iter()
            .any(|d| d.span == expr.span),
        // lower::friendly_id, on the configurations it honors.
        "friendly_id" | "extend" => crate::lower::friendly_id::claims(model, expr.span),
        // lower::generates_token_for, on the forms it expands.
        "generates_token_for" => crate::lower::generates_token_for::claims(model, expr.span),
        // lower::has_json; a schema entry it cannot expand is declined.
        "has_json" => crate::lower::has_json::has_json_decls(&model.body)
            .iter()
            .any(|d| d.span == expr.span),
        // lower::serialize (`coder: JSON`, legacy positional `JSON`).
        "serialize" => crate::lower::serialize::serialize_decls(model).iter().any(|d| d.span == expr.span),
        // lower::default_scope, `default_scope { where(col: literal) }`.
        "default_scope" => crate::lower::default_scope::static_default_scope(model).is_some(),
        // lower::rich_text, the bare form only.
        "has_rich_text" => crate::lower::rich_text::rich_text_attrs(model)
            .iter()
            .any(|(span, _)| *span == expr.span),
        // lower::plain_text_attr, bare and with its table present.
        "has_markdown" => {
            crate::lower::plain_text_attr::record_table_present(schema)
                && crate::lower::plain_text_attr::plain_text_attrs(model)
                    .iter()
                    .any(|(span, _)| *span == expr.span)
        }
        // lower::normalizes, a one-parameter lambda over columns.
        "normalizes" => crate::lower::normalizes::claims(model, expr),
        // markers::push_attribute_api_methods, on `attribute_api_decl`'s form.
        "attribute" => super::markers::attribute_api_decl(expr).is_some(),
        // markers::is_abstract_class, the bare marker and a literal setting.
        "primary_abstract_class" => args.is_empty(),
        "abstract_class=" => super::markers::abstract_class_setting(expr).is_some(),
        // A model concern, spliced by the analyzer (`model_includes` +
        // `concern_model_items`); an unresolved one carries its own
        // diagnostic (skipped above).
        "include" => true,
        // markers::push_block_callback, on exactly the shapes it lowers.
        _ => super::markers::block_callback_shape(expr).is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ident::Symbol;

    fn send(method: &str, args: Vec<Expr>) -> Expr {
        Expr::new(
            Span::synthetic(),
            ExprNode::Send { recv: None, method: Symbol::from(method), args, block: None, parenthesized: false },
        )
    }

    fn sym(value: &str) -> Expr {
        Expr::new(Span::synthetic(), ExprNode::Lit { value: Literal::Sym { value: Symbol::from(value) } })
    }

    fn model(body: Vec<Expr>) -> Model {
        Model {
            name: crate::ident::ClassId(Symbol::from("Article")),
            parent: None,
            parent_span: Default::default(),
            table: crate::ident::TableRef(Symbol::from("articles")),
            primary_key: None,
            attributes: Default::default(),
            body: body
                .into_iter()
                .map(|expr| ModelBodyItem::Unknown { expr, leading_comments: vec![], leading_blank_line: false })
                .collect(),
            enums: Default::default(),
            enum_defaults: Default::default(),
            class_attr_defaults: Default::default(),
            lexical_json_shadow: false,
            sti_subclass_names: Vec::new(),
            i18n: Default::default(),
            span: Span::synthetic(),
        }
    }

    fn names(model: &Model) -> Vec<String> {
        unclaimed_calls(model, &Schema::default()).into_iter().map(|c| c.name).collect()
    }

    #[test]
    fn an_unknown_macro_is_unclaimed() {
        assert_eq!(names(&model(vec![send("positioned_by", vec![sym("position")])])), ["positioned_by"]);
    }

    #[test]
    fn no_op_calls_and_sorbet_extends_are_not_reported() {
        let t_sig = Expr::new(
            Span::synthetic(),
            ExprNode::Const { path: vec![Symbol::from("T"), Symbol::from("Sig")] },
        );
        let m = model(vec![send("private", vec![]), send("private_constant", vec![sym("X")]), send("extend", vec![t_sig])]);
        assert!(names(&m).is_empty());
    }

    #[test]
    fn every_no_op_entry_has_a_reason() {
        assert!(NO_OP_CALLS.iter().all(|(name, reason)| !name.is_empty() && reason.len() > 20));
    }

    #[test]
    fn an_attachment_option_that_is_not_lowered_is_unclaimed() {
        let hash = |key: &str| {
            Expr::new(
                Span::synthetic(),
                ExprNode::Hash { entries: vec![(sym(key), sym("x"))], kwargs: true },
            )
        };
        let m = model(vec![
            send("has_one_attached", vec![sym("avatar"), hash("dependent")]),
            send("has_one_attached", vec![sym("cover"), hash("service")]),
        ]);
        assert_eq!(names(&m), ["has_one_attached"]);
    }

    #[test]
    fn a_call_with_its_own_diagnostic_is_not_reported_again() {
        let mut include = send("positioned_by", vec![]);
        include.set_diagnostic(Some(crate::diagnostic::DiagnosticKind::Unsupported {
            target: None,
            construct: Symbol::from("positioned_by"),
            detail: String::new(),
        }));
        assert!(names(&model(vec![include])).is_empty());
    }

    #[test]
    fn a_class_macro_the_evaluator_refused_is_reported_by_it_alone() {
        let mut call = send("setting", vec![sym("name")]);
        call.decisions |= crate::expr::REFUSED_CLASS_MACRO;
        assert!(names(&model(vec![call, send("shout", vec![sym("title")])])) == ["shout"]);
    }
}
