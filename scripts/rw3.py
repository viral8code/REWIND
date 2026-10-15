"""Move top-level statements of a REWIND source into `fn main()->Int` (language 3.0.0).

Used by SDK smoke scripts whose inline test programs were written for 2.0.0.
Sources that already contain only declarations are returned unchanged.
"""
import re
_DECL = re.compile(r'^(pub\s+)?(async\s+fn|fn|struct|enum|record|const|import|trait|impl|runtime|type|test\s+fn)\b')
def _end(s, i):
    n = len(s); depth = 0; j = i
    m = re.match(r'\s*(\w+)', s[i:]); head = m.group(1) if m else ''
    block = (bool(_DECL.match(s[i:])) and head not in ('import', 'const', 'type')) or head in ('for', 'while', 'if', 'match', 'loop')
    while j < n:
        c = s[j]
        if c == '/' and s[j:j + 2] == '//':
            j = s.find('\n', j); j = n if j < 0 else j; continue
        if c == '"':
            j += 1
            while j < n and s[j] != '"':
                j += 2 if s[j] == '\\' else 1
            j += 1; continue
        if c in '([{': depth += 1
        elif c in ')]}':
            depth -= 1
            if depth == 0 and c == '}' and block:
                k = j + 1
                m = re.match(r'\s*(else|;)', s[k:])
                if m and m.group(1) == 'else':
                    j = k + m.end(); continue
                if re.match(r'\s*\{', s[k:]):
                    j = k; continue
                return k
        elif c == ';' and depth == 0:
            return j + 1
        j += 1
    return n
def to_main(src):
    decls = []; stmts = []; i = 0; n = len(src)
    while i < n:
        lead = re.match(r'(\s*(?://[^\n]*\n\s*)*)', src[i:]).group(1); i += len(lead)
        if i >= n: break
        e = _end(src, i); item = src[i:e]
        (decls if _DECL.match(item) else stmts).append(lead + item); i = e
    if not stmts: return src
    while stmts and stmts[-1].strip() == 'publish;': stmts.pop()
    body = ''.join(stmts).strip('\n')
    body = '\n'.join(('    ' + l if l.strip() else l) for l in body.split('\n'))
    return ''.join(decls).rstrip() + '\n\nfn main()->Int {\n' + body + '\n    return 0;\n}\n'

def install():
    """Make Path.write_text migrate legacy top-level programs written to *.rw files."""
    import pathlib
    original = pathlib.Path.write_text
    def write_text(self, data, *args, **kwargs):
        if self.suffix == '.rw' and isinstance(data, str):
            data = to_main(data)
        return original(self, data, *args, **kwargs)
    pathlib.Path.write_text = write_text
