//! A model class-body call no pass lowers is an error naming the call and
//! its line. It used to be an emit-time warning that `check` never shows,
//! so the app checked clean while the call's effect was missing from the
//! emitted model (AGENTS.md invariant 6).

use super::emit_and_run::real_blog;
use roundhouse::project::BuildTarget;

/// A gem's macro roundhouse does not model (positioning's `positioned_by`): Rails runs it as the class
/// loads, and the output has nothing that does. (An app class method of the same shape is replayed.)
const UNMODELLED: &str = "class Article < ApplicationRecord
  positioned_by :title
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
    let (errors, article) = errors_for(UNMODELLED);
    let hits: Vec<&String> = errors
        .iter()
        .filter(|e| e.contains("app/models/article.rb:2:") && e.contains("`positioned_by` in the body of `Article` is not modelled"))
        .collect();
    assert_eq!(hits.len(), 1, "{errors:#?}");
    // The drop the error stands for: the emitted model does not make the call.
    assert!(!article.contains("positioned_by"), "{article}");
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

fn widgets(model: &str) -> super::emit_and_run::Overlay {
    super::emit_and_run::empty_app()
        .write(
            "db/schema.rb",
            "ActiveRecord::Schema[8.1].define(version: 1) do\n  create_table \"widgets\", force: :cascade do |t|\n    t.string \"title\"\n    t.string \"label\"\n    t.text \"settings\"\n  end\nend\n",
        )
        .write("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\n  primary_abstract_class\nend\n")
        .write("app/models/widget.rb", model)
        .write("config/routes.rb", "Rails.application.routes.draw do\nend\n")
}

/// A concern's class method that declares on its caller (solid_queue's `assumes_attributes_from_job`): the
/// class attribute it widens and the `before_create` it registers run in the model's body, as in Rails.
/// The expected values are ActiveRecord 8.1.4's answers.
#[test]
fn a_class_method_that_declares_on_its_caller_runs_in_the_body() {
    widgets(
        "class Widget < ApplicationRecord\n  include Stamping\n  stamps :label\nend\n",
    )
    .write(
        "app/models/concerns/stamping.rb",
        r##"module Stamping
  extend ActiveSupport::Concern
  included do
    class_attribute :stamped_fields, instance_accessor: false, default: %i[title]
  end
  class_methods do
    def stamps(*names)
      self.stamped_fields |= names
      before_create -> { stamp }
    end
  end
  private
    def stamp
      self.class.stamped_fields.each { |f| send("#{f}=", "#{send(f)}!") }
    end
end
"##,
    )
    .run_ruby(
        r##"w = Widget.create!(title: "a", label: "b")
got = [w.title, w.label, Widget.stamped_fields]
raise "stamps: #{got.inspect}" unless got == ["a!", "b!", [:title, :label]]
"##,
    )
    .assert_passes();
}

/// `store :col, coder: JSON`: a Hash read by a String or a Symbol, `{}` for a row that never set it.
/// The expected values are ActiveRecord 8.1.4's answers.
#[test]
fn a_json_store_reads_by_either_key() {
    widgets("class Widget < ApplicationRecord\n  store :settings, coder: JSON\nend\n")
        .run_ruby(
            r##"w = Widget.create!(title: "a", settings: { theme: "dark" })
r = Widget.find(w.id)
got = [r.settings[:theme], r.settings["theme"], Widget.find(Widget.create!(title: "c").id).settings]
raise "store: #{got.inspect}" unless got == ["dark", "dark", {}]
stored = Widget.connection.select_value("SELECT settings FROM widgets WHERE id = #{w.id}")
raise "stored: #{stored.inspect}" unless stored == "{\"theme\":\"dark\"}"
"##,
        )
        .assert_passes();
}

/// A target without HashWithIndifferentAccess refuses the store rather than answering a Symbol key with nil.
#[test]
fn a_json_store_is_refused_where_the_runtime_has_no_indifferent_hash() {
    let (_, errors) = widgets("class Widget < ApplicationRecord\n  store :settings, coder: JSON\nend\n").emit(BuildTarget::Spinel);
    assert!(errors.iter().any(|e| e.contains("store not supported (spinel)")), "{errors:#?}");
}
