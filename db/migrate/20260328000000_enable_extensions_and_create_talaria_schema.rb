class EnableExtensionsAndCreateTalariaSchema < ActiveRecord::Migration[8.0]
  def change
    enable_extension 'pgcrypto'
    enable_extension 'postgis'

    create_table :ingestion_runs, id: :uuid do |t|
      t.string :status, null: false, default: 'pending'
      t.datetime :started_at
      t.datetime :ended_at
      t.jsonb :config, null: false, default: {}
      t.jsonb :stats, null: false, default: {}
      t.timestamps
    end

    create_table :raw_documents, id: :uuid do |t|
      t.string :source_type, null: false
      t.string :source_uri, null: false
      t.string :source_identifier
      t.string :title
      t.string :language
      t.datetime :fetched_at
      t.string :content_hash
      t.string :license
      t.string :revision_id
      t.jsonb :payload, null: false, default: {}
      t.text :normalized_text
      t.jsonb :metadata, null: false, default: {}
      t.references :ingestion_run, type: :uuid, foreign_key: true
      t.timestamps
    end

    create_table :raw_fragments, id: :uuid do |t|
      t.references :raw_document, null: false, type: :uuid, foreign_key: true
      t.string :fragment_kind, null: false, default: 'paragraph'
      t.string :section_title
      t.integer :ordinal, null: false
      t.text :text, null: false
      t.integer :start_offset
      t.integer :end_offset
      t.jsonb :metadata, null: false, default: {}
      t.timestamps
    end

    create_table :raw_candidates, id: :uuid do |t|
      t.references :raw_fragment, null: false, type: :uuid, foreign_key: true
      t.string :candidate_kind, null: false
      t.jsonb :structured_payload, null: false, default: {}
      t.string :extractor_name, null: false
      t.string :extractor_version, null: false
      t.float :confidence, null: false, default: 0.0
      t.string :status, null: false, default: 'pending'
      t.timestamps
    end

    create_table :raw_ai_judgments, id: :uuid do |t|
      t.references :raw_candidate, type: :uuid, foreign_key: true
      t.references :raw_fragment, type: :uuid, foreign_key: true
      t.string :model_name, null: false
      t.string :task_name, null: false
      t.jsonb :result_json, null: false, default: {}
      t.float :confidence, null: false, default: 0.0
      t.string :payload_hash, null: false
      t.string :status, null: false, default: 'pending'
      t.timestamps
    end

    create_table :raw_place_resolutions, id: :uuid do |t|
      t.references :raw_candidate, null: false, type: :uuid, foreign_key: true
      t.string :resolver_name, null: false
      t.string :query_text, null: false
      t.string :resolved_label
      t.st_point :geom, geographic: true
      t.string :geo_precision
      t.float :confidence, null: false, default: 0.0
      t.boolean :accepted, null: false, default: false
      t.jsonb :metadata, null: false, default: {}
      t.timestamps
    end

    create_table :source_snapshots, id: :uuid do |t|
      t.references :raw_document, null: false, type: :uuid, foreign_key: true
      t.string :storage_key, null: false
      t.bigint :byte_size, null: false, default: 0
      t.string :sha256, null: false
      t.timestamps
    end

    create_table :entities, id: :uuid do |t|
      t.string :entity_type, null: false
      t.string :canonical_name, null: false
      t.text :description
      t.jsonb :external_ids, null: false, default: {}
      t.string :status, null: false, default: 'active'
      t.references :merged_into, type: :uuid, foreign_key: { to_table: :entities }
      t.timestamps
    end

    create_table :entity_aliases, id: :uuid do |t|
      t.references :entity, null: false, type: :uuid, foreign_key: true
      t.string :name, null: false
      t.string :language
      t.string :source_kind
      t.string :source_ref
      t.timestamps
    end

    create_table :places, id: :uuid do |t|
      t.string :canonical_label, null: false
      t.geometry :geom, geographic: true
      t.string :geo_precision
      t.string :resolution_source
      t.float :confidence, null: false, default: 0.0
      t.geometry :bbox, geographic: true
      t.timestamps
    end

    create_table :events, id: :uuid do |t|
      t.references :subject_entity, type: :uuid, foreign_key: { to_table: :entities }
      t.string :event_type, null: false
      t.string :title, null: false
      t.text :summary
      t.string :start_edtf
      t.string :end_edtf
      t.date :start_time
      t.date :end_time
      t.string :date_precision
      t.string :time_kind
      t.references :place, type: :uuid, foreign_key: true
      t.geometry :geom, geographic: true
      t.string :map_status, null: false, default: 'pending'
      t.string :narrative_kind
      t.float :confidence, null: false, default: 0.0
      t.float :canonical_score, null: false, default: 0.0
      t.string :visibility_status, null: false, default: 'public'
      t.timestamps
    end

    create_table :event_participants, id: :uuid do |t|
      t.references :event, null: false, type: :uuid, foreign_key: true
      t.references :entity, null: false, type: :uuid, foreign_key: true
      t.string :role, null: false
      t.float :confidence, null: false, default: 0.0
      t.timestamps
    end

    create_table :claims, id: :uuid do |t|
      t.references :subject_entity, null: false, type: :uuid, foreign_key: { to_table: :entities }
      t.string :predicate, null: false
      t.references :object_entity, type: :uuid, foreign_key: { to_table: :entities }
      t.jsonb :value_json
      t.string :valid_from_edtf
      t.string :valid_to_edtf
      t.date :valid_from
      t.date :valid_to
      t.float :confidence, null: false, default: 0.0
      t.string :status, null: false, default: 'draft'
      t.boolean :exportable, null: false, default: true
      t.timestamps
    end

    create_table :event_evidence, id: :uuid do |t|
      t.references :event, type: :uuid, foreign_key: true
      t.references :claim, type: :uuid, foreign_key: true
      t.references :raw_document, null: false, type: :uuid, foreign_key: true
      t.references :raw_fragment, null: false, type: :uuid, foreign_key: true
      t.references :raw_candidate, type: :uuid, foreign_key: true
      t.references :raw_ai_judgment, type: :uuid, foreign_key: true
      t.text :quoted_text
      t.string :evidence_type, null: false
      t.string :source_kind
      t.float :confidence, null: false, default: 0.0
      t.string :review_status, null: false, default: 'pending'
      t.jsonb :metadata, null: false, default: {}
      t.timestamps
    end

    create_table :media_assets, id: :uuid do |t|
      t.string :label
      t.jsonb :metadata, null: false, default: {}
      t.timestamps
    end

    create_table :entity_links, id: :uuid do |t|
      t.references :from_entity, null: false, type: :uuid, foreign_key: { to_table: :entities }
      t.references :to_entity, null: false, type: :uuid, foreign_key: { to_table: :entities }
      t.string :link_type, null: false
      t.float :confidence, null: false, default: 0.0
      t.timestamps
    end

    add_index :raw_documents, %i[source_type source_identifier]
    add_index :raw_fragments, %i[raw_document_id ordinal], unique: true
    add_index :raw_candidates, :candidate_kind
    add_index :raw_place_resolutions, :geom, using: :gist
    add_index :places, :geom, using: :gist
    add_index :places, :bbox, using: :gist
    add_index :events, :geom, using: :gist

    add_check_constraint :raw_candidates, 'confidence >= 0 AND confidence <= 1', name: 'raw_candidates_confidence_between_0_1'
    add_check_constraint :raw_ai_judgments, 'confidence >= 0 AND confidence <= 1', name: 'raw_ai_judgments_confidence_between_0_1'
    add_check_constraint :raw_place_resolutions, 'confidence >= 0 AND confidence <= 1', name: 'raw_place_resolutions_confidence_between_0_1'
    add_check_constraint :places, 'confidence >= 0 AND confidence <= 1', name: 'places_confidence_between_0_1'
    add_check_constraint :events, 'confidence >= 0 AND confidence <= 1', name: 'events_confidence_between_0_1'
    add_check_constraint :events, 'canonical_score >= 0 AND canonical_score <= 1', name: 'events_canonical_score_between_0_1'
    add_check_constraint :claims, 'confidence >= 0 AND confidence <= 1', name: 'claims_confidence_between_0_1'
    add_check_constraint :event_evidence, 'confidence >= 0 AND confidence <= 1', name: 'event_evidence_confidence_between_0_1'
    add_check_constraint :entity_links, 'confidence >= 0 AND confidence <= 1', name: 'entity_links_confidence_between_0_1'
  end
end
