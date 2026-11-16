#!/usr/bin/env python3
"""Draw the showcase set: six motifs and eleven pattern documents.

Run it with no arguments; it writes into its own directory.

    python3 examples/showcase/make-showcase.py

Generated rather than hand-written because several of these documents make a
claim about each other that a person editing one by hand would quietly break.
The three grid documents must be identical apart from a single field, and the
two colourway documents identical apart from the palette. If that stops being
true the pictures still render and the captions start lying.

The first half is a written style brief, held as code so the five read as one hand:

  Flat colour only. No gradients, no shading, no texture, no outline: shapes
  are defined where one flat colour meets another. Geometric simplification,
  so each subject reduces to a handful of clean shapes and stays readable at
  20 mm across. Three to four colours per motif, all of them from the one
  shared palette. One subject, upright, filling its frame.

Every path's bounding box is made to fill the viewBox exactly, because the
engine maps a drawing's bounds onto `natural_size_mm` and a margin inside the
frame would therefore shrink the subject rather than pad it.
"""
import math, os, sys, json, copy

OUT = os.path.dirname(os.path.abspath(__file__))

PAPER = "#faf6f0"; INK = "#141c3a"; BRASS = "#c68b3c"
SAGE = "#8fa98b"; FOREST = "#3f6b52"; TERRA = "#b4512f"
BLUSH = "#e8c4b0"; CREAM = "#f2e3c6"; SLATE = "#5b7fa6"


def svg(name, w, h, body, mm_w, mm_h):
    s = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{mm_w}mm" '
         f'height="{mm_h}mm" viewBox="0 0 {w} {h}">\n{body}</svg>\n')
    open(os.path.join(OUT, name), "w").write(s)
    print(f"  {name}  {w}x{h} units, {mm_w}x{mm_h} mm")


def leaf(cx, cy, length, width, angle, fill):
    """A pointed lens, tip outward, hinged at (cx, cy)."""
    d = (f"M 0 0 Q {length*0.45} {-width/2} {length} 0 "
         f"Q {length*0.45} {width/2} 0 0 Z")
    return (f'<path d="{d}" fill="{fill}" '
            f'transform="translate({cx},{cy}) rotate({angle})"/>\n')


# ---------------------------------------------------------------- 1. sprig
# Tall and airy. The lightest thing in the set, so a dense scatter of it
# still reads as open.
b = ""
b += '<path d="M 38 150 L 38 18" stroke="%s" stroke-width="4.4" fill="none" stroke-linecap="round"/>\n' % FOREST
for i, y in enumerate(range(34, 142, 21)):
    side = -1 if i % 2 else 1
    b += leaf(38, y, 36, 21, side * 32 + (0 if side > 0 else 180), SAGE)
b += leaf(38, 18, 26, 15, -90, SAGE)
svg("sprig.svg", 76, 152, b, 44, 88)

# ---------------------------------------------------------------- 2. bloom
# The set's anchor: round, symmetrical, high contrast. Eight petals rather
# than six, because six reads as a snowflake and eight as a flower.
b = ""
R, PET_L, PET_W = 50, 34, 26
for i in range(8):
    b += leaf(R, R, PET_L, PET_W, i * 45, TERRA)
for i in range(8):
    b += leaf(R, R, PET_L * 0.52, PET_W * 0.5, i * 45 + 22.5, BLUSH)
b += f'<circle cx="{R}" cy="{R}" r="13" fill="{BRASS}"/>\n'
b += f'<circle cx="{R}" cy="{R}" r="6" fill="{CREAM}"/>\n'
svg("bloom.svg", 100, 100, b, 46, 46)

# ------------------------------------------------------------- 3. seedhead
# Radial lines, which is the one texture in the set that is not a mass. It
# gives a scatter somewhere to breathe between the solid shapes.
b = ""
C, SPOKE, DOT = 50, 40, 4.6
for i in range(16):
    a = math.radians(i * 22.5)
    x, y = C + math.cos(a) * SPOKE, C + math.sin(a) * SPOKE
    b += (f'<path d="M {C} {C} L {x:.2f} {y:.2f}" stroke="{INK}" '
          f'stroke-width="1.8" stroke-linecap="round"/>\n')
    b += f'<circle cx="{x:.2f}" cy="{y:.2f}" r="{DOT}" fill="{BRASS}"/>\n'
b += f'<circle cx="{C}" cy="{C}" r="7" fill="{INK}"/>\n'
svg("seedhead.svg", 100, 100, b, 40, 40)

