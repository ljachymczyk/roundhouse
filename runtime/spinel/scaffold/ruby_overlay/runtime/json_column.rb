# frozen_string_literal: true
# The storage boundary for schema-less ActiveRecord json/jsonb columns
# and `serialize …, coder: JSON`. SQLite hands the generated model
# serialized text; Rails exposes the decoded Array/Hash/scalar.
# Hydration writes stored text directly to the backing slot (never
# through dump); application assignment always JSON-encodes, including
# String values, so `"true"` round-trips as a string rather than bool.
#
# Named `RoundhouseJsonColumn`, not `JsonColumn`: that is a common name for
# an app's own `serialize` coder class, and a runtime module of the same
# name made the app's `class JsonColumn` fail to load ("is not a class").
require "json"

module RoundhouseJsonColumn
  def self.load(serialized)
    return nil if serialized.nil?
    JSON.parse(serialized)
  end

  # Not `JSON.generate`: Rails 8.1's JSON coder and json column encode through ActiveSupport's as_json (a Time as ISO 8601), unescaped.
  def self.dump(value)
    return nil if value.nil?
    ActiveSupport::JSON.encode_without_escape(value)
  end

  # `store :col, coder: JSON`: Rails' `IndifferentCoder`, whose Hash reads by a Symbol too and stands in for nil. Each
  # read decodes the stored text, so a change made in place (`record.settings[:k] = v`, Rails' store idiom) is written
  # back through the record's writer; a change to a Hash nested inside it is not.
  def self.load_store(serialized, owner = nil, writer = nil)
    decoded = serialized.nil? || serialized.empty? ? nil : JSON.parse(serialized)
    store = Store.new(decoded.is_a?(Hash) ? decoded : {})
    store.bind(owner, writer)
  end

  def self.dump_store(value)
    hash = value.respond_to?(:to_hash) ? ActiveSupport::HashWithIndifferentAccess.new(value.to_hash) : ActiveSupport::HashWithIndifferentAccess.new
    ActiveSupport::JSON.encode_without_escape(hash)
  end

  class Store < ActiveSupport::HashWithIndifferentAccess
    def bind(owner, writer)
      @owner = owner
      @writer = writer
      self
    end

    def []=(key, value)
      super
      write_back
      value
    end

    def store(key, value)
      self[key] = value
    end

    def delete(key)
      removed = super
      write_back
      removed
    end

    def update(*others, &block)
      super
      write_back
      self
    end

    # Not Hash's: the indifferent `update` converts the keys.
    def merge!(*others, &block)
      update(*others, &block)
    end

    def replace(other)
      super
      write_back
      self
    end

    def clear
      super
      write_back
      self
    end

    private

    def write_back
      @owner.public_send(@writer, self) if @owner && @writer
    end
  end
end
