//! A `try` whose class defines its own (parslet's atoms' `try(source, context, consume_all)`, called as
//! `obj.try(source, self, consume_all)` from its parse cache): ActiveSupport's Object#try is not the method, so the
//! call is not written as its body (`respond_to?(source)` raised on lobsters' search parse). An untyped receiver in a
//! program with such a class takes the class's `try` where it answers one, and ActiveSupport's otherwise. A receiver
//! only a subclass's method types (parslet's `Parser#try` calling the `root` each parser defines) takes its own.

use super::emit_and_run;

const PROBE: &str = r#"class Atom
  def try(source, context, consume_all)
    source + (consume_all ? "!" : "?")
  end
end

class Grammar
  def run(source)
    root.try(source, self, true)
  end
end

class WordGrammar < Grammar
  def initialize
    @atom = Atom.new
  end

  def root
    @atom
  end
end

class Plain
  def label
    "plain"
  end
end

class Probe
  def initialize(raw)
    @raw = raw
    @atom = Atom.new
  end

  def typed
    source = "atom"
    @atom.try(source, self, true)
  end

  # (untyped)

  def grammar
    source = "word"
    WordGrammar.new.run(source)
  end

  # (picked)
end
"#;

const UNTYPED: &str = "  def untyped\n    name = \"label\"\n    JSON.parse(@raw)[\"word\"].try(name) || \"none\"\n  end\n";
const PICKED: &str = "  def picked\n    name = \"label\"\n    pick = @raw.empty? ? Atom.new : Plain.new\n    pick.try(name)\n  end\n";

fn app() -> emit_and_run::Overlay {
    let probe = PROBE.replace("  # (untyped)\n", UNTYPED).replace("  # (picked)\n", PICKED);
    let text = "probe.typed + \" \" + probe.untyped + \" \" + probe.picked + \" \" + probe.grammar";
    emit_and_run::real_blog()
        .edit("config/routes.rb", "  root \"articles#index\"\n", "  root \"articles#index\"\n  get \"/probe\", to: \"probes#index\"\n")
        .write("lib/probe.rb", &probe)
        .write(
            "app/controllers/probes_controller.rb",
            &format!("class ProbesController < ApplicationController\n  def index\n    probe = Probe.new('{{\"word\":\"hey\"}}')\n    render plain: {text}\n  end\nend\n"),
        )
}

const EXPECTED: &str = "atom! none plain word!";

#[test]
fn a_class_s_own_try_is_called() {
    let run = app().run_ruby(&format!(
        r##"response = Main.run_rack("REQUEST_METHOD" => "GET", "PATH_INFO" => "/probe", "QUERY_STRING" => "", "rack.input" => StringIO.new(""))
raise "GET /probe answered #{{response[0]}}" unless response[0] == 200
text = +""
response[2].each {{ |part| text << part }}
raise "probe: #{{text.inspect}}" unless text == "{EXPECTED}"
puts "own try passed"
"##
    ));
    run.assert_passes();
    let probe = std::fs::read_to_string(run.emitted.join("app/models/probe.rb")).expect("emitted probe");
    assert!(probe.contains("@atom.try(source, self, true)"), "{probe}");
    // A method only the subclasses define answers as they do, so `root` is an Atom or nil and its own `try` is called.
    assert!(!probe.contains("public_send(source"), "{probe}");
}

#[test]
#[ignore = "requires the Spinel toolchain"]
fn a_class_s_own_try_is_called_on_spinel() {
    let run = app().run_spinel(&format!(
        r##"controller = ProbesController.new
controller.request_method = "GET"
controller.params = {{}}
controller.process_action(:index)
raise "probe: #{{controller.body}}" unless controller.body == "{EXPECTED}"
puts "own try passed"
"##
    ));
    run.assert_passes();
}

/// A subclass that answers `root` through its `method_missing` is not bounded by the subclasses' typed answers: its
/// String takes ActiveSupport's `try`, which answers nil for a name the String lacks.
#[test]
fn a_subclass_s_method_missing_keeps_active_support_s_try() {
    let probe = PROBE.replace("  # (untyped)\n", "").replace("  # (picked)\n", "  def missing\n    MissingGrammar.new.run(\"word\").inspect\n  end\n").replace(
        "class Plain\n",
        "class MissingGrammar < Grammar\n  def method_missing(name, *args)\n    \"missing\"\n  end\n\n  def respond_to_missing?(name, include_private = false)\n    true\n  end\nend\n\nclass Plain\n",
    );
    let run = emit_and_run::real_blog()
        .edit("config/routes.rb", "  root \"articles#index\"\n", "  root \"articles#index\"\n  get \"/probe\", to: \"probes#index\"\n")
        .write("lib/probe.rb", &probe)
        .write(
            "app/controllers/probes_controller.rb",
            "class ProbesController < ApplicationController\n  def index\n    probe = Probe.new(\"\")\n    render plain: probe.grammar + \" \" + probe.missing\n  end\nend\n",
        )
        .run_ruby(
            r##"response = Main.run_rack("REQUEST_METHOD" => "GET", "PATH_INFO" => "/probe", "QUERY_STRING" => "", "rack.input" => StringIO.new(""))
text = +""
response[2].each { |part| text << part }
raise "probe: #{response[0]} #{text.inspect}" unless response[0] == 200 && text == "word! nil"
puts "method_missing try passed"
"##,
        );
    run.assert_passes();
    let grammar = std::fs::read_to_string(run.emitted.join("app/models/grammar.rb")).expect("emitted grammar");
    assert!(grammar.contains("public_send(source"), "{grammar}");
}
