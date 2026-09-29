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
  /** The ground as the sequence editor needs it, so reopening a document
   *  shows the bands it has rather than an empty editor. */
  ground: GroundSummary;
  /** The garment this repeat was cut for, if it has ever been told. */
  drawn_for: string | null;
  /** Anything worth telling a person about artwork that still came in.
   *
   *  These used to go to stderr from inside the engine, where this app could
   *  not see them — so nobody learned that most of a motif had dissolved into
   *  the fabric colour, or that a shadow had been kept as ink. */
  notes: Note[];
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
  /** How strongly the motifs' near neighbours line up — a ranking, not a
   *  verdict. High is worth looking at; low is not a guarantee. */
  alignment: number;
  alignment_warns_above: number;
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

/** The named regions of the reaction's parameter space.
 *
 *  Not styles layered on top of one model: each is a pair of rates, and the
 *  difference between a spot and a maze is two thousandths. The names were
 *  chosen from what the renders actually look like rather than from the
 *  literature, whose rates are quoted for a different Laplacian and are dead
 *  in this one. */
export const GROWTHS = [
  { id: "spots", label: "Spots" },
  { id: "pebbles", label: "Pebbles" },
  { id: "rings", label: "Rings" },
  { id: "worms", label: "Worms" },
  { id: "maze", label: "Maze" },
  { id: "coral", label: "Coral" },
] as const;

/** A grown ground: two chemicals spreading across the tile and consuming each
 *  other, quantised onto palette slots.
 *
 *  The one generator here with no lattice in it. A tossed layout still repeats
 *  on one and the eye finds it; this has only the tile's own period. */
export interface TextureSpec {
  growth: string;
  /** Cells across the tile — the pattern's *scale*, not its quality. Doubling
   *  it does not resolve the same picture more finely, it grows twice as many
   *  spots. */
  cells: number;
  /** How long to let it run. The field settles by about two thousand steps;
   *  stopping short gives a rougher, more broken version of the same look. */
  steps: number;
  seed: number;
  /** A colour per level, sparsest first. `null` is the fabric showing
   *  through, which is how the same growth gives light-on-dark or
   *  dark-on-light without needing two of everything. */
  levels: Array<string | null>;
}

/** What fills one cell of an ornament.
 *
 *  The figures divide in two: the first three are drawn once at the cell's own
 *  size, the last three contain smaller copies of themselves and answer to
 *  `depth`. An interface should not offer depth for the first three, because
 *  it does nothing there. */
export const FIGURES = [
  { id: "truchet", label: "Truchet", recurses: false, stroked: true },
  { id: "weave", label: "Weave", recurses: false, stroked: true },
  { id: "scale", label: "Scale", recurses: false, stroked: false },
  { id: "carpet", label: "Carpet", recurses: true, stroked: false },
  { id: "dust", label: "Dust", recurses: true, stroked: false },
  { id: "cross", label: "Cross", recurses: true, stroked: false },
] as const;

export const figureIs = (id: string) =>
  FIGURES.find((f) => f.id === id) ?? FIGURES[0];

/** A ground drawn by rule on a grid of cells.
 *
 *  The opposite pole from a texture. That one grows and has no lattice in it
 *  anywhere; this is all lattice — a cell size, a figure, and a rule for what
 *  a figure contains. */
export interface OrnamentSpec {
  figure: string;
  /** How big a cell wants to be. The grid is fitted to the tile, so what gets
   *  drawn is the nearest whole number of cells to this — see `grid` on the
   *  reply for what was actually used. */
  cell_mm: number;
  /** How many times a recursive figure divides. One is the figure itself. */
  depth: number;
  /** Line width, for the figures that are stroked. */
  stroke_mm: number;
  /** How often a cell departs from the strict figure, 0 to 1. At zero the
   *  figure is the classical one and the seed does nothing. */
  variation: number;
  seed: number;
  /** A colour per level, coarsest first. `null` is the fabric showing. */
  levels: Array<string | null>;
}

/** The grid an ornament was actually fitted to. */
export interface FittedGrid {
  cols: number;
  rows: number;
  cell_w_mm: number;
  cell_h_mm: number;
}

/** Something the engine wants a person to know about their artwork. */
export interface Note {
  /** The motif or file it concerns. */
  about: string;
  says: string;
}

/** One band in the ground's sequence. */
export interface GroundBand {
  width_mm: number;
  /** `null` is a gap — the fabric showing. Expressed as the absence of a
   *  colour rather than a colour that happens to match, so recolouring the
   *  fabric takes the gaps with it. */
  colour: string | null;
}

