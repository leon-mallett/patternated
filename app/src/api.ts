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
  colourways: string[];
  active_colourway: string | null;
  scattered: boolean;
  layout: string;
  prevent_overlap: boolean;
  grid_rows: number;
  grid_cols: number;
}

/** The classic repeats, as a designer names them. */
export const LAYOUTS = [
  { id: "tossed", label: "Tossed (scattered)" },
  { id: "square", label: "Square grid" },
  { id: "brick", label: "Brick" },
  { id: "half-drop", label: "Half-drop" },
  { id: "diagonal", label: "Diagonal" },
  { id: "gentle-diagonal", label: "Gentle diagonal" },
] as const;

export interface MotifEntry {
  id: string;
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
  /** Tiles actually shown, which may exceed what was asked for. */
  cols: number;
  rows: number;
  /** Whether the window shown is itself a repeating unit, so that holding its
   *  opposite edges together is a fair test of the pattern. */
  window_repeats: boolean;
}

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
