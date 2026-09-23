import { invoke } from "@tauri-apps/api/core";

/** What a freshly opened document looks like to the UI. */
export interface Opened {
  path: string;
  name: string;
  tile_mm: [number, number];
  lattice: string;
  seed: number;
  density: number;
  rotation_jitter_deg: number;
  scale_range: [number, number];
  motifs: MotifEntry[];
  palette: string[];
  /** Colours the design is quantised onto, ground included. */
  palette_colours: number;
  band_style: string;
  band_width_mm: number;
  band_gap_mm: number;
  band_slots: number[];
  colourways: string[];
  active_colourway: string | null;
  scattered: boolean;
  layout: string;
  /** Share of motifs mounted on a mirror line. Zero without a mirror. */
  on_axis_share: number;
  prevent_overlap: boolean;
  grid_rows: number;
  grid_cols: number;
}

/** The classic repeats, as a designer names them.
 *
 *  Grouped by what they do to the motifs rather than by how the engine builds
 *  them: the strewn arrangements scatter motifs and the ruled ones put them on
 *  cells, and that is the choice being made. A mirror belongs with the strewn
 *  ones — it is a scatter of a smaller area, reflected — which is why the
 *  coverage control stays and rows and columns do not appear. */
export const LAYOUTS = [
  { id: "tossed", label: "Tossed (scattered)", group: "Strewn" },
  { id: "mirror", label: "Mirror, side to side", group: "Strewn" },
  { id: "mirror-vertical", label: "Mirror, top to bottom", group: "Strewn" },
  { id: "mirror-four", label: "Mirror, four-way", group: "Strewn" },
  { id: "square", label: "Square grid", group: "Ruled" },
  { id: "brick", label: "Brick", group: "Ruled" },
  { id: "half-drop", label: "Half-drop", group: "Ruled" },
  { id: "diagonal", label: "Diagonal", group: "Ruled" },
  { id: "gentle-diagonal", label: "Gentle diagonal", group: "Ruled" },
] as const;

/** The order the groups appear in the arrangement menu. */
export const LAYOUT_GROUPS = ["Strewn", "Ruled"] as const;

/** Whether a layout strews motifs rather than putting them on a grid.
 *
 *  Mirrors are scatters, so the controls that belong to a scatter — coverage,
 *  size variation — apply to them and rows and columns do not. Asking the
 *  question in one place keeps the panels from disagreeing about it. */
export const isScatterLayout = (layout: string) =>
  layout === "tossed" || layout.startsWith("mirror");

/** Whether a layout reflects the tile, and so designs only part of it. */
export const isMirrorLayout = (layout: string) => layout.startsWith("mirror");

export interface MotifEntry {
  id: string;
  /** Share of this motif's ink that resolves to the ground colour, 0..1.
   *  Invisible on a plain preview and a hole the moment the ground changes. */
  ground_share: number;
  natural_size_mm: [number, number];
  /** Longest edge in mm — the handle a designer wants. Aspect comes from the
   *  artwork and is never adjusted. */
  longest_edge_mm: number;
  weight: number;
}

/** Everything the engine needs to produce a preview. */
export interface Params {
  tile_mm: [number, number];
  lattice: string;
  density: number;
  rotation_jitter_deg: number;
  scale_range: [number, number];
  seed: number;
  layout: string;
  prevent_overlap: boolean;
  on_axis_share: number;
  grid_rows: number;
  grid_cols: number;
  weights: [string, number][];
  sizes: [string, number][];
  preview_width_px: number;
  cols: number;
  rows: number;
}

/**
 * A rendered preview and the numbers beside it.
 *
 * `colours` and `achieved_density` are not decoration: colour count is what a
 * screen printer charges by, and achieved density is what the density control
 * actually delivered rather than what was asked for.
 */
export interface Preview {
  png_base64: string;
  width: number;
  height: number;
  placements: number;
  colours: number;
  achieved_density: number;
  dpi: number;
  /** Below 1 when a grid shrank its motifs to stop them intersecting. */
  size_limited_to: number;
  /** Rows and columns a grid actually used, which may exceed what was asked
   *  for when the offsets would not otherwise close over the tile. */
  grid_cells: [number, number];
  /** Tiles actually shown, which may exceed what was asked for. */
  cols: number;
  rows: number;
  /** Whether the window shown is itself a repeating unit, so that holding its
   *  opposite edges together is a fair test of the pattern. */
  window_repeats: boolean;
}

/** What a request to close did, or why it did not. */
export interface CloseOutcome {
  /** False when the close was refused because work would be lost. */
  closed: boolean;
  /** The pattern has never been written to a file, so closing discards it. */
  never_saved: boolean;
  /** The pattern has changed since it was last written. */
  changed: boolean;
}

/** Put the app back to its empty state.
 *
 *  Refuses rather than asking when there is unsaved work, and carries the two
 *  facts the question needs, so the asking stays in the interface where a
 *  person can see it. Call again with `discard` to go through with it. */