/** A ground as it comes back.
 *
 *  No `mirrored`: reflecting is applied once and the reflected sequence is
 *  what gets stored, so there is no hidden flag for a reopened document to
 *  guess at — and guessing it wrong would double the reflection next edit. */
export interface GroundSummary {
  /** Which editor this ground belongs to. */
  kind: "bands" | "texture" | "ornament";
  texture: TextureSpec;
  ornament: OrnamentSpec;
  bands: GroundBand[];
  direction: "vertical" | "horizontal" | "both";
  /** What to paint where two bands cross.
   *
   *  `blend` is the woven answer and the default — half the threads at a
   *  crossing are each colour, and a thread crossing itself stays itself.
   *  `overprint` multiplies, which is what a second pass of ink does.
   *  `across` adds nothing: the across-band wins, and it is the fewest
   *  colours to print. */
  crossings: "blend" | "overprint" | "across";
}

/** A whole ground, as the sequence editor sends it. */
export interface GroundSpec extends GroundSummary {
  /** Reflect the sequence about its end bands, which is how a tartan sett is
   *  written: `A B C D` becomes `A B C D C B`, neither pivot doubled.
   *
   *  A one-shot instruction, not a stored mode: the reflected sequence comes
   *  back as the sequence, and the editor shows the rows it produced. */
  mirrored: boolean;
}

/** The ground after a change. */
export interface BandState extends GroundSummary {
  style: string;
  width_mm: number;
  gap_mm: number;
  /** The slots the bands paint with, so their swatches can sit beside the
   *  control rather than lost among the motif colours. */
  slots: number[];
  /** Colours this ground puts on the fabric, crossings included. */
  ground_colours: number;
  palette: string[];
  lattice: string;
  /** The share of the tile each texture level actually covers.
   *
   *  Worth showing: the thresholds are evenly spaced by concentration and a
   *  field may not reach between two of them, so a colour can be paid for and
   *  not printed. */
  level_shares: number[];
  /** The grid an ornament was fitted to, when the ground is one. */
  grid: FittedGrid | null;
}

/** Put a striped or checked ground under the pattern, or take it away.
 *
 *  Its own command because it rewrites the tail of the palette, which `apply`
 *  never does. Colours go in and slots are allocated behind it, so the editor
 *  never has to know a palette slot exists. */
export const setGround = (spec: GroundSpec) =>
  invoke<BandState>("set_ground", { spec });

/** Starting points for the sequence editor, not modes.
 *
 *  Choosing one fills the sequence in; everything after that is editing rows,
 *  which is why a tartan needs no preset of its own. */
export const DEFAULT_TEXTURE: TextureSpec = {
  growth: "spots",
  cells: 192,
  steps: 2500,
  seed: 1,
  levels: [null, "#1f3d3a"],
};

export const DEFAULT_ORNAMENT: OrnamentSpec = {
  figure: "truchet",
  cell_mm: 24,
  depth: 3,
  stroke_mm: 4,
  variation: 0,
  seed: 1,
  levels: ["#1f3d3a"],
};

