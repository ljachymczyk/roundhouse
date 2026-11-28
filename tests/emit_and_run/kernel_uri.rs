use super::emit_and_run;

/// `URI(origin).host`: Kernel#URI from the uri library, the call an app
/// writes to take a host or a scheme out of a configured origin. CRuby
/// parses a String to a URI (`URI::HTTPS` for an https origin), returns a
/// URI it is given unchanged, and raises `URI::InvalidURIError` on a
/// string that is not one. Answers are CRuby 4.0.5's (uri 1.1.2).
const METHODS: &str = "  validates :title, presence: true

  #: (String) -> String?
  def self.origin_host(origin)
    URI(origin).host
  end

  #: (String) -> String?
  def self.origin_scheme(origin)
    URI(origin).scheme
  end

  #: (String) -> Integer?
  def self.origin_port(origin)
    URI(origin).port
  end

  #: (String) -> String?
  def self.origin_path(origin)
    URI(origin).path
  end

  #: (String) -> String?
  def self.origin_query(origin)
    URI(origin).query
  end

  #: (String) -> String?
  def self.reparsed_host(origin)
    URI(URI(origin)).host
  end

  #: (String) -> String?
  def self.checked_host(origin)
    URI(origin).host
  rescue URI::InvalidURIError
    \"invalid\"
  end
";

fn app() -> emit_and_run::Overlay {
    emit_and_run::real_blog().edit("app/models/article.rb", "  validates :title, presence: true\n", METHODS)
}

const ASSERTIONS: &str = r#"got = [
  Article.origin_host("https://x.example/a?b=1"),
  Article.origin_scheme("https://x.example/a?b=1"),
  Article.origin_port("https://x.example/a?b=1"),
  Article.origin_port("http://x.example:8080/"),
  Article.origin_path("https://x.example/a?b=1"),
  Article.origin_query("https://x.example/a?b=1"),
  Article.reparsed_host("https://x.example/"),
  Article.checked_host("https://bad host/"),
]
want = ["x.example", "https", 443, 8080, "/a", "b=1", "x.example", "invalid"]
raise "URI(): #{got.inspect}" unless got == want
puts "kernel uri passed"
"#;

#[test]
fn kernel_uri_reports_no_error() {
    let errors = app().check_errors();
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn kernel_uri_answers_like_cruby() {
    let run = app().run_ruby(ASSERTIONS);
    run.assert_passes();
    assert!(run.stdout.contains("kernel uri passed"), "{}", run.stdout);
    let model = std::fs::read_to_string(run.emitted.join("app/models/article.rb")).expect("emitted model");
    assert!(model.lines().any(|line| line.trim() == "require \"uri\""), "{model}");
}

#[test]
#[ignore = "requires the Spinel toolchain"]
fn kernel_uri_answers_like_cruby_on_spinel() {
    app().run_spinel(ASSERTIONS).assert_passes();
}