# ----------------------------------------------------------------- 4. leaf
# One broad leaf, the largest single mass, for the statement repeats.
b = ""
b += ('<path d="M 34 170 C 34 130 0 108 0 62 C 0 22 18 0 34 0 '
      'C 50 0 68 22 68 62 C 68 108 34 130 34 170 Z" fill="%s"/>\n' % FOREST)
b += ('<path d="M 34 150 C 34 120 8 104 8 62 C 8 28 22 8 34 8 Z" '
      'fill="%s"/>\n' % SAGE)
for vy, vl in ((40, 20), (66, 25), (94, 23), (120, 17)):
    b += (f'<path d="M 34 {vy} L {34 + vl} {vy + vl*0.75:.0f}" stroke="{CREAM}" '
          f'stroke-width="2.2" fill="none" stroke-linecap="round"/>\n')
b += ('<path d="M 34 168 L 34 6" stroke="%s" stroke-width="3" fill="none" '
      'stroke-linecap="round"/>\n' % CREAM)
svg("leaf.svg", 68, 170, b, 30, 75)

# ---------------------------------------------------------------- 5. berry
# Three circles on forked stems: the set's only repeated geometric form, and
# the one that holds up smallest.
b = ""
b += ('<path d="M 46 132 L 46 86 M 46 100 L 16 64 M 46 100 L 76 60" '
      'stroke="%s" stroke-width="3.6" fill="none" stroke-linecap="round"/>\n' % FOREST)
b += leaf(46, 112, 24, 13, -150, SAGE)
for cx, cy, r in ((46, 70, 16), (14, 50, 13), (78, 46, 14)):
    b += f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{TERRA}"/>\n'
    b += f'<circle cx="{cx - r*0.3:.1f}" cy="{cy - r*0.32:.1f}" r="{r*0.32:.1f}" fill="{BLUSH}"/>\n'
svg("berry.svg", 92, 132, b, 42, 60)

# ------------------------------------------------------------------ 6. bud
# The filler. Every strewn pattern needs something small enough to sit in the
# gaps without competing, and scaling a large motif down to do that job gives
# detail nobody can see at a cost everybody pays.
b = ""
b += ('<path d="M 22 76 L 22 44" stroke="%s" stroke-width="3.4" fill="none" '
      'stroke-linecap="round"/>\n' % FOREST)
b += leaf(22, 62, 20, 12, -140, SAGE)
b += ('<path d="M 22 0 C 40 14 40 34 22 44 C 4 34 4 14 22 0 Z" fill="%s"/>\n' % TERRA)
b += ('<path d="M 22 6 C 32 17 32 31 22 40 Z" fill="%s"/>\n' % BLUSH)
svg("bud.svg", 44, 76, b, 22, 38)

print("drawn")

# ============================================================ documents

def rgb(h):
    h = h.lstrip("#")
    return {"r": int(h[0:2], 16), "g": int(h[2:4], 16), "b": int(h[4:6], 16)}

# Slot 0 is the fabric. Everything else is a slot the artwork and the grounds
# resolve through, which is what makes a colourway a palette swap rather than a
# redraw.
DAY = ["#faf6f0", "#141c3a", "#c68b3c", "#8fa98b", "#3f6b52",
       "#b4512f", "#e8c4b0", "#f2e3c6", "#5b7fa6", "#2a3358"]
# The same ten slots, repainted for a dark cloth. Slot 1 is the drawn ink, so
# it has to become light or the seedhead's spokes disappear into the ground.
NIGHT = ["#141c3a", "#f2e3c6", "#c68b3c", "#a8c0a3", "#7d9b76",
         "#d9a05b", "#e8c4b0", "#f7efdf", "#7fa3c9", "#2a3358"]

MOTIFS = [
    ("sprig",    "sprig.svg",    [44.0, 88.0]),
    ("bloom",    "bloom.svg",    [46.0, 46.0]),
    ("seedhead", "seedhead.svg", [40.0, 40.0]),
    ("leaf",     "leaf.svg",     [30.0, 75.0]),
    ("berry",    "berry.svg",    [42.0, 60.0]),
    ("bud",      "bud.svg",      [22.0, 38.0]),
]

def motifs(*ids, **sizes):
    """Motif entries. `sizes={"bud": 0.6}` scales that motif's natural size.

    Scale belongs to the document, not to the drawing: the same bud is a
    40 mm statement on one cloth and a 14 mm filler on another, and the
    engine measures in millimetres on the cloth rather than pixels in the
    file, so this is the only place that decision can live."""
    by = {m[0]: m for m in MOTIFS}
    out = []
    for i in ids:
        w, h = by[i][2]
        k = sizes.get(i, 1.0)
        out.append({"id": i, "file": by[i][1],
                    "natural_size_mm": [round(w * k, 1), round(h * k, 1)]})
    return out