export const GROUND_PRESETS: Array<{
  id: string;
  label: string;
  make: (ink: string) => GroundSpec;
}> = [
  {
    id: "none",
    label: "Plain",
    make: () => ({
      bands: [],
      kind: "bands",
      texture: DEFAULT_TEXTURE,
      ornament: DEFAULT_ORNAMENT,
      mirrored: false,
      direction: "both",
      crossings: "blend",
    }),
  },
  {
    id: "stripe",
    label: "Stripe",
    make: (ink) => ({
      bands: [
        { width_mm: 18, colour: ink },
        { width_mm: 18, colour: null },
      ],
      kind: "bands",
      texture: DEFAULT_TEXTURE,
      ornament: DEFAULT_ORNAMENT,
      mirrored: false,
      direction: "vertical",
      crossings: "blend",
    }),
  },
  {
    id: "pinstripe",
    label: "Pinstripe",
    make: (ink) => ({
      bands: [
        { width_mm: 2, colour: ink },
        { width_mm: 22, colour: null },
      ],
      kind: "bands",
      texture: DEFAULT_TEXTURE,
      ornament: DEFAULT_ORNAMENT,
      mirrored: false,
      direction: "vertical",
      crossings: "blend",
    }),
  },
  {
    id: "check",
    label: "Check",
    // Two *threads*, not a thread and a gap. A gap has nothing to blend with,
    // so an ink and a gap give two tones; an ink and a pale thread give three,
    // which is what makes a gingham a gingham.
    make: (ink) => ({
      bands: [
        { width_mm: 16, colour: ink },
        { width_mm: 16, colour: "#f2f0ea" },
      ],
      kind: "bands",
      texture: DEFAULT_TEXTURE,
      ornament: DEFAULT_ORNAMENT,
      mirrored: false,
      direction: "both",
      crossings: "blend",
    }),
  },
  {
    id: "windowpane",
    label: "Windowpane",
    make: (ink) => ({
      bands: [
        { width_mm: 3, colour: ink },
        { width_mm: 40, colour: null },
      ],
      kind: "bands",
      texture: DEFAULT_TEXTURE,
      ornament: DEFAULT_ORNAMENT,
      mirrored: false,
      direction: "both",
      crossings: "blend",
    }),
  },
  {
    id: "tartan",
    label: "Tartan",
    make: () => ({
      bands: [
        { width_mm: 24, colour: "#1b2a4a" },
        { width_mm: 4, colour: "#0b0b0b" },
        { width_mm: 24, colour: "#1f4020" },
        { width_mm: 3, colour: "#c8a020" },
      ],
      kind: "bands",
      texture: DEFAULT_TEXTURE,
      ornament: DEFAULT_ORNAMENT,
      mirrored: true,
      direction: "both",
      crossings: "blend",
    }),
  },
];

/// A sett reflected about its end bands, as `Bands::reflected` does it:
/// `A B C D` becomes `A B C D C B`, with neither pivot doubled.
function reflected(bands: GroundBand[]): GroundBand[] {
  if (bands.length < 3) return bands;
  return [...bands, ...bands.slice(1, -1).reverse()];
}

/** Which preset a ground's sequence is, if it is one of them.
 *
 *  Worked out by comparing the sequence rather than by remembering what was
 *  last clicked, which is the same choice made for detecting unsaved work and
 *  for the same reasons: a flag has to be cleared by every edit that
 *  invalidates it and is wrong the moment one forgets, and it knows nothing
 *  about a document that was opened rather than built.
 *
 *  Colours are deliberately not compared. A preset describes an arrangement —
 *  a thread and a gap, four threads reflected — and recolouring a stripe
 *  leaves it a stripe. Whether a band is an ink or a gap *is* compared,
 *  because that is arrangement: a gingham is two threads where a stripe is a
 *  thread and a gap.
 *
 *  Crossings are compared only where both directions paint, since a one-way
 *  ground has no crossings to have a rule about. */
export function presetOf(ground: {
  bands: GroundBand[];
  direction: string;
  crossings: string;
}): string {
  if (ground.bands.length === 0) return "none";

  for (const preset of GROUND_PRESETS) {
    const want = preset.make("#3c78a0");
    if (want.bands.length === 0) continue;
    const sequence = want.mirrored ? reflected(want.bands) : want.bands;
    if (
      sequence.length !== ground.bands.length ||
      want.direction !== ground.direction ||
      (ground.direction === "both" && want.crossings !== ground.crossings)
    ) {
      continue;
    }
    const alike = sequence.every(
      (band, i) =>
        Math.abs(band.width_mm - ground.bands[i].width_mm) < 1e-4 &&
        (band.colour === null) === (ground.bands[i].colour === null),
    );
    if (alike) return preset.id;
  }
  return "";
}

/** What cutting a design for a garment did. */
export interface Cut {
  /** What every length was multiplied by. One means the document had never
   *  been told which garment it was for, so there was no factor to apply. */
  factor: number;
  tile_mm: [number, number];
  motifs: MotifEntry[];
  drawn_for: string;
}

/** Cut this design for a garment, scaling the repeat and the motifs together.
 *
 *  A 240 mm repeat reads completely differently on an adult shirt and on a
 *  three-year-old's, so the size chart sets a factor on both the tile and the
 *  motifs within it — scaling the tile alone leaves one enormous motif on a
 *  small garment. */
export const cutFor = (garment: string) => invoke<Cut>("cut_for", { garment });

export const openDocument = (path: string) =>
  invoke<Opened>("open_document", { path });

/** Start a pattern from image files, with no document in existence. */
/** Start a pattern with no artwork at all.
 *
 *  A tartan or a procedural ground has no motifs in it. The engine never
 *  required any; the only way *in* did. */
export const newPlain = () => invoke<Opened>("new_plain");

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
