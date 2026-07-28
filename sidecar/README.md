# COSMOS sidecar (phrase-candidate extraction)

## Setup

```bash
git clone https://github.com/ZhangDataLab/COSMOS.git cosmos
python3 -m venv .venv
source .venv/bin/activate
# Minimal deps for preprocessing/tuple_extraction.py (CPU). Full torch CUDA stack from
# cosmos/requirements.txt is NOT required for phrase-candidate extraction.
pip install "numpy<1.24" "nltk==3.8.1" "spacy==3.7.5" "timexy==0.1.3" "benepar==0.2.0" "protobuf<3.21"
python -m spacy download en_core_web_trf
python -c "import nltk; nltk.download('punkt'); nltk.download('punkt_tab')"
```

Point `.env` at the venv:

```bash
COSMOS_PYTHON=/absolute/path/to/TalariaV2/sidecar/.venv/bin/python
COSMOS_BATCH_SCRIPT=/absolute/path/to/TalariaV2/sidecar/cosmos_batch.py
```

Install benepar + timexy per COSMOS docs (covered by the pip line above).

## Batch runner

Talaria calls `sidecar/cosmos_batch.py` (override with `COSMOS_BATCH_SCRIPT`):

```bash
# Input: [{ "id": "<sentence-uuid>", "text": "..." }, ...]
# Output: [{ "id": "...", "tuples": [{ "person", "time", "place", "verb" }] }]
python3 sidecar/cosmos_batch.py --input batch.json --output out.json
```

## Engine command

```bash
# Dev without models:
cargo run -p talaria-api -- cosmos-extract --mock --skip-existing

# Production (loads spaCy once per batch subprocess):
cargo run -p talaria-api -- cosmos-extract --batch-size 32 --skip-existing
```

Each batch spawns Python, loads COSMOS models, processes sentences, returns JSON.
For large dumps, consider a long-running sidecar HTTP service (future).
