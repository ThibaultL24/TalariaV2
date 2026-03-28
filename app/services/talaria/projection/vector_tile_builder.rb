module Talaria
  module Projection
    class VectorTileBuilder
      def self.call
        return nil unless FeatureFlags::VECTOR_TILES_ENABLED

        <<~SQL
          SELECT ST_AsMVT(tile, 'events', 4096, 'geom') FROM (
            SELECT id, ST_AsMVTGeom(geom, ST_TileEnvelope($1, $2, $3)) AS geom
            FROM events
            WHERE geom IS NOT NULL
          ) tile;
        SQL
      end
    end
  end
end
