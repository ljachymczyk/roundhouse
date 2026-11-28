//! `ActionView::ViewHelpers.asset_url`, which a bare `asset_url(...)` in
//! a non-view class lowers to. a large app's `Icons::DefaultIcon
//! .url` calls it with `skip_pipeline: true` on every app-install path;
//! the runtime had `asset_path` and `image_url` but no `asset_url`
//! (`undefined method 'asset_url' for module ActionView::ViewHelpers`).
//!
//! With no request or `config.asset_host` in reach, Rails answers the
//! same path `asset_path` does; `data:` and `cid:` sources pass through.

#[path = "support/emit_and_run.rs"]
mod emit_and_run;

#[test]
fn asset_url_answers_the_asset_path() {
    let out = emit_and_run::real_blog().run_ruby(
        r#"
puts ActionView::ViewHelpers.asset_url("/assets/default-app.png", skip_pipeline: true)
puts ActionView::ViewHelpers.asset_url("logo.png")
puts ActionView::ViewHelpers.asset_url("https://cdn.example/x.png")
puts ActionView::ViewHelpers.asset_url("data:image/png;base64,abc")
puts ActionView::ViewHelpers.asset_url("DATA:image/png;base64,abc")
puts ActionView::ViewHelpers.asset_url("Cid:logo")
"#,
    );
    out.assert_passes();
    assert_eq!(
        out.stdout.lines().collect::<Vec<_>>(),
        ["/assets/default-app.png", "/assets/logo.png", "https://cdn.example/x.png", "data:image/png;base64,abc", "DATA:image/png;base64,abc", "Cid:logo"],
        "{}",
        out.stdout
    );
}

/// An app's asset helper object asks
/// `ApplicationController.new.view_context` for `asset_url` / `asset_path`
/// outside a request, with an options hash (`host:`, a CDN override):
/// `undefined method 'view_context' for an instance of ApplicationController`.
/// The answers are Rails 8.1.4's AssetUrlHelper's for a context without a request (with an asset pipeline, so
/// `/assets/`): `asset_url` asks the request's protocol for a host without a scheme, and there is none.
#[test]
fn a_controllers_view_context_answers_asset_url_and_path() {
    let out = emit_and_run::empty_app()
        .write("app/controllers/application_controller.rb", "class ApplicationController < ActionController::Base\nend\n")
        .write("config/routes.rb", "Rails.application.routes.draw do\nend\n")
        .run_ruby(
        r#"
view = ApplicationController.new.view_context
puts view.asset_url("logo.png", {})
puts view.asset_url("logo.png?v=2#top")
puts view.asset_path("logo.png", host: "cdn.example.com/cdn")
puts view.asset_url("logo.png", host: "https://cdn.example.com")
puts view.asset_url("https://other.example/x.png", host: "cdn.example.com")
puts view.asset_url("/already/rooted.png")
puts view.asset_url("raw.png", skip_pipeline: true)
puts view.asset_url("").inspect
begin
  view.asset_url(nil)
rescue ArgumentError => e
  puts e.message
end
puts view.asset_path("logo.png", host: "cdn.example.com", protocol: "https")
puts view.asset_path("logo.png", { host: "cdn.example.com", protocol: "https" })
begin
  view.asset_url("logo.png", host: "cdn.example.com")
rescue NoMethodError => e
  puts e.message
end
"#,
    );
    out.assert_passes();
    assert_eq!(
        out.stdout.lines().collect::<Vec<_>>(),
        [
            "/assets/logo.png",
            "/assets/logo.png?v=2#top",
            "//cdn.example.com/cdn/assets/logo.png",
            "https://cdn.example.com/assets/logo.png",
            "https://other.example/x.png",
            "/already/rooted.png",
            "/raw.png",
            "\"\"",
            "nil is not a valid asset source",
            "https://cdn.example.com/assets/logo.png",
            "https://cdn.example.com/assets/logo.png",
            "undefined method 'protocol' for nil",
        ],
        "{}",
        out.stdout
    );
}

/// The same call from the app's own code: analysis types `view_context` as the view context, so its asset helpers
/// resolve and the class checks clean; a strict target, whose runtime has no view context, refuses it by name.
#[test]
fn an_app_class_asking_a_controller_for_its_view_context_checks_and_runs() {
    let app = || {
        emit_and_run::empty_app()
            .write("app/controllers/application_controller.rb", "class ApplicationController < ActionController::Base\nend\n")
            .write("config/routes.rb", "Rails.application.routes.draw do\nend\n")
            .write(
                "app/models/asset_links.rb",
                "class AssetLinks\n  def self.logo\n    ApplicationController.new.view_context.asset_url(\"logo.png\", host: \"https://cdn.example.com\")\n  end\nend\n",
            )
    };
    let run = app().run_ruby("puts AssetLinks.logo\n");
    run.assert_passes();
    assert_eq!(run.stdout.trim(), "https://cdn.example.com/assets/logo.png");
    let (_, errors) = app().emit(roundhouse::project::BuildTarget::Rust);
    assert!(errors.iter().any(|e| e.contains("ActionController::Base#view_context")), "{errors:#?}");
}
