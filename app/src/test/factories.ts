/**
 * Builders for the shapes the engine hands the interface.
 *
 * A test should say only what it is about. Spelling out twenty fields to
 * assert one of them buries the point, and every field written by hand is a
 * field that silently stops matching the engine when the engine changes —
 * these are typed, so that drift is a compile error rather than a test
 * asserting something the app can no longer receive.
 */
import { DEFAULT_ORNAMENT, DEFAULT_TEXTURE } from "../api";
import type { MotifEntry, Opened, Preview } from "../api";

export function makeMotif(over: Partial<MotifEntry> = {}): MotifEntry {
  return {
    id: "cherry",
    ground_share: 0,
    natural_size_mm: [52, 52],
    longest_edge_mm: 52,
    weight: 1,
    ...over,
  };
}

export function makeOpened(over: Partial<Opened> = {}): Opened {
  return {
    path: "/patterns/cherries.json",
    name: "cherries",
    tile_mm: [240, 180],
    lattice: "half-drop",
    seed: 1,
    density: 0.3,
    rotation_jitter_deg: 20,
    scale_range: [0.85, 1.15],
    motifs: [makeMotif()],
    palette: ["#faf6ee", "#c8202d", "#3c7837"],
    palette_colours: 3,
    ground: {
      kind: "bands",
      texture: DEFAULT_TEXTURE,
      ornament: DEFAULT_ORNAMENT,
      bands: [],
      direction: "both",
      crossings: "blend",
    },
    band_slots: [],
    colourways: [],
    active_colourway: null,
    scattered: true,
    layout: "tossed",
    on_axis_share: 0,
    prevent_overlap: true,
    grid_rows: 3,
    grid_cols: 3,
    ...over,
  };
}

/** A one-pixel PNG, so the preview has something to put in an `<img src>`. */
export const TINY_PNG =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

export function makePreview(over: Partial<Preview> = {}): Preview {
  return {
    png_base64: TINY_PNG,
    width: 600,
    height: 450,
    placements: 14,
    colours: 3,
    achieved_density: 0.28,
    dpi: 64,
    size_limited_to: 1,
    grid_cells: [3, 3],
    cols: 4,
    rows: 3,
    window_repeats: true,
    ...over,
  };
}
