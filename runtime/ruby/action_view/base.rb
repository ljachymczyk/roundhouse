# frozen_string_literal: true

# The view context Rails instantiates for a template. Roundhouse still
# macro-expands `form_with` into the accumulator (the block binds a
# typed builder the runtime does not reconstruct), but each expanded
# site begins by calling this instance method. A fragment-cache hit
# never walks that site, so it never enters `form_with`.
#
# That is the entry `ActionView::Base.any_instance.expects(:form_with)`
# observes. The method returns nil; the markup that follows in the
# expansion is the form. A body that also emitted the tags would
# double them.
module ActionView
  class Base
    def form_with
      nil
    end

    # Rails' AssetUrlHelper#asset_path for a view context no request made (`ApplicationController.new.view_context`):
    # the source as the asset pipeline names it, `skip_pipeline:` leaving it under public/, then the `host:` option as the
    # asset host. `config.asset_host` and a `%d` host are not read.
    def asset_path(source, options = nil, skip_pipeline: false, host: nil, protocol: nil)
      skip = options.nil? ? skip_pipeline : options[:skip_pipeline] == true
      at = options.nil? ? host : asset_option_string(options, :host)
      asked = options.nil? ? protocol : asset_option_protocol(options)
      path = asset_path_with(source, skip, at, asked)
      path
    end

    # Rails' asset_url: asset_path asked for the request's protocol, which a context with no request does not have.
    def asset_url(source, options = nil, skip_pipeline: false, host: nil)
      skip = options.nil? ? skip_pipeline : options[:skip_pipeline] == true
      at = options.nil? ? host : asset_option_string(options, :host)
      url = asset_path_with(source, skip, at, :request)
      url
    end

    # Rails' positional options Hash (`asset_url(src, { host: cdn })`), read by key.
    def asset_option_string(options, key)
      value = options[key]
      value.is_a?(String) ? value : nil
    end

    def asset_option_protocol(options)
      value = options[:protocol]
      value.is_a?(String) || value.is_a?(Symbol) ? value : nil
    end

    def asset_path_with(source, skip_pipeline, host, protocol)
      raise ArgumentError, "nil is not a valid asset source" if source.nil?
      return "" if ActionView::ViewHelpers.blank_source?(source)
      return source if ActionView::ViewHelpers.asset_url?(source)
      # Rails' `[?#].+` tail: from the first `?` or `#` that something follows.
      cut = source.length
      i = 0
      while i < source.length - 1
        if source[i, 1] == "?" || source[i, 1] == "#"
          cut = i
          break
        end
        i += 1
      end
      tail = source[cut, source.length - cut]
      path = source[0, cut]
      unless path.start_with?("/")
        path = skip_pipeline ? "/#{path}" : ActionView::ViewHelpers.digested_asset_path(path)
      end
      answered = host.nil? ? "#{path}#{tail}" : "#{asset_host(host, protocol)}#{path}#{tail}"
      answered
    end

    # Rails' compute_asset_host with no request: a host without a scheme takes the protocol asked for, relative when none.
    def asset_host(host, protocol)
      return host.delete_suffix("/") if ActionView::ViewHelpers.asset_url?(host)
      wanted = protocol.nil? ? "relative" : protocol.to_s
      raise NoMethodError, "undefined method 'protocol' for nil" if wanted == "request"
      prefix = wanted == "relative" ? "//" : "#{wanted}://"
      prefixed = "#{prefix}#{host.delete_suffix("/")}"
      prefixed
    end
    private :asset_path_with, :asset_host, :asset_option_string, :asset_option_protocol
  end
end

module ActionView
  # Rails 8.1's `ActionView::OutputBuffer`: a template's markup, a String escaped as it is appended unless html_safe
  # (`ERB::Util.unwrapped_html_escape`), raw through `safe_concat`. A component's render (view_component's
  # `render_in`) starts one.
  class OutputBuffer
    def initialize(buffer = "")
      @raw_buffer = +""
      @raw_buffer << buffer
    end

    def to_s
      @raw_buffer
    end

    def to_str
      @raw_buffer.dup
    end

    def length
      @raw_buffer.length
    end

    def empty?
      @raw_buffer.empty?
    end

    def html_safe?
      true
    end

    def <<(value)
      @raw_buffer << ActionView::ViewHelpers.html_escape(value) unless value.nil?
      self
    end

    def safe_concat(value)
      @raw_buffer << value
      self
    end
  end

  # What a view's `lookup_context` answers a component: no request variant (`+phone`) and the HTML format, the
  # template a request without either renders.
  class LookupContext
    def variants
      []
    end

    def formats
      [:html]
    end
  end

  class Base
    def lookup_context
      @lookup_context ||= LookupContext.new
    end

    # Rails' renderer and content_for flow: none is modelled here, and a component reads them only through helpers
    # this runtime does not have (older Rails' path helpers, `content_for` from a component).
    def view_renderer
      nil
    end

    def view_flow
      nil
    end

    # A view built on its own (`ActionView::Base.new`, as a component's `render_in` is handed one) has no controller.
    def controller
      nil
    end

    # ERB::Util's `html_escape`, which Rails' view includes (view_component's `maybe_escape_html` calls it bare).
    # Not ERB::Util's check of a safe String first, which no typed String answers here: view_component's one caller
    # (`maybe_escape_html`) hands it only text that is not html_safe.
    def html_escape(text)
      ActionView::ViewHelpers.html_escape(text.to_s)
    end
  end
end
