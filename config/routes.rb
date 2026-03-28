Rails.application.routes.draw do
  namespace :api do
    resources :entities, only: [:show] do
      get :events, on: :member
      get :graph_context, on: :member
    end
    resources :events, only: [:show] do
      get :evidence, on: :member
      get :export_graph, on: :member
    end
    resources :claims, only: [] do
      get :export_graph, on: :member
    end

    get 'map/features', to: 'map#features'
    get 'timeline', to: 'timeline#index'
    get 'search', to: 'search#index'

    resources :ingestion_runs, only: [:create]
    post 'sources/:source_type/import', to: 'sources#import'
    post 'raw_documents/:id/reprocess', to: 'raw_documents#reprocess'
    post 'raw_candidates/:id/promote', to: 'raw_candidates#promote'

    namespace :admin do
      get :review_queue, to: 'review_queue#index'
      patch 'review_queue/:id', to: 'review_queue#update'
    end
  end
end
