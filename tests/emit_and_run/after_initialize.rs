//! `Rails.application.config.after_initialize do … end` in an initializer
//! runs once the app is booted, in initializer order, and a `next` in the
//! body leaves that callback only, as it leaves the block in Rails.

use super::emit_and_run;

fn app() -> emit_and_run::Overlay {
    emit_and_run::empty_app()
        .write("app/models/application_record.rb", "class ApplicationRecord < ActiveRecord::Base\n  self.abstract_class = true\nend\n")
        .write("app/controllers/application_controller.rb", "class ApplicationController < ActionController::Base\nend\n")
        .write("config/routes.rb", "Rails.application.routes.draw do\nend\n")
        .write("db/schema.rb", "ActiveRecord::Schema.define do\n  create_table :widgets do |t|\n    t.string :label\n  end\nend\n")
        .write("app/models/widget.rb", "class Widget < ApplicationRecord\nend\n")
        .write("app/models/boot_log.rb", r#"class BootLog
  ENTRIES = []

  def self.record(entry)
    ENTRIES << entry
  end

  def self.entries
    ENTRIES
  end
end
"#)
        .write("config/initializers/a_first.rb", r#"Rails.application.config.after_initialize do
  BootLog.record("first")
end
"#)
        .write("config/initializers/b_second.rb", r#"Rails.application.config.after_initialize do
  next if BootLog.entries.empty?
  BootLog.record("second")
end

Rails.application.config.after_initialize do
  next unless BootLog.entries.empty?
  BootLog.record("skipped")
end
"#)
}

const ASSERTIONS: &str = r#"
raise "after_initialize callbacks: #{BootLog.entries.inspect}" unless BootLog.entries == ["first", "second"]
puts "after_initialize callbacks ran"
"#;

#[test]
fn after_initialize_callbacks_run_at_boot_on_ruby() {
    app().run_ruby(ASSERTIONS).assert_passes();
}

#[test]
#[ignore = "requires the Spinel toolchain"]
fn after_initialize_callbacks_run_at_boot_on_spinel() {
    app().run_spinel(ASSERTIONS).assert_passes();
}
