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
//! - a known no-op on every target (visibility keywords, sorbet annotations).
//!
//! A call that already carries its own diagnostic (an unresolved
//! `include`) or a project extension's attribute macro (refused by
//! `analyze::diagnostics::attribute_macro_refusals`) is not reported a
//! second time.

use crate::app::App;
use crate::diagnostic::Diagnostic;
use crate::dialect::Model;
use crate::expr::{ExprNode, Literal};
use crate::schema::Schema;
use crate::span::Span;

/// One unclaimed class-body statement.
pub(crate) struct Unclaimed {
    pub span: Span,
    pub name: String,
    /// Why a pass that knows the macro declined this form, when it says.
    pub reason: Option<String>,
}

/// The model's class-body statements no pass claims, in body order.
pub(crate) fn unclaimed_calls(model: &Model, schema: &Schema) -> Vec<Unclaimed> {
    use crate::diagnostic::{Diagnostic, Severity};
    use crate::expr::LValue;
    let mut out = Vec::new();

    for item in &model.body {
        let crate::dialect::ModelBodyItem::Unknown { expr, .. } = item else { continue };
        // Replayed where the source has it (`ingest::app_extension_calls`),
        // or a class macro `ingest::class_macros` refused and reports itself.
        if expr.decisions & crate::expr::REPLAYED_CLASS_BODY != 0 || model.refused_class_macros.contains(&expr.span) {
            continue;
        }
        // An unresolved `include` and the like carry their own diagnostic.
        if expr.diagnostic().is_some() {
            continue;
        }
        // `prepend Concern`: the concern splice carried the module's
        // `prepended do` items here and the Ruby emit writes the line.
        if expr.decisions & crate::expr::CONCERN_PREPEND != 0 {
            continue;
        }
        // `connects_to` / `establish_connection`: the model's database,
        // decided (or refused) by `ingest::database_ownership`.
        if crate::ingest::database_ownership::claims_model_statement(expr) {
            continue;
        }
        // Claimed by `apply_inherited_visibility` and `analyze::private_calls`.
        if expr.decisions & crate::expr::INHERITED_VISIBILITY != 0 {
            continue;
        }
        // Class-scope constants — claimed by analyze::extract_const_assignments.
        if matches!(&*expr.node, ExprNode::Assign { target: LValue::Const { .. }, .. }) {
            continue;
        }
        let ExprNode::Send { recv, method, args, .. } = &*expr.node else {
            continue;
        };
        let name = method.as_str();
        // `KINDS.each { |k| define_method(…) }` that `define_method_loop` could not unroll: `check`'s error (`unrolled_loop_diagnostics`).
        if super::defines_methods_in_block(expr) {
            continue;
        }
        if let Some(recv) = recv {
            // Class-body writes are sends too, and dropping an unclaimed
            // setting must report just like dropping a receiver-less DSL.
            if !matches!(&*recv.node, ExprNode::SelfRef)
                || !name.ends_with('=')
                || matches!(name, "==" | "!=" | "<=" | ">=" | "===")
            {
                continue;
            }
            // `inheritance_column = nil` / `false` turns STI off, which
            // `lower::sti_scope` models. A column name stays unsupported.
            if crate::lower::sti_scope::turns_sti_off(expr).is_some() {
                continue;
            }
            // Literal table settings are consumed by ingest::model's
            // explicit_class_setting; dynamic values remain unsupported.
            if matches!(name, "table_name=" | "table_name_prefix=")
                && args.len() == 1
                && matches!(&*args[0].node, ExprNode::Lit { value: Literal::Str { .. } | Literal::Sym { .. } })
            {
                continue;
            }
        }
        // A bare visibility keyword is a marker for the `def`s after
        // it (the method walk reads it as one); it is not a DSL call.
        if matches!(name, "private" | "protected" | "public") {
            continue;
        }
        // `private_constant` restricts access from outside the class, which a program that runs on Rails never makes.
        if name == "private_constant" {
            continue;
        }
        // `attr_accessor`/`attr_reader`/`attr_writer` — claimed by
        // markers::push_attribute_api_methods, which synthesizes the
        // virtual attributes the permit filter counts as writers.
        if matches!(name, "attr_accessor" | "attr_reader" | "attr_writer") {
            continue;
        }
        // `has_one_attached` / `has_many_attached :name` (with or
        // without a variants block) — claimed by lower::attached on
        // exactly that shape. Campfire's three (`avatar`, `attachment`,
        // `logo`) were reported as not lowered on every emit while the
        // attachment lowering ran on each.
        if (name == "has_one_attached" || name == "has_many_attached") && crate::lower::attached::claims(expr) {
            continue;
        }
        // `broadcasts_to` — claimed by lower::broadcasts.
        if name == "broadcasts_to" && crate::lower::broadcasts::claims_broadcasts_to(args) {
            continue;
        }
        // `typed_store` — claimed by lower::typed_store's shared
        // method synthesis (all targets).
        if name == "typed_store" {
            continue;
        }
        // `has_secure_password` — claimed by lower::secure_password's
        // shared method synthesis (all targets).
        if name == "has_secure_password" {
            continue;
        }
        // `has_secure_token` — claimed by lower::secure_token, which
        // expands the bare/`length:`/`on:` forms it can honor. Asked
        // by span rather than re-derived, same as has_json/
        // has_rich_text below: an option it declines to expand keeps
        // warning.
        if name == "has_secure_token"
            && crate::lower::secure_token::secure_token_decls(&model.body)
                .iter()
                .any(|d| d.span == expr.span)
        {
            continue;
        }
        // `extend FriendlyId` / `friendly_id :col, use: [...]` — claimed
        // by lower::friendly_id on the configurations it honors. The
        // slug-generation gap that remains is reported once, below the
        // claim, as its own warning.
        if (name == "friendly_id" || name == "extend")
            && crate::lower::friendly_id::claims(model, expr.span)
        {
            if name == "friendly_id" {
                if let Some((span, message)) = crate::lower::friendly_id::write_gap(model) {
                    let kind = crate::diagnostic::DiagnosticKind::LowerResidue {
                        pass: crate::ident::Symbol::from("friendly_id"),
                        construct: crate::ident::Symbol::from("friendly_id"),
                        reason: crate::ident::Symbol::from("slug generation on save"),
                    };
                    crate::emit::diagnostics::push(Diagnostic {
                        span,
                        severity: Severity::Warning,
                        kind,
                        message,
                    });
                }
            }
            continue;
        }
        // `generates_token_for :purpose, expires_in: D do … end` —
        // claimed by lower::generates_token_for for the forms it
        // expands; asked by span for the same reason.
        if name == "generates_token_for" && crate::lower::generates_token_for::claims(model, expr.span) {
            continue;
        }
        // `has_json :col, key: <literal>, …` — claimed by
        // lower::has_json's shared method synthesis. A declaration
        // carrying a schema entry that pass cannot expand (a
        // symbol-declared type, whose Rails default is nil) is NOT
        // claimed and keeps warning: half an expansion is worse than
        // none. `has_json_decls` is the one place that decides which
        // declarations are claimed; this test asks it rather than
        // re-deriving the shape (same dance as `has_rich_text`).
        if name == "has_json"
            && crate::lower::has_json::has_json_decls(&model.body)
                .iter()
                .any(|d| d.span == expr.span)
        {
            continue;
        }
        // `serialize :col, coder: JSON` (and legacy positional `JSON`) —
        // claimed by lower::serialize; accessors go through RoundhouseJsonColumn.
        if name == "serialize"
            && crate::lower::serialize::serialize_decls(model)
                .iter()
                .any(|d| d.span == expr.span)
        {
            continue;
        }
        // `default_scope { where(…) }`, static or read at build time —
        // claimed by lower::default_scope (the runtime Relation applies
        // it; a target without one reports it per target). Other shapes
        // keep warning.
        if name == "default_scope" && crate::lower::default_scope::default_scope(model).is_some() {
            continue;
        }
        // Bare `has_rich_text :body` — claimed by lower::rich_text.
        // The option-carrying forms (`encrypted:`, `store_if_blank:`)
        // are NOT claimed: each changes the expansion and none is
        // implemented, so they keep warning. `rich_text_attrs` is the
        // one place that decides which shape is claimed; this test
        // asks it rather than re-deriving the arity.
        if name == "has_rich_text"
            && crate::lower::rich_text::rich_text_attrs(model)
                .iter()
                .any(|(span, _)| *span == expr.span)
        {
            continue;
        }
        // Bare `has_markdown :body` — claimed by lower::plain_text_attr
        // (named plain-text association+storage) only when the
        // `action_text_markdowns` table is present. Option-carrying
        // forms and missing-table apps stay unclaimed.
        if name == "has_markdown"
            && crate::lower::plain_text_attr::record_table_present(schema)
            && crate::lower::plain_text_attr::plain_text_attrs(model)
                .iter()
                .any(|(span, _)| *span == expr.span)
        {
            continue;
        }
        // `normalizes :attr, with: ->(v) { … }` over columns — claimed by
        // `lower::normalizes` (the `_normalize_<attr>` class method the
        // column writer and finders call). Another `with:`, or an
        // attribute that isn't a column, stays unclaimed and warns.
        if name == "normalizes"
            && crate::lower::normalizes::lowers_call(expr, |attr| {
                schema.tables.get(&model.table.0).is_some_and(|t| t.columns.iter().any(|c| c.name == *attr))
            })
        {
            continue;
        }
        // `attribute :name` — claimed by
        // markers::push_attribute_api_methods (virtual attributes),
        // with an optional type and a supported `default:`. A
        // default over an existing column (Rails makes it the column's
        // default; the column's reader is kept here, so it would be
        // dropped) or beside the model's own writer
        // (`markers::default_beside_source_writer`) stays unclaimed and
        // warns, as other arities do.
        if name == "attribute" {
            if let ExprNode::Send { args, .. } = &*expr.node {
                if let Some(decl) = super::markers::attribute_api_decl(args)
                    && !(decl.options.is_some()
                        && schema.tables.get(&model.table.0).is_some_and(|t| t.columns.iter().any(|c| c.name == *decl.name)))
                    && !super::markers::default_beside_source_writer(&model.body, args)
                {
                    // Over a column of another type, the column's cast,
                    // reader and storage are kept
                    // (`markers::attribute_matches_column`): reported.
                    let attr = decl.name;
                    if let Some(ty) = decl.ty
                        && !model.enums.contains_key(attr)
                        && let Some(col) = schema
                            .tables
                            .get(&model.table.0)
                            .and_then(|t| t.columns.iter().find(|c| c.name == *attr))
                        && !super::markers::attribute_matches_column(ty.as_str(), col)
                    {
                        let mut d = Diagnostic::unsupported(
                            expr.span,
                            None,
                            name,
                            format!(
                                "`attribute :{attr}, :{ty}` over a {}{} column on `{}`: the column's type is kept (its reader and storage don't follow the declared type)",
                               super::column_type_word(&col.col_type),
                                if col.array { " array" } else { "" },
                                model.name.0.as_str()
                            ),
                        );
                        d.severity = Severity::Warning;
                        crate::emit::diagnostics::push(d);
                    }
                    continue;
                }
            }
        }
        // `primary_abstract_class` — claimed by markers.rs.
        if name == "primary_abstract_class" {
            continue;
        }
        // `self.abstract_class = true|false` — the older spelling of the
        // same marker, read in order by `markers::is_abstract_class`.
        if name == "abstract_class=" && super::markers::is_literal_abstract_class_write(expr) {
            continue;
        }
        // `include Notifications` — a model concern, spliced by the
        // analyzer (`model_includes` + `concern_model_items`) into the
        // includer's registry and lowered with its methods; nothing is
        // left for an emitter to lower. Framework marker includes are
        // dropped by `model_includes` for the same reason.
        if name == "include" {
            continue;
        }
        // Block-form and lambda-argument lifecycle hooks
        // (`before_create -> { … }`) — both claimed by
        // markers::push_block_callback, on the shape it reads: a block,
        // or a parameterless lambda as the first argument.
        // Exactly the shapes `markers::push_block_callback` lowers
        // (`block_callback_shape`); a declined one reports below.
        if super::markers::block_callback_shape(expr).is_some() {
            continue;
        }
        // sorbet's pure ANNOTATIONS. The library-class walk drops
        // these at ingest; a model never saw them until an abstract
        // base outside `app/models` started being classified as one,
        // and then every such base reported its `abstract!` as
        // unlowered DSL. They carry types and nothing else, and the
        // emitted tree has no sorbet-runtime to read them.
        if matches!(
            name,
            "abstract!" | "interface!" | "final!" | "sealed!" | "sig" | "type_parameters"
        ) {
            continue;
        }
        // The mixin spelling of the same thing: `extend T::Helpers`.
        // (`include` is already skipped above, whatever its argument.)
        if name == "extend" {
            if let ExprNode::Send { args, .. } = &*expr.node {
                if let Some(ExprNode::Const { path }) = args.first().map(|a| &*a.node) {
                    let written =
                        path.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("::");
                    if matches!(written.as_str(), "T::Sig" | "T::Helpers" | "T::Generic") {
                        continue;
                    }
                }
            }
        }
        // Reported once by the shared analysis diagnostic, on every target.
        if name == "encrypts" {
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
        out.extend(
            model_diagnostics(model, &app.schema)
                .into_iter()
                .filter(|d| !matches!(&d.kind, crate::diagnostic::DiagnosticKind::Unsupported { construct, .. } if app.extensions.attribute_macro(construct.as_str()).is_some())),
        );
    }
    out
}

/// One error per unclaimed statement of `model`.
pub fn model_diagnostics(model: &Model, schema: &Schema) -> Vec<Diagnostic> {
    unclaimed_calls(model, schema)
        .into_iter()
        .map(|call| {
            let mut detail = format!(
                "`{}` in the body of `{}` is not modelled, so its effect would be dropped from the output",
                call.name,
                model.name.0.as_str()
            );
            if let Some(reason) = &call.reason {
                detail.push_str(": ");
                detail.push_str(reason);
            }
            Diagnostic::unsupported(call.span, None, call.name.as_str(), detail)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::ModelBodyItem;
    use crate::expr::Expr;
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
            mixin_sends: Default::default(),
            select_aliases: Default::default(),
            refused_class_macros: Vec::new(),
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
        call.span = Span { file: crate::span::FileId(1), start: 4, end: 17 };
        let mut refusing = model(vec![call, send("shout", vec![sym("title")])]);
        refusing.refused_class_macros.push(Span { file: crate::span::FileId(1), start: 4, end: 17 });
        assert!(names(&refusing) == ["shout"]);
    }
}
