module FeatureFlags
  VECTOR_TILES_ENABLED = ActiveModel::Type::Boolean.new.cast(ENV.fetch('VECTOR_TILES_ENABLED', false))
end
