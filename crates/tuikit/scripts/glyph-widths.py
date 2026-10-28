#!/usr/bin/env python3
"""Measure whether glyphs fit one terminal cell in common monospace fonts.

    scripts/glyph-widths.py GLYPH...          # measure the glyphs given
    scripts/glyph-widths.py --source [DIR...] # every non-ASCII character the
                                              # Rust sources under each DIR draw
                                              # (default: src)

A glyph the monospace font lacks is drawn from a fallback font, and when
that font's advance is wider than a cell the glyph spills into its
neighbor, so a selection highlight cuts it in half. Prefer glyphs that
DejaVu Sans Mono carries at exactly 1.00. Record the result in the vetted
table (`tuikit::glyphs::VettedGlyphs::register`; the framework's own
glyphs are in `src/glyphs/vetted.rs`, and a host registers its own from an
adapter). Pure Python, no packages: it reads the fonts' cmap and hmtx
tables directly.

`--source` skips comment lines, test files (`tests.rs`, `*_tests.rs`), and
directories named `tests`, `snapshots`, or ending in `_tests`. It does not
skip a `#[cfg(test)]` module inside a source file, so it can list a glyph
the Rust scanner (`tuikit::glyphs::drawn_glyphs`) would not.
"""
import struct, sys, subprocess, unicodedata
def tables(d):
    n=struct.unpack('>H',d[4:6])[0]; t={}
    for i in range(n):
        tag,cs,off,ln=struct.unpack('>4sIII',d[12+16*i:28+16*i]); t[tag.decode()]=(off,ln)
    return t
def load(path):
    d=open(path,'rb').read()
    if d[:4]==b'ttcf':
        off=struct.unpack('>I',d[12:16])[0]; d=d[off:] if False else d
    t=tables(d)
    upm=struct.unpack('>H',d[t['head'][0]+18:t['head'][0]+20])[0]
    nh=struct.unpack('>H',d[t['hhea'][0]+34:t['hhea'][0]+36])[0]
    hm=t['hmtx'][0]
    def adv(g):
        g=min(g,nh-1); return struct.unpack('>H',d[hm+4*g:hm+4*g+2])[0]
    cm=t['cmap'][0]; nsub=struct.unpack('>H',d[cm+2:cm+4])[0]; maps={}
    for i in range(nsub):
        pid,eid,so=struct.unpack('>HHI',d[cm+4+8*i:cm+12+8*i]); s=cm+so
        fmt=struct.unpack('>H',d[s:s+2])[0]
        if fmt==4:
            sc=struct.unpack('>H',d[s+6:s+8])[0]//2
            ends=struct.unpack('>%dH'%sc,d[s+14:s+14+2*sc]); st=s+16+2*sc
            starts=struct.unpack('>%dH'%sc,d[st:st+2*sc]); dl=st+2*sc
            deltas=struct.unpack('>%dh'%sc,d[dl:dl+2*sc]); ro=dl+2*sc
            ros=struct.unpack('>%dH'%sc,d[ro:ro+2*sc])
            for k in range(sc):
                for c in range(starts[k],ends[k]+1):
                    if c==0xFFFF: continue
                    if ros[k]==0: g=(c+deltas[k])&0xFFFF
                    else:
                        p=ro+2*k+ros[k]+2*(c-starts[k]); g=struct.unpack('>H',d[p:p+2])[0]
                        if g: g=(g+deltas[k])&0xFFFF
                    if g: maps[c]=g
        elif fmt==12:
            ng=struct.unpack('>I',d[s+12:s+16])[0]
            for k in range(ng):
                a,b,g0=struct.unpack('>III',d[s+16+12*k:s+28+12*k])
                for c in range(a,b+1): maps[c]=g0+c-a
    return upm,adv,maps

import glob, os
FONTS = {
    'DejaVu Sans Mono': '/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf',
    'Liberation Mono': '/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf',
    'Andale Mono': '/usr/local/share/fonts/msttcorefonts/Andale Mono/andalemo.ttf',
    'Courier New': '/usr/local/share/fonts/msttcorefonts/Courier/Courier_New.ttf',
}
FALLBACK = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'


def drawn_glyphs(roots):
    """Non-ASCII characters on non-comment lines of the Rust sources under roots."""
    seen = []
    for root in roots:
        for dirpath, _, names in os.walk(root):
            for name in names:
                is_test_file = name == 'tests.rs' or name.endswith('_tests.rs')
                in_test_dir = any(d == 'tests' or d.endswith('_tests') or d == 'snapshots' for d in dirpath.split(os.sep))
                if not name.endswith('.rs') or is_test_file or in_test_dir:
                    continue
                for line in open(os.path.join(dirpath, name), encoding='utf-8'):
                    if line.lstrip().startswith('//'):
                        continue
                    for ch in line:
                        if ord(ch) > 0x7F and ch not in seen:
                            seen.append(ch)
    return seen


def main():
    args = sys.argv[1:]
    if args[:1] == ['--source']:
        glyphs = drawn_glyphs(args[1:] or ['src'])
    else:
        glyphs = args
    if not glyphs:
        print('usage: glyph-widths.py GLYPH... | --source [DIR...]', file=sys.stderr)
        return 2
    fonts = {}
    for name, path in FONTS.items():
        if os.path.exists(path):
            fonts[name] = load(path)
    if 'DejaVu Sans Mono' not in fonts:
        print('DejaVu Sans Mono is required as the baseline font', file=sys.stderr)
        return 1
    fallback = load(FALLBACK) if os.path.exists(FALLBACK) else None
    upm, adv, maps = fonts['DejaVu Sans Mono']
    cell = adv(maps[ord('M')]) / upm
    print('advance as a fraction of one cell; MISSING means the terminal falls back to another font')
    print('%-5s %-8s %-34s' % ('glyph', 'cp', 'name'), *['%-17s' % n for n in fonts], 'fallback (DejaVu Sans)')
    for g in glyphs:
        c = ord(g)
        row = []
        for n, (u, a, m) in fonts.items():
            c0 = a(m[ord('M')]) / u
            row.append('%.2f' % (a(m[c]) / u / c0) if c in m else 'MISSING')
        fb = ''
        if fallback and c in fallback[2] and c not in maps:
            fb = '%.2f' % (fallback[1](fallback[2][c]) / fallback[0] / cell)
        print('%-5s U+%04X  %-34s' % (g, c, unicodedata.name(g, '?')[:34]), *['%-17s' % r for r in row], fb)
    return 0


sys.exit(main())
