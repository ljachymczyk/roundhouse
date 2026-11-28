# frozen_string_literal: true
# Rails' `ActionView::Rendering#view_context`: the object a template's helper calls land on, which a plain object
# also asks `ApplicationController.new` for outside any request (`view_context.asset_url`).
#
# RUBY-FAMILY ONLY, like `default_headers.rb` beside it: the strict targets' runtime tables carry no
# `ActionView::Base` to construct.
module ActionController
  class Base
    def view_context
      ActionView::Base.new
    end
  end
end
