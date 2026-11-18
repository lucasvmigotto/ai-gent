#!/usr/bin/env python3
"""Validate this planning program, not runtime behavior or provider attestation.

Uses only the standard library. The schema checker implements the exact keyword
subset used by this plan; it is not a general JSON Schema conformance engine.
"""
from pathlib import Path
import json
import re
import sys
import uuid

ROOT = Path(__file__).resolve().parents[3]
PROGRAM = Path(__file__).resolve().with_name('program.json')
ERRORS = []

def check(condition, message):
    if not condition:
        ERRORS.append(message)

def matches(value, schema):
    if 'anyOf' in schema:
        return any(matches(value, item) for item in schema['anyOf'])
    if 'const' in schema and (type(value) is not type(schema['const']) or value != schema['const']):
        return False
    if 'enum' in schema and value not in schema['enum']:
        return False
    types = schema.get('type', [])
    types = [types] if isinstance(types, str) else types
    actual = ('null' if value is None else 'boolean' if isinstance(value, bool)
              else 'integer' if isinstance(value, int) else 'string'
              if isinstance(value, str) else 'array' if isinstance(value, list)
              else 'object' if isinstance(value, dict) else 'unsupported')
    if types and actual not in types:
        return False
    if actual == 'integer':
        if value < schema.get('minimum', value) or value > schema.get('maximum', value):
            return False
    if actual == 'string':
        if not schema.get('minLength', 0) <= len(value) <= schema.get('maxLength', len(value)):
            return False
        if 'pattern' in schema and not re.search(schema['pattern'], value):
            return False
        if schema.get('format') == 'uuid':
            try:
                uuid.UUID(value)
            except ValueError:
                return False
    if actual == 'array':
        if not schema.get('minItems', 0) <= len(value) <= schema.get('maxItems', len(value)):
            return False
        if not all(matches(item, schema.get('items', {})) for item in value):
            return False
    if actual == 'object':
        if not set(schema.get('required', [])).issubset(value):
            return False
        if len(value) > schema.get('maxProperties', len(value)):
            return False
        properties = schema.get('properties', {})
        for key, item in value.items():
            if key in properties:
                if not matches(item, properties[key]):
                    return False
            elif schema.get('additionalProperties', True) is False:
                return False
            elif isinstance(schema.get('additionalProperties'), dict):
                if not matches(item, schema['additionalProperties']):
                    return False
    return True