def palette(colors):
    return {"colors": [rgb(c) for c in colors]}

def mix(*pairs):
    return [{"motif_id": m, "weight": w} for m, w in pairs]

def doc(name, d):
    with open(os.path.join(OUT, name), "w") as f:
        json.dump(d, f, indent=2)
        f.write("\n")
    print(f"  {name}")

TILE = [300.0, 225.0]
BLOCK = {"u": [300.0, 0.0], "v": [0.0, 225.0]}
HALF_DROP = {"u": [300.0, 112.5], "v": [0.0, 225.0]}

# ------------------------------------------------------- 1. the hero, strewn
doc("night-strewn.json", {
    "motifs": motifs("bloom", "sprig", "seedhead", "bud", "berry",
                     bloom=0.8, sprig=0.78, seedhead=0.8, bud=0.9, berry=0.8),
    "design_palette": palette(DAY),
    "scatter": {
        "density": 0.52, "prevent_overlap": True, "mirror": "none",
        "on_axis_share": 0.0, "rotation_jitter_deg": 26.0,
        "scale_range": [0.85, 1.15],
        "mix": mix(("bloom", 2.0), ("sprig", 1.6), ("seedhead", 1.3),
                   ("bud", 2.2), ("berry", 1.4)),
        "seed": 20261116,
    },
    "pattern": {
        "tile_size_mm": TILE, "lattice": HALF_DROP, "placements": [],
        "palette": palette(NIGHT), "seed": 20261116,
    },
})

# ---------------------------------------------- 2-4. one field, three prints
BASE_GRID = {
    "motifs": motifs("bloom", "leaf", "berry"),
    "grid": {
        "rows": 4, "cols": 4, "prevent_overlap": True,
        "row_offset": 0.0, "col_offset": 0.0,
        "rotation_jitter_deg": 0.0, "scale_range": [1.0, 1.0],
        "mix": mix(("bloom", 2.0), ("leaf", 1.5), ("berry", 1.5)),
        "seed": 4,
    },
    "pattern": {
        "tile_size_mm": TILE, "lattice": BLOCK, "placements": [],
        "palette": palette(DAY), "seed": 4,
    },
}
for name, field, value in [("grid-block.json", None, None),
                           ("grid-brick.json", "row_offset", 0.5),
                           ("grid-half-drop.json", "col_offset", 0.5)]:
    d = copy.deepcopy(BASE_GRID)
    if field:
        d["grid"][field] = value
    doc(name, d)

# ---------------------------------------- 5-6. one drawing, two colourways
COLOURWAY = {
    "motifs": motifs("bloom", "sprig", "berry", "bud"),
    "scatter": {
        "density": 0.26, "prevent_overlap": True, "mirror": "none",
        "on_axis_share": 0.0, "rotation_jitter_deg": 20.0,
        "scale_range": [0.9, 1.1],
        "mix": mix(("bloom", 2.0), ("sprig", 1.5), ("berry", 1.5), ("bud", 2.0)),
        "seed": 61,
    },
    "pattern": {
        "tile_size_mm": TILE, "lattice": BLOCK, "placements": [],
        "palette": palette(DAY), "seed": 61,
    },
}
# Both carry `design_palette`, including the one where it equals the render
# palette. It is redundant there and it is the point: the artwork is matched
# to slots against the colours it was drawn in, and what gets painted is a
# separate decision. With it present in both, the two files differ in exactly
# one field, which is what the pair claims.
day = copy.deepcopy(COLOURWAY)
day["design_palette"] = palette(DAY)
doc("colourway-day.json", day)
night = copy.deepcopy(COLOURWAY)
night["design_palette"] = palette(DAY)
night["pattern"]["palette"] = palette(NIGHT)
doc("colourway-night.json", night)

# -------------------------------------------------------- 7-9. the grounds
# A sett is symmetric about its own ends, which is what makes a tartan look
# woven rather than striped. Sage and navy are the two grounds, with a brass
# overcheck and one cream line through the middle of the navy.
SETT = [
    {"width_mm": 42.0, "slot": 3}, {"width_mm": 2.5, "slot": 2},
    {"width_mm": 9.0, "slot": 3}, {"width_mm": 2.5, "slot": 2},
    {"width_mm": 26.0, "slot": 9}, {"width_mm": 3.0, "slot": 7},
    {"width_mm": 26.0, "slot": 9}, {"width_mm": 2.5, "slot": 2},
    {"width_mm": 9.0, "slot": 3}, {"width_mm": 2.5, "slot": 2},
]

