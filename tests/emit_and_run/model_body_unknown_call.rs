//! A model class-body call no pass lowers is an error naming the call and
//! its line. It used to be an emit-time warning that `check` never shows,
//! so the app checked clean while the call's effect was missing from the
//! emitted model (AGENTS.md invariant 6).

use super::emit_and_run::real_blog;
use roundhouse::project::BuildTarget;

/// An app class method run from the class body, the plainest macro
/// roundhouse does not model: Ruby defines `title_shout` at load time.
const SHOUT: &str = "class Article < ApplicationRecord
  def self.shout(attr)
    define_method(\"#{attr}_shout\") { public_send(attr).upcase }
  end
  shout :title
";

fn errors_for(replacement: &str) -> (Vec<String>, String) {
    let (emitted, app, errors) = real_blog()
        .edit("app/models/article.rb", "class Article < ApplicationRecord\n", replacement)
        .emit_with_app(BuildTarget::Ruby);
    let rendered = errors.iter().map(|d| d.render(&app.sources)).collect();
    let article = std::fs::read_to_string(emitted.join("app/models/article.rb")).unwrap_or_default();
    (rendered, article)
}

#[test]
fn an_unmodelled_class_body_call_is_an_error_naming_the_call_and_its_line() {
    let (errors, article) = errors_for(SHOUT);
    // One report per call: this pass's, or the class-macro evaluator's when it refused the expansion.
    let hits: Vec<&String> = errors
        .iter()
        .filter(|e| {
            e.contains("app/models/article.rb:5:")
                && (e.contains("`shout` in the body of `Article` is not modelled")
                    || e.contains("`shout` defines methods this call does not expand"))
        })
        .collect();
    assert_eq!(hits.len(), 1, "{errors:#?}");
    assert!(hits[0].contains("app/models/article.rb:5:"), "{}", hits[0]);
    // The drop the error stands for: the emitted model has no such method.
    assert!(!article.contains("title_shout"), "{article}");
}

#[test]
fn a_known_macro_option_that_is_not_lowered_is_an_error() {
    let (errors, _) = errors_for("class Article < ApplicationRecord\n  has_one_attached :cover, service: :mirror\n");
    assert!(
        errors.iter().any(|e| e.contains("`has_one_attached` in the body of `Article` is not modelled")
            && e.contains("app/models/article.rb:2:")),
        "{errors:#?}"
    );
}

/// The other half: a macro a pass lowers and a documented no-op add no
/// error, and the emitted model still passes its tests.
#[test]
fn a_lowered_macro_and_a_no_op_call_stay_clean_and_run() {
    real_blog()
        .edit(
            "app/models/article.rb",
            "class Article < ApplicationRecord\n",
            "class Article < ApplicationRecord\n  LIMIT = 10\n  private_constant :LIMIT\n  normalizes :title, with: ->(t) { t.strip }\n",
        )
        .run_test("test/models/article_test.rb")
        .assert_passes();
}
