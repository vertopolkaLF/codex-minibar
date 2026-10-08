"""Regenerate the translator template from the authoritative English catalog."""
from pathlib import Path
import re


def generate(source):
    messages = []
    current = None
    for line in source.splitlines():
        match = re.match(r'^([a-z][a-z0-9-]*) = ', line)
        if match:
            current = (match.group(1), [line])
            messages.append(current)
        elif current and line.startswith((' ', '\t')):
            current[1].append(line)
        else:
            current = None
    result = [
        '### Translator template generated from locales/en/app.ftl.',
        '### Copy to a locale directory, keep IDs and $parameters, and translate values.',
        '### Empty or missing values fall back to English at runtime.',
        '',
    ]
    for key, lines in messages:
        result.extend(['# English source:'] + ['# ' + line for line in lines])
        result.extend([key + ' = { "" }', ''])
    return '\n'.join(result)


if __name__ == '__main__':
    source = Path('locales/en/app.ftl').read_text(encoding='utf-8')
    Path('locales/template/app.ftl').write_text(generate(source), encoding='utf-8')
