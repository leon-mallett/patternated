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
  scattered: boolean;
}

export interface MotifEntry {
  id: string;
  natural_size_mm: [number, number];
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
  weights: [string, number][];
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
}

export const openDocument = (path: string) =>
  invoke<Opened>("open_document", { path });

export const apply = (params: Params) => invoke<Preview>("apply", { params });

export const exportPng = (
  path: string,
  dpi: number,
  cols: number,
  rows: number,
) => invoke<string>("export_png", { path, dpi, cols, rows });

export const saveDocument = (path?: string) =>
  invoke<string>("save_document", { path: path ?? null });