export const closeDocument = (discard: boolean) =>
  invoke<CloseOutcome>("close_document", { discard });

/** The striped and checked grounds, as a designer names them.
 *
 *  Two numbers cover all of them — a band width and a gap — so a pinstripe is
 *  a stripe with a thin band rather than a style of its own. What differs is
 *  which directions are painted and what a crossing resolves to. */
export const BAND_STYLES = [
  { id: "none", label: "Plain" },
  { id: "stripe", label: "Stripe" },
  { id: "band", label: "Bands across" },
  { id: "check", label: "Check" },
  { id: "windowpane", label: "Windowpane" },
] as const;

/** The ground after a change. */
export interface BandState {
  style: string;
  width_mm: number;
  gap_mm: number;
  /** The slots the bands paint with, so their swatches can sit beside the
   *  control rather than lost among the motif colours. */
  slots: number[];
  palette: string[];
  lattice: string;
}

/** Put a striped or checked ground under the pattern, or take it away.
 *
 *  Its own command because it can add colours to the palette, which `apply`
 *  never does. */
export const setBands = (style: string, widthMm: number, gapMm: number) =>
  invoke<BandState>("set_bands", { style, widthMm, gapMm });

export const openDocument = (path: string) =>
  invoke<Opened>("open_document", { path });

/** Start a pattern from image files, with no document in existence. */
export const newFromImages = (files: string[], longestEdgeMm: number) =>
  invoke<Opened>("new_from_images", { files, longestEdgeMm });

export interface SaveTarget {
  needs_path: boolean;
  suggested: string;
}

export const saveTarget = () => invoke<SaveTarget>("save_target");

/** The palette as it stands, after any change. */
export interface PaletteState {
  palette: string[];
  colourways: string[];
  active_colourway: string | null;
}

/** The colour counts the palette control offers.
 *
 *  A slider over these rather than over 2..64 directly: the useful range is
 *  bunched at the bottom, and a linear control would spend three quarters of
 *  its travel between counts nobody chooses. Monotone at one end,
 *  photographic at the other. */
export const COLOUR_COUNTS = [
  2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 64,
] as const;

/** What changing the colour count did. */
export interface PaletteSized {
  palette: string[];
  palette_colours: number;
  colourways: string[];
  active_colourway: string | null;
  /** Named, not counted: a designer should know which work was discarded. */
  colourways_dropped: string[];
  motifs: MotifEntry[];
}

/** Re-derive the design palette at a different number of colours.
 *
 *  Re-runs ingest — every motif is quantised again — so it is its own command
 *  rather than another parameter. */
export const setPaletteSize = (colours: number) =>
  invoke<PaletteSized>("set_palette_size", { colours });

export const setSlotColour = (slot: number, colour: string) =>
  invoke<PaletteState>("set_slot_colour", { slot, colour });

export const deriveColourway = (kind: string, amount: number) =>
  invoke<PaletteState>("derive_colourway", { kind, amount });

export const rememberColourway = (name: string) =>
  invoke<PaletteState>("remember_colourway", { name });

export const useColourway = (name: string | null) =>
  invoke<PaletteState>("use_colourway", { name });

export const apply = (params: Params) => invoke<Preview>("apply", { params });

/** What an export will be, before it is written. */
export interface ExportPlan {
  width_mm: number;
  height_mm: number;
  width_px: number;
  height_px: number;
  cols: number;
  rows: number;
  width_verdict: "fits" | "wide" | "too-wide";
  megabytes: number;
  suggested_name: string;
}

export interface ExportRequest {
  format: "png" | "tiff";
  dpi: number;
  shape: "tile" | "sheet";
  min_cols: number;
  min_rows: number;
  compress: boolean;
  roll_width_mm: number;
  /** A CMYK ICC profile from the print bureau. Absent means write RGB. */
  cmyk_profile: string | null;
  rgb_profile: string | null;
}

export const exportPlan = (request: ExportRequest) =>
  invoke<ExportPlan>("export_plan", { request });

export const exportFile = (path: string, request: ExportRequest) =>
  invoke<string>("export", { path, request });

/** The preview, in the colours a press can actually reach. */
export const proof = (params: Params, cmykProfile: string) =>
  invoke<Preview>("proof", { params, cmykProfile });

export const saveDocument = (path?: string) =>
  invoke<string>("save_document", { path: path ?? null });

/** A garment a print can be seen on, by the measurements a size chart lists. */
export interface GarmentOption {
  name: string;
  chest_mm: number;
  length_mm: number;
}

export const garments = () => invoke<GarmentOption[]>("garments");

/** A print on a garment, at true scale. */
export interface MockupView {
  png_base64: string;
  width: number;
  height: number;
  repeats_across: number;
  chest_mm: number;
  length_mm: number;
}

export const mockup = (params: Params, garment: string, widthPx: number) =>
  invoke<MockupView>("mockup", { params, garment, widthPx });