# Every crossing, named. Without an entry the horizontal thread simply wins,
# and a sett with no crossings therefore reads as horizontal bars with the
# vertical bands showing only in the gaps, which is what this looked like on
# the first attempt. The two ground colours chequer, by resolving each way
# round to the opposite colour; the fine brass and cream threads stay on top
# of both, because an overcheck that disappears under the ground is not an
# overcheck.
def _cross(pairs):
    return [{"a": a, "b": b, "slot": s} for a, b, s in pairs]

CROSSINGS = _cross([
    (3, 9, 9), (9, 3, 3),
    (3, 2, 2), (2, 3, 2), (9, 2, 2), (2, 9, 2),
    (3, 7, 7), (7, 3, 7), (9, 7, 7), (7, 9, 7),
    (2, 7, 7), (7, 2, 7), (2, 2, 2), (7, 7, 7),
])

doc("ground-tartan.json", {
    "motifs": motifs("bud", "berry", bud=0.8, berry=0.75),
    "scatter": {
        "density": 0.11, "prevent_overlap": True, "mirror": "none",
        "on_axis_share": 0.0, "rotation_jitter_deg": 0.0,
        "scale_range": [1.0, 1.0],
        "mix": mix(("bud", 2.0), ("berry", 1.0)), "seed": 9,
    },
    "pattern": {
        "tile_size_mm": TILE, "lattice": BLOCK, "placements": [],
        "palette": palette(DAY), "seed": 9,
        "ground": {
            "kind": "bands",
            "vertical": copy.deepcopy(SETT),
            "horizontal": copy.deepcopy(SETT),
            "crossings": copy.deepcopy(CROSSINGS),
        },
    },
})

doc("ground-texture.json", {
    "motifs": motifs("seedhead"),
    "scatter": {
        "density": 0.10, "prevent_overlap": True, "mirror": "none",
        "on_axis_share": 0.0, "rotation_jitter_deg": 0.0,
        "scale_range": [1.0, 1.0], "mix": mix(("seedhead", 1.0)), "seed": 31,
    },
    "pattern": {
        "tile_size_mm": TILE, "lattice": BLOCK, "placements": [],
        "palette": palette(DAY), "seed": 31,
        "ground": {"kind": "texture", "growth": "pebbles", "cells": 150,
                   "steps": 2600, "seed": 5, "levels": [0, 6, 2]},
    },
})

doc("ground-ornament.json", {
    "motifs": motifs("berry", "bud"),
    "scatter": {
        "density": 0.17, "prevent_overlap": True, "mirror": "none",
        "on_axis_share": 0.0, "rotation_jitter_deg": 14.0,
        "scale_range": [0.9, 1.1],
        "mix": mix(("berry", 1.6), ("bud", 1.0)), "seed": 17,
    },
    "pattern": {
        "tile_size_mm": TILE, "lattice": BLOCK, "placements": [],
        "palette": palette(DAY), "seed": 17,
        "ground": {"kind": "ornament", "figure": "scale", "cell_mm": 56.0,
                   "depth": 1, "stroke_mm": 4.0, "variation": 0.0,
                   "seed": 3, "levels": [8]},
    },
})

# --------------------------------------------------- 10. the build example
doc("ditsy.json", {
    "motifs": motifs("sprig", "bud", "seedhead",
                     sprig=0.42, bud=0.6, seedhead=0.45),
    "scatter": {
        "density": 0.30, "prevent_overlap": True, "mirror": "none",
        "on_axis_share": 0.0, "rotation_jitter_deg": 24.0,
        "scale_range": [0.85, 1.15],
        "mix": mix(("sprig", 1.4), ("bud", 2.4), ("seedhead", 1.0)),
        "seed": 1116,
    },
    "pattern": {
        "tile_size_mm": [200.0, 150.0],
        "lattice": {"u": [200.0, 75.0], "v": [0.0, 150.0]},
        "placements": [], "palette": palette(DAY), "seed": 1116,
    },
})

# ------------------------------------------------------- 11. mirrored damask
doc("damask.json", {
    "motifs": motifs("bloom", "leaf", "sprig", "berry", "bud"),
    "scatter": {
        "density": 0.70, "prevent_overlap": True, "mirror": "side-to-side",
        "on_axis_share": 0.35, "rotation_jitter_deg": 0.0,
        "scale_range": [0.85, 1.05],
        "mix": mix(("bloom", 2.0), ("leaf", 1.4), ("sprig", 1.4),
                   ("berry", 1.6), ("bud", 2.0)),
        "seed": 23,
    },
    "pattern": {
        "tile_size_mm": TILE, "lattice": BLOCK, "placements": [],
        "palette": palette(DAY), "seed": 23,
    },
})

print("written")