def main():
    data = json.loads(PROGRAM.read_text())
    features = data['features']
    ids = [f['id'] for f in features]
    check(len(ids) == len(set(ids)) == 9, 'feature IDs must be unique and cover nine features')
    check(ids == list(range(46, 55)), 'reserved feature range changed; reconcile all references')
    check(data['status'] in ('Planned', 'In progress'), 'unsupported program status')
    visiting, visited = set(), set()
    def visit(ident):
        if ident in visiting:
            check(False, f'dependency cycle at {ident}')
            return
        if ident in visited:
            return
        visiting.add(ident)
        f = next(f for f in features if f['id'] == ident)
        for dep in f['depends']:
            check(dep in ids, f'{ident}: unknown dependency {dep}')
            check(dep < ident, f'{ident}: numbering not topological for dependency {dep}')
            if dep in ids:
                visit(dep)
        visiting.remove(ident)
        visited.add(ident)
    for ident in ids:
        visit(ident)
    task_total = requirement_total = completed_total = 0
    markdown = []
    for f in features:
        slug = f"{f['id']:03}-{f['slug']}"
        p = ROOT / 'specs' / slug
        required = ['spec.md', 'plan.md', 'research.md', 'data-model.md', 'quickstart.md',
                    'tasks.md', 'backend.md', 'qa.md', 'analysis.md',
                    'contracts/cli.md', 'contracts/operations.md', 'checklists/requirements.md']
        for name in required:
            check((p/name).is_file(), f'{slug}: missing {name}')
        graph = json.loads((p/'task-graph.json').read_text())
        graph_nodes = {node['id']: node for node in graph['tasks']}
        check(len(graph['tasks']) == len(graph_nodes) == 22, f'{slug}: incomplete task DAG')
        check(set(graph_nodes) == {f'T{i:03}' for i in range(1, 23)}, f'{slug}: task DAG IDs diverge')
        done, pending = set(), set(graph_nodes)
        while pending:
            ready = {tid for tid in pending if set(graph_nodes[tid]['depends']) <= done}
            if not ready:
                check(False, f'{slug}: task graph cycle or unknown predecessor {sorted(pending)}')
                break
            done.update(ready)
            pending.difference_update(ready)
        spec = (p/'spec.md').read_text()
        tasks = (p/'tasks.md').read_text()
        qa = (p/'qa.md').read_text()
        check(any('**Status**: '+status in spec for status in ('Planned', 'In progress')), f'{slug}: unsupported feature status')
        check(not re.search(r'NEEDS CLARIFICATION|\[TODO|\[TBD|ACTION REQUIRED', spec + tasks),
              f'{slug}: unresolved placeholder')
        found = re.findall(r'^- \[[ xX]\] (T\d+) (.+)$', tasks, re.M)
        check(len(found) == 22 and [x[0] for x in found] == [f'T{i:03}' for i in range(1,23)],
              f'{slug}: task sequence/count wrong')
        if '**Status**: Planned' in spec:
            check(not re.search(r'^- \[[xX]\] T\d+', tasks, re.M), f'{slug}: Planned feature has completed tasks')
        for tid, text in found:
            check('Executor:' in text and 'Reviewer:' in text and 'Effort:' in text,
                  f'{slug}/{tid}: missing role/effort')
            check('`' in text, f'{slug}/{tid}: no concrete file reference')
        frs = re.findall(r'^- \*\*(FR-\d+)\*\*', spec, re.M)
        check(len(frs) == 9 and len(set(frs)) == 9, f'{slug}: requirement count/IDs')
        for fr in frs:
            check(fr in tasks and fr in qa, f'{slug}/{fr}: missing task or test mapping')
        for i, story in enumerate(f['stories'],1):
            check(f'User Story {i}' in spec and f'[US{i}]' in tasks,
                  f'{slug}: missing story {i} task mapping')
            check(story['test'] in qa and story['positive'] in spec and story['negative'] in spec,
                  f'{slug}: missing story {i} acceptance evidence')
        for sc in ('SC-001','SC-002','SC-003'):
            check(sc in qa and sc in tasks, f'{slug}: missing {sc} coverage')
        check('## Backend' in tasks and '## QA' in tasks, f'{slug}: missing layer tasks')
        completed_total += len(re.findall(r'^- \[[xX]\] T\d+', tasks, re.M))
        task_total += len(found)
        requirement_total += len(frs)
        markdown.extend(p.rglob('*.md'))
    audits = {a for f in features for a in f['audit']}
    check(audits == {f'F{i:02}' for i in range(1,15)}, 'audit findings F01–F14 not fully covered')
    markdown.extend(PROGRAM.parent.glob('*.md'))
    for p in markdown:
        for target in re.findall(r'\[[^\]]*\]\(([^)]+)\)', p.read_text()):
            if '://' in target or target.startswith('#'):
                continue
            path = target.split('#',1)[0].split(':',1)[0]
            check((p.parent/path).exists(), f'{p.relative_to(ROOT)}: broken link {target}')
    for p in list(PROGRAM.parent.rglob('*.json')) + list((ROOT/'specs').glob('04[6-9]-*/contracts/**/*.json')) + list((ROOT/'specs').glob('05[0-4]-*/contracts/**/*.json')):
        try:
            json.loads(p.read_text())
        except ValueError as error:
            check(False, f'{p}: invalid JSON {error}')
    usage_dir = ROOT/'specs/046-task-usage-ledger/contracts'
    usage = json.loads((usage_dir/'usage-observation.schema.json').read_text())
    check(matches(json.loads((usage_dir/'examples/usage-valid.json').read_text()), usage),
          'valid usage fixture fails planned schema subset')
    check(not matches(json.loads((usage_dir/'examples/usage-invalid-negative.json').read_text()), usage),
          'negative usage fixture unexpectedly accepted')
    receipt_dir = ROOT/'specs/050-metered-completion/contracts'
    receipt = json.loads((receipt_dir/'completion-receipt.schema.json').read_text())
    for name in ('receipt-valid-shape.json', 'receipt-invalid-domain.json'):
        check(matches(json.loads((receipt_dir/'examples'/name).read_text()), receipt),
              f'{name} must be schema-valid; domain verification is separate')
    index = (ROOT/'specs/README.md').read_text()
    for ident in range(44,55):
        check(f'| {ident:03} |' in index, f'index missing feature {ident}')
    if ERRORS:
        print('\n'.join('FAIL: '+error for error in ERRORS), file=sys.stderr)
        return 1
    print(f'Plan structural checks passed: {len(features)} features, 27 stories, '
          f'{requirement_total} requirements, {task_total} tasks ({completed_total} completed); DAG, links and schema examples valid.')
    print('This validates planning structure and schema subset only; no runtime, live host or billing claim.')
    return 0

if __name__ == '__main__':
    sys.exit(main())
