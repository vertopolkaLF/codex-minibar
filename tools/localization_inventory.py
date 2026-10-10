"""Inventory Rust string literals without treating comments as application copy.

Run from the repository root. This is an audit aid; protocol values and UI copy
must be distinguished by a reviewer before introducing translation calls.
"""
import json
import re
import argparse
import csv
import subprocess
from pathlib import Path

TOKEN = re.compile(r'//[^\n]*|/\*[\s\S]*?\*/|(?:b?r)(?P<hash>\#*)"[\s\S]*?"(?P=hash)|b?"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])\'')

def literals(source):
    for match in TOKEN.finditer(source):
        raw = match.group()
        if raw.startswith(('//', '/*', "'", 'b')):
            continue
        if raw.startswith('r'):
            value = raw[raw.index('"') + 1:raw.rindex('"')]
        else:
            try:
                value = json.loads(raw)
            except ValueError:
                value = raw[1:-1].replace('\\n', '\n').replace('\\"', '"')
        yield match.start(), match.end(), value

def inventory(revision=None):
    rows = []
    for path in sorted(Path('src').rglob('*.rs')):
        if revision:
            tracked = subprocess.run(['git', 'show', f'{revision}:{path.as_posix()}'],
                                     capture_output=True)
            if tracked.returncode:
                continue
            source = tracked.stdout.decode('utf-8')
        else:
            source = path.read_text(encoding='utf-8')
        tests = re.search(r'#\[cfg\(test\)\]\s*mod\s+tests\s*\{', source)
        if tests:
            source = source[:tests.start()]
        if path.name == 'tests.rs':
            continue
        for start, end, value in literals(source):
            if not value:
                continue
            rows.append(dict(file=path.as_posix(), line=source.count('\n', 0, start)+1,
                             start=start, end=end, text=value,
                             context=source[max(0, start-90):min(len(source), end+60)]))
    return rows

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--revision', help='Optional Git source revision to inventory')
    args = parser.parse_args()
    rows = inventory(args.revision)
    Path('docs/localization').mkdir(parents=True, exist_ok=True)
    with Path('docs/localization/string-inventory.tsv').open('w', encoding='utf-8', newline='') as output:
        writer = csv.writer(output, delimiter='\t', lineterminator='\n')
        writer.writerow(['file', 'line', 'literal'])
        for row in rows:
            writer.writerow([row['file'], row['line'],
                             row['text'].replace('\n', '\\n').replace('\t', '\\t')])
    print(f'{len(rows)} non-test string occurrences inventoried')
