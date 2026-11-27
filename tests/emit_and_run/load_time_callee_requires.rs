//! A constant whose value calls a class method as its file loads (et-orbi's `ZONES_OLSON = TZInfo::Timezone.all…`):
//! what that method runs names classes too, and they must be loaded first. Ruby has them by et-orbi's
//! `require 'tzinfo'`; the emitted files load in name order, so `AZone` loaded ahead of the classes `Registry.all`
//! reaches (a class method, the `new` it makes and the `initialize` its parent's `super` runs) and raised NameError.

use super::emit_and_run;

const ZONE: &str = r#"module AZone
  NAMES = Registry.all.map { |n| n.upcase }
end
"#;

const REGISTRY: &str = r#"class Registry
  def self.all
    source.names
  end

  def self.source
    Source.get
  end
end
"#;

const SOURCE: &str = r#"class SourceBase
  def initialize
    @cache = Cache.new
  end
end

class Source < SourceBase
  def self.get
    new
  end

  def initialize
    super
    @names = Seed::NAMES
  end

  def names
    @names
  end
end
"#;

const CACHE: &str = "class Cache\nend\n";
const SEED: &str = "module Seed\n  NAMES = [\"utc\", \"jst\"].freeze\nend\n";

fn app() -> emit_and_run::Overlay {
    emit_and_run::real_blog()
        .edit("config/routes.rb", "  root \"articles#index\"\n", "  root \"articles#index\"\n  get \"/probe\", to: \"probes#index\"\n")
        .write("lib/a_zone.rb", ZONE)
        .write("lib/registry.rb", REGISTRY)
        .write("lib/source.rb", SOURCE)
        .write("lib/zz_cache.rb", CACHE)
        .write("lib/zz_seed.rb", SEED)
        .write("app/controllers/probes_controller.rb", "class ProbesController < ApplicationController\n  def index\n    render plain: AZone::NAMES.join(\",\")\n  end\nend\n")
}

#[test]
fn what_a_load_time_call_reaches_is_loaded_first() {
    let run = app().run_ruby(
        r##"response = Main.run_rack("REQUEST_METHOD" => "GET", "PATH_INFO" => "/probe", "QUERY_STRING" => "", "rack.input" => StringIO.new(""))
raise "GET /probe answered #{response[0]}" unless response[0] == 200
text = +""
response[2].each { |part| text << part }
raise "probe: #{text.inspect}" unless text == "UTC,JST"
puts "load-time callee passed"
"##,
    );
    run.assert_passes();
}

#[test]
#[ignore = "requires the Spinel toolchain"]
fn what_a_load_time_call_reaches_is_loaded_first_on_spinel() {
    let run = app().run_spinel(
        r##"controller = ProbesController.new
controller.request_method = "GET"
controller.params = {}
controller.process_action(:index)
raise "probe: #{controller.body}" unless controller.body == "UTC,JST"
puts "load-time callee passed"
"##,
    );
    run.assert_passes();
}
