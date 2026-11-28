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

  # `store :col, coder: JSON`: Rails' `IndifferentCoder`, whose Hash reads by a Symbol too and stands in for nil. As
  # Rails keeps one Hash per attribute value, a record's reads answer the same Hash while the stored text is the one it
  # came from; a change made in place on it, or on a Hash nested in it, is written back through the record's writer.
  def self.load_store(serialized, owner = nil, writer = nil)
    if owner && writer
      held = owner.instance_variable_get(store_slot(writer))
      return held[1] if held && held[0] == serialized
    end
    decoded = serialized.nil? || serialized.empty? ? nil : JSON.parse(serialized)
    store = Store.new(decoded.is_a?(Hash) ? decoded : {})
    store.bind(owner, writer, store)
    owner.instance_variable_set(store_slot(writer), [serialized, store]) if owner && writer
    store
  end

  def self.dump_store(value)
    hash = value.respond_to?(:to_hash) ? ActiveSupport::HashWithIndifferentAccess.new(value.to_hash) : ActiveSupport::HashWithIndifferentAccess.new
    ActiveSupport::JSON.encode_without_escape(hash)
  end

  # Not an attribute of the record's: a name no column or method can take.
  def self.store_slot(writer)
    :"@__roundhouse_store_#{writer.delete_suffix("=")}"
  end

  def self.remember(owner, writer, store)
    owner.instance_variable_set(store_slot(writer), [dump_store(store), store])
  end

  class Store < ActiveSupport::HashWithIndifferentAccess
    def bind(owner, writer, root)
      @owner = owner
      @writer = writer
      @root = root
      each_value { |value| bind_nested(value) }
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

    def convert_value(value)
      converted = super
      bind_nested(converted)
      converted
    end

    def bind_nested(value)
      case value
      when Store then value.bind(@owner, @writer, @root || self)
      when Array then value.each { |item| bind_nested(item) }
      end
    end

    def write_back
      return unless @owner && @writer
      root = @root || self
      @owner.public_send(@writer, root)
      RoundhouseJsonColumn.remember(@owner, @writer, root)
    end
  end
end
