#!/usr/bin/env python3
"""Validate both OpenAPI copies and all administrative schema references."""
from pathlib import Path
import yaml
from jsonschema import Draft202012Validator

root = Path(__file__).resolve().parents[1]
source = (root / 'docs/openapi-v3.yml').read_bytes()
assert source == (root / 'web/src/lib/schemas/openapi-v3.yml').read_bytes(), 'OpenAPI copies diverge'
doc = yaml.safe_load(source)

def walk(value):
    if isinstance(value, dict):
        if '$ref' in value and value['$ref'].startswith('#/'):
            target = doc
            for segment in value['$ref'][2:].split('/'):
                target = target[segment]
        for child in value.values():
            walk(child)
    elif isinstance(value, list):
        for child in value:
            walk(child)

walk(doc)
for name, schema in doc['components']['schemas'].items():
    if name.startswith('AdminIngestion'):
        Draft202012Validator.check_schema(schema)
for path, item in doc['paths'].items():
    if path.startswith('/api/v1/admin/ingestion/') or path.endswith('/metrics'):
        operation = item['get']
        assert operation['security'] == [{'TalariaSession': []}], path
        assert {'200','400','401','403','404','500'} <= operation['responses'].keys(), path
print('OpenAPI copies, admin security, responses, schemas and references validated')
