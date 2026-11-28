//! `String#parameterize` in a library class's method (mission_control's `IdentifiedByName#id`, `name.parameterize`):
//! the emitted runtime defines no String#parameterize, so the call is `Inflector.parameterize(name)` there as in a
//! model's body. Left as written, lobsters' Spinel build raised NoMethodError as it booted.

use super::emit_and_run;

const PROBE: &str = r#"class Probe
  def initialize(name)
    @name = name.to_s
  end

  def name
    @name
  end

  def id
    name.parameterize
  end
end
"#;

fn app() -> emit_and_run::Overlay {
    emit_and_run::real_blog()
        .edit("config/routes.rb", "  root \"articles#index\"\n", "  root \"articles#index\"\n  get \"/probe\", to: \"probes#index\"\n")
        .write("lib/probe.rb", PROBE)
        .write("app/controllers/probes_controller.rb", "class ProbesController < ApplicationController\n  def index\n    render plain: Probe.new(\"Solid Queue Server\").id\n  end\nend\n")
}

#[test]
fn a_library_class_s_parameterize_runs() {
    let run = app().run_ruby(
        r##"response = Main.run_rack("REQUEST_METHOD" => "GET", "PATH_INFO" => "/probe", "QUERY_STRING" => "", "rack.input" => StringIO.new(""))
raise "GET /probe answered #{response[0]}" unless response[0] == 200
text = +""
response[2].each { |part| text << part }
raise "probe: #{text.inspect}" unless text == "solid-queue-server"
puts "library parameterize passed"
"##,
    );
    run.assert_passes();
}

#[test]
#[ignore = "requires the Spinel toolchain"]
fn a_library_class_s_parameterize_runs_on_spinel() {
    let run = app().run_spinel(
        r##"controller = ProbesController.new
controller.request_method = "GET"
controller.params = {}
controller.process_action(:index)
raise "probe: #{controller.body}" unless controller.body == "solid-queue-server"
puts "library parameterize passed"
"##,
    );
    run.assert_passes();
}
