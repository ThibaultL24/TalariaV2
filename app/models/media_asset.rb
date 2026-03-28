class MediaAsset < ApplicationRecord
  has_one_attached :file
end
