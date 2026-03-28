require 'rails_helper'

RSpec.describe RawCandidate, type: :model do
  it 'validates confidence range' do
    candidate = build(:raw_candidate, confidence: 1.2)
    expect(candidate).not_to be_valid
  end
end
