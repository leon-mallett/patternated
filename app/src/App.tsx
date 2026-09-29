import { useCallback, useEffect, useRef, useState } from "react";
import {
  confirm as confirmDialog,
  open as openDialog,
  save as saveDialog,
} from "@tauri-apps/plugin-dialog";
import {
  apply,
  deriveColourway,
  exportFile,
  exportPlan,
  garments,
  mockup,
  newFromImages,
  newPlain,
  cutFor,
  openDocument,
  proof,
  rememberColourway,
  saveDocument,
  saveTarget,
  setSlotColour,
  useColourway,
  type MotifEntry,
  type Note,
  type Opened,
  type Params,
  type Preview,
  type ExportPlan,
  type ExportRequest,
  type PaletteState,
  type GarmentOption,
  type MockupView,
  LAYOUTS,
  COLOUR_COUNTS,
  GROUND_PRESETS,
  presetOf,
  GROWTHS,
  FIGURES,
  figureIs,
  DEFAULT_TEXTURE,
  DEFAULT_ORNAMENT,
  setGround,
  type GroundBand,
  type GroundSpec,
  type TextureSpec,
  type OrnamentSpec,
  type FittedGrid,
  closeDocument,
  setPaletteSize,
  LAYOUT_GROUPS,
  isScatterLayout,
  isMirrorLayout,
} from "./api";

/**
 * Tiles shown in the preview.
 *
 * Three, and not adjustable. A single tile says nothing about whether a pattern
 * tracks, and three by three is the smallest field where the eye finds
 * corridors — so it is the default the tool should make hard to avoid.
 */
const FIELD = 3;

/** Longest a control may be dragged before a re-render is asked for. */
const SETTLE_MS = 90;

export default function App() {
  const [doc, setDoc] = useState<Opened | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const [tile, setTile] = useState<[number, number]>([240, 180]);
  const [lattice, setLattice] = useState("half-drop");
  const [density, setDensity] = useState(0.3);
  const [onAxisShare, setOnAxisShare] = useState(0);
  const [colourIndex, setColourIndex] = useState(
    COLOUR_COUNTS.indexOf(12 as never),
  );
  const [motifs, setMotifs] = useState<MotifEntry[] | null>(null);
  const [dropped, setDropped] = useState<string[]>([]);
  const [ground, setGroundState] = useState<GroundSpec>({
    kind: "bands",
    texture: DEFAULT_TEXTURE,
    ornament: DEFAULT_ORNAMENT,
    bands: [],
    mirrored: false,
    direction: "both",
    crossings: "blend",
  });
  const [levelShares, setLevelShares] = useState<number[]>([]);
  const [fittedGrid, setFittedGrid] = useState<FittedGrid | null>(null);
  const [bandSlots, setBandSlots] = useState<number[]>([]);
  const [groundColours, setGroundColours] = useState(0);
  /// Whether the ground paints anything. Read from the slots it was given
  /// rather than from which editor is showing, because an editor can be open
  /// over a ground that is set to plain.
  const hasGround = bandSlots.length > 0;
  /// Whether there is artwork to arrange at all. A pattern can be made
  /// entirely of a ground, and then everything about placing motifs, reducing
  /// their colours and editing those colours has nothing to act on.
  const hasArtwork = doc !== null && doc.motifs.length > 0;
  const [jitter, setJitter] = useState(20);
  const [scale, setScale] = useState<[number, number]>([0.85, 1.15]);
  const [seed, setSeed] = useState(1);
  const [weights, setWeights] = useState<Record<string, number>>({});
  const [layout, setLayout] = useState("tossed");
  const [preventOverlap, setPreventOverlap] = useState(true);
  const [gridRows, setGridRows] = useState(3);
  const [gridCols, setGridCols] = useState(3);
  const [palette, setPalette] = useState<PaletteState | null>(null);

  const [wearing, setWearing] = useState<string | null>(null);
  const [drawnFor, setDrawnFor] = useState<string | null>(null);
  const [notes, setNotes] = useState<Note[]>([]);
  const [garmentList, setGarmentList] = useState<GarmentOption[]>([]);
  const [worn, setWorn] = useState<MockupView | null>(null);

  useEffect(() => {
    garments().then(setGarmentList).catch(() => setGarmentList([]));
  }, []);

  const [exporting, setExporting] = useState(false);
  const [plan, setPlan] = useState<ExportPlan | null>(null);
  const [exp, setExp] = useState<ExportRequest>({
    format: "tiff",
    dpi: 150,
    shape: "tile",
    min_cols: 3,
    min_rows: 3,
    compress: false,
    roll_width_mm: 1600,
    cmyk_profile: null,
    rgb_profile: null,
  });
  const [proofing, setProofing] = useState(false);

  // Keep the plan in step with the settings, so the size and the verdict are
  // in front of a designer while they still have a choice.
  useEffect(() => {
    if (!doc || !exporting) return;
    let live = true;
    exportPlan(exp)
      .then((p) => live && setPlan(p))
      .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [doc, exporting, exp, preview]);
  const [sizes, setSizes] = useState<Record<string, number>>({});

  const paneRef = useRef<HTMLDivElement | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const groundTimer = useRef<number | undefined>(undefined);
  // Guards against a slow render landing after a newer one and overwriting it.
  const generation = useRef(0);

  const currentParams = useCallback(
    (): Params => ({
      tile_mm: tile,
      lattice,
      density,
      on_axis_share: onAxisShare,
      rotation_jitter_deg: jitter,
      scale_range: scale,
      seed,
      layout,
      prevent_overlap: preventOverlap,
      grid_rows: gridRows,
      grid_cols: gridCols,
      weights: Object.entries(weights),
      sizes: Object.entries(sizes),
      preview_width_px: Math.max(320, Math.round(paneRef.current?.clientWidth ?? 720)),
      cols: FIELD,
      rows: FIELD,
    }),
    [
      tile,
      lattice,
      density,
      onAxisShare,
      jitter,
      scale,
      seed,
      layout,
      preventOverlap,
      gridRows,
      gridCols,
      weights,
      sizes,
    ],
  );

  const refresh = useCallback(async () => {
    if (!doc) return;
    const mine = ++generation.current;
    setBusy(true);
    const params = currentParams();
    try {
      const next =
        proofing && exp.cmyk_profile
          ? await proof(params, exp.cmyk_profile)
          : await apply(params);
      if (mine === generation.current) {
        setPreview(next);
        setError(null);
      }
    } catch (e) {
      if (mine === generation.current) setError(String(e));
    } finally {
      if (mine === generation.current) setBusy(false);
    }
  }, [doc, currentParams, proofing, exp.cmyk_profile]);

  // The garment view follows the pattern rather than being a snapshot of it,
  // so a scale decision is made against what is actually on screen.
  useEffect(() => {
    if (!doc || !wearing) {
      setWorn(null);
      return;
    }
    let live = true;
    mockup(currentParams(), wearing, 520)
      .then((m) => live && setWorn(m))
      .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [doc, wearing, currentParams, preview]);

  // Coalesce a drag into one render rather than one per pixel.
  useEffect(() => {
    if (!doc) return;
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(refresh, SETTLE_MS);
    return () => window.clearTimeout(timer.current);
  }, [doc, refresh]);

  /** Adopt a freshly opened or created document into the controls. */
  function adopt(opened: Opened, note: string) {
    setDoc(opened);
    setTile(opened.tile_mm);
    setLattice(opened.lattice === "block" ? "block" : "half-drop");
    setDensity(opened.density);
    setOnAxisShare(opened.on_axis_share);
    setMotifs(null);
    setDropped([]);
    setColourIndex(nearestColourIndex(opened.palette_colours));
    setGroundState({ ...opened.ground, mirrored: false });
    setBandSlots(opened.band_slots);
    setGroundColours(opened.band_slots.length);
    setJitter(opened.rotation_jitter_deg);
    setScale(opened.scale_range);
    setSeed(opened.seed);
    setDrawnFor(opened.drawn_for);
    setNotes(opened.notes);
    setWeights(Object.fromEntries(opened.motifs.map((m) => [m.id, m.weight])));
    setSizes(
      Object.fromEntries(opened.motifs.map((m) => [m.id, m.longest_edge_mm])),
    );
    setPalette({
      palette: opened.palette,
      colourways: opened.colourways,
      active_colourway: opened.active_colourway,
    });
    setLayout(opened.layout);
    setPreventOverlap(opened.prevent_overlap);
    setGridRows(opened.grid_rows);
    setGridCols(opened.grid_cols);
    setError(null);
    setStatus(note);
  }

  /// Put the app back to the start screen.
  ///
  /// The engine refuses while there is unsaved work and says which kind, so
  /// the question asked here is the one the situation actually calls for
  /// rather than a blanket "are you sure".
  async function onClose() {
    try {
      const first = await closeDocument(false);
      if (!first.closed) {
        const ok = await confirmDialog(
          first.never_saved
            ? "This pattern has never been saved. Closing it discards it."
            : "There are changes since this pattern was last saved. Closing loses them.",
          { title: "Close this pattern?", kind: "warning", okLabel: "Close" },
        );
        if (!ok) return;
        await closeDocument(true);
      }
      forget();
    } catch (e) {
      setError(String(e));
    }
  }

  /// Drop everything belonging to the open document.
  ///
  /// Not only `doc`: a stale preview or garment would flash behind the next
  /// pattern before its own render arrived.
  function forget() {
    setDoc(null);
    setPreview(null);
    setPalette(null);
    setMotifs(null);
    setDropped([]);
    setWorn(null);
    setWearing(null);
    setExporting(false);
    setPlan(null);
    setProofing(false);
    setError(null);
    setStatus(null);
  }

  /// Start a pattern with no artwork — a tartan, a stripe, a ground.
  async function onNewPlain() {
    try {
      adopt(await newPlain(), "Started a plain ground");
    } catch (e) {
      setError(String(e));
    }
  }

  async function onNew() {
    const chosen = await openDialog({
      title: "Choose the artwork for this pattern",
      multiple: true,
      // A drawing keeps its outlines and is rasterised at the size each
      // render needs; artwork is sampled once. Both arrive the same way.
      filters: [{ name: "Artwork or drawings", extensions: ["png", "svg"] }],
    });
    const files = Array.isArray(chosen) ? chosen : chosen ? [chosen] : [];
    if (files.length === 0) return;
    try {
      // 50 mm is a starting point, not a decision — every motif gets its own
      // size control the moment the pattern is on screen.
      const opened = await newFromImages(files, 50);
      adopt(opened, `Started a pattern from ${files.length} image${files.length > 1 ? "s" : ""}`);
    } catch (e) {
      setError(String(e));
    }
  }

  /// Apply a palette change and re-render, since colour is resolved at draw
  /// time rather than stored in the motifs.
  async function changePalette(action: () => Promise<PaletteState>) {
    try {
      setPalette(await action());
      setError(null);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  /// Re-derive the palette at a different colour count.
  ///
  /// Its own path rather than `changePalette`, because this one re-runs
  /// ingest: the motifs are quantised again and come back with new ground
  /// shares, and any colourway describing the old slot count is gone.
  async function changeColourCount(index: number) {
    setColourIndex(index);
    try {
      const sized = await setPaletteSize(COLOUR_COUNTS[index]);
      setPalette({
        palette: sized.palette,
        colourways: sized.colourways,
        active_colourway: sized.active_colourway,
      });
      setMotifs(sized.motifs);
      setDropped(sized.colourways_dropped);
      setError(null);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  /// Put a striped or checked ground under the pattern.
  ///
  /// Its own path rather than `changePalette`, because a ground rewrites the
  /// tail of the palette and can move the repeat back onto a block lattice.
  /// Colours go down and slots come back: the editor never handles a slot.
  function changeGround(spec: GroundSpec) {
    setGroundState(spec);
    window.clearTimeout(groundTimer.current);

    // A texture is shown at once and sent when the hand stops. Growing one is
    // a few hundred milliseconds where everything else in a render is a few,
    // so a drag across the scale slider would otherwise queue a simulation per
    // step and arrive at the last one long after the mouse did.
    //
    // Bands are not coalesced, and that is deliberate rather than an
    // oversight. `mirrored` is a one-shot instruction that the engine answers
    // by returning the reflected sequence; coalescing a reflect with the edit
    // that followed it would cancel the pending send and lose the reflection
    // entirely. Bands are cheap, so there is nothing to win by risking it.
    if (spec.kind === "texture") {
      groundTimer.current = window.setTimeout(
        () => void sendGround(spec),
        SETTLE_MS,
      );
    } else {
      void sendGround(spec);
    }
  }

  async function sendGround(spec: GroundSpec) {
    try {
      const state = await setGround(spec);
      // The engine's answer is the state, not the request: reflecting is
      // applied once and comes back expanded, so the editor shows the rows it
      // actually produced rather than the shorter sequence that asked for
      // them.
      setGroundState({ ...state, mirrored: false });
      setBandSlots(state.slots);
      setGroundColours(state.ground_colours);
      setLevelShares(state.level_shares);
      setFittedGrid(state.grid);
      setPalette((p) => ({
        palette: state.palette,
        colourways: p?.colourways ?? [],
        active_colourway: p?.active_colourway ?? null,
      }));
      setLattice(state.lattice === "block" ? "block" : lattice);
      setError(null);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  /// Replace one row of the sequence.
  function editBand(index: number, over: Partial<GroundBand>) {
    changeGround({
      ...ground,
      bands: ground.bands.map((b, i) => (i === index ? { ...b, ...over } : b)),
    });
  }

  /// Cut the design for a garment: the repeat and the motifs move together.
  async function onCutFor(garment: string) {
    try {
      const cut = await cutFor(garment);
      setDrawnFor(cut.drawn_for);
      setTile(cut.tile_mm);
      setMotifs(cut.motifs);
      setSizes(
        Object.fromEntries(cut.motifs.map((m) => [m.id, m.longest_edge_mm])),
      );
      setError(null);
      setStatus(
        cut.factor === 1
          ? `These measurements are now recorded as ${garment}.`
          : `Cut for ${garment} — everything scaled by ${cut.factor.toFixed(2)}.`,
      );
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function onOpen() {
    const chosen = await openDialog({
      title: "Open a pattern document",
      filters: [{ name: "Pattern document", extensions: ["json"] }],
    });
    if (typeof chosen !== "string") return;
    try {
      const opened = await openDocument(chosen);
      adopt(opened, `Opened ${opened.name}`);
    } catch (e) {
      setError(String(e));
    }
  }

  async function onPickProfile() {
    const chosen = await openDialog({
      title: "Choose the bureau's CMYK profile",
      filters: [{ name: "ICC profile", extensions: ["icc", "icm"] }],
    });
    if (typeof chosen !== "string") return;
    setExp({ ...exp, cmyk_profile: chosen });
  }

  async function onExport() {
    if (!plan) return;
    const target = await saveDialog({
      title: "Export",
      defaultPath: plan.suggested_name,
      filters: [
        exp.format === "tiff"
          ? { name: "TIFF", extensions: ["tif", "tiff"] }
          : { name: "PNG", extensions: ["png"] },
      ],
    });
    if (typeof target !== "string") return;
    try {
      const where = await exportFile(target, exp);
      setExporting(false);
      setStatus(`Exported ${where}`);
    } catch (e) {
      setError(String(e));
    }
  }

  async function onSave() {
    try {
      // A pattern started from images has never been written anywhere, so
      // Save asks rather than inventing a filename beside the artwork.
      const target = await saveTarget();
      let path: string | undefined;
      if (target.needs_path) {
        const chosen = await saveDialog({
          title: "Save this pattern",
          defaultPath: target.suggested,
          filters: [{ name: "Pattern document", extensions: ["json"] }],
        });
        if (typeof chosen !== "string") return;
        path = chosen;
      }
      const where = await saveDocument(path);
      setStatus(`Saved ${where}`);
    } catch (e) {
      setError(String(e));
    }
  }

  if (!doc) {
    return (
      <main className="empty">
        <h1>Pattern Engine</h1>
        <p className="lede">
          Turn illustrations into repeating prints. Everything is measured in
          millimetres — a resolution is chosen only when you export.
        </p>
        <div className="start">
          <button className="primary" onClick={onNew}>
            New pattern from artwork…
          </button>
          <button onClick={onOpen}>Open a saved pattern…</button>
          <button data-testid="new-plain" onClick={onNewPlain}>
            Start from a plain ground…
          </button>
        </div>
        <p className="hint start-hint">
          Start with one or more images, each showing a single motif. A{" "}
          <strong>PNG</strong> can have a transparent background or sit on
          plain paper — the background is measured away either way. An{" "}
          <strong>SVG</strong> keeps its outlines and is drawn afresh at
          whatever size it is printed, so it stays sharp however far it is
          scaled up; convert any text in it to outlines first. A palette is
          worked out from the artwork itself.
        </p>
        <p className="hint start-hint">
          Or start from a <strong>plain ground</strong> and use no artwork at
          all: a stripe, a check and a tartan are the fabric being a different
          colour in bands, and need nothing placed on them.
        </p>
        <p className="hint start-hint">
          A <strong>saved pattern</strong> is a small <code>.json</code> file
          recording the repeat, the motifs it uses and where they sit. There are
          examples under <code>examples/</code> in the project folder.
        </p>
        {error && <p className="error">{error}</p>}

        {/* What the ingest wanted to say. It used to say it to stderr from
            inside the engine, where nothing could show it. Dismissible
            because it is about artwork that *did* come in — a warning that
            cannot be put away is one that gets ignored. */}
        {notes.length > 0 && (
          <div className="notes" data-testid="ingest-notes">
            {notes.map((note, i) => (
              <p className="hint warn" key={i}>
                <strong>{note.about}</strong> — {note.says}
              </p>
            ))}
            <button data-testid="dismiss-notes" onClick={() => setNotes([])}>
              Dismiss
            </button>
          </div>
        )}
      </main>
    );
  }

  // Composed per workflow rather than in one fixed order. A pattern made
  // entirely of a ground is a different job from a pattern of motifs strewn
  // over one, and a panel that opens with the same controls for both asks the
  // designer to skip past the half that cannot apply. So the ground leads when
  // it *is* the design, and sits under the background when it is what the
  // motifs are strewn on.
  const groundPanel = (
          <Group title="Ground">
            {!hasArtwork && (
              <p className="hint" data-testid="ground-is-the-design">
                This pattern has no artwork in it, so the ground is the whole
                design. Stripes, checks, tartans, grown textures and drawn
                ornament are all made here.
              </p>
            )}
            <label>
              Made of
              <select
                data-testid="ground-kind"
                value={ground.kind}
                onChange={(e) =>
                  changeGround({
                    ...ground,
                    kind: e.target.value as GroundSpec["kind"],
                    bands: e.target.value === "bands" ? ground.bands : [],
                  })
                }
              >
                <option value="bands">Bands — stripes, checks, tartans</option>
                <option value="texture">Texture — grown, no lattice in it</option>
                <option value="ornament">Ornament — drawn by rule on a grid</option>
              </select>
            </label>

            {ground.kind === "ornament" ? (
              <OrnamentEditor
                ornament={ground.ornament}
                grid={fittedGrid}
                onChange={(ornament) => changeGround({ ...ground, ornament })}
              />
            ) : ground.kind === "texture" ? (
              <TextureEditor
                texture={ground.texture}
                shares={levelShares}
                onChange={(texture) => changeGround({ ...ground, texture })}
              />
            ) : (
            <>
            <label>
              Pattern
              <select
                data-testid="ground-preset"
                value={presetOf(ground)}
                onChange={(e) => {
                  const preset = GROUND_PRESETS.find((g) => g.id === e.target.value);
                  if (preset) changeGround(preset.make("#3c78a0"));
                }}
              >
                {/* Only offered once the sequence has actually been edited
                    away from every preset. Showing it the moment a preset is
                    chosen reads as though the choice did not take. */}
                {presetOf(ground) === "" && <option value="">Custom</option>}
                {GROUND_PRESETS.map((g) => (
                  <option key={g.id} value={g.id}>
                    {g.label}
                  </option>
                ))}
              </select>
            </label>

            {ground.bands.length > 0 && (
              <>
                <div className="sequence" data-testid="ground-sequence">
                  {ground.bands.map((band, i) => (
                    <div className="sequence-row" key={i}>
                      <label className="swatch-edit">
                        <input
                          type="color"
                          data-testid={`band-colour-${i}`}
                          value={band.colour ?? "#ffffff"}
                          disabled={band.colour === null}
                          onChange={(e) => editBand(i, { colour: e.target.value })}
                        />
                        <span
                          className="swatch"
                          style={{
                            background: band.colour ?? "transparent",
                            backgroundImage: band.colour
                              ? undefined
                              : "repeating-linear-gradient(45deg, #0001 0 4px, #0000 4px 8px)",
                          }}
                        />
                      </label>
                      <input
                        type="number"
                        className="sequence-width"
                        data-testid={`band-width-${i}`}
                        value={band.width_mm}
                        min={0.5}
                        max={200}
                        step={0.5}
                        onChange={(e) =>
                          editBand(i, { width_mm: Number(e.target.value) || 0.5 })
                        }
                      />
                      <button
                        className="ghost"
                        data-testid={`band-gap-${i}`}
                        title={band.colour === null ? "Give it a colour" : "Make it a gap"}
                        onClick={() =>
                          editBand(i, { colour: band.colour === null ? "#3c78a0" : null })
                        }
                      >
                        {band.colour === null ? "gap" : "ink"}
                      </button>
                      <button
                        className="ghost"
                        data-testid={`band-remove-${i}`}
                        title="Remove this band"
                        onClick={() =>
                          changeGround({
                            ...ground,
                            bands: ground.bands.filter((_, j) => j !== i),
                          })
                        }
                      >
                        ×
                      </button>
                    </div>
                  ))}
                </div>

                <button
                  data-testid="band-add"
                  onClick={() =>
                    changeGround({
                      ...ground,
                      bands: [
                        ...ground.bands,
                        { width_mm: 12, colour: "#3c78a0" },
                      ],
                    })
                  }
                >
                  Add a band
                </button>

                <label>
                  Runs
                  <select
                    data-testid="ground-direction"
                    value={ground.direction}
                    onChange={(e) =>
                      changeGround({
                        ...ground,
                        direction: e.target.value as GroundSpec["direction"],
                      })
                    }
                  >
                    <option value="vertical">Down the fabric</option>
                    <option value="horizontal">Across the fabric</option>
                    <option value="both">Both ways</option>
                  </select>
                </label>

                <button
                  data-testid="ground-mirror"
                  disabled={ground.bands.length < 3}
                  onClick={() => changeGround({ ...ground, mirrored: true })}
                >
                  Mirror the sequence
                </button>
                <p className="hint">
                  How a tartan sett is written: the threadcount runs from one
                  pivot to the other and the repeat reflects, without doubling
                  either end band. This writes the reflected bands into the list
                  rather than remembering a mode, so what you see is the repeat.
                </p>

                {ground.direction === "both" && (
                  <>
                    <label>
                      Where bands cross
                      <select
                        data-testid="ground-crossings"
                        value={ground.crossings}
                        onChange={(e) =>
                          changeGround({
                            ...ground,
                            crossings: e.target
                              .value as GroundSpec["crossings"],
                          })
                        }
                      >
                        <option value="blend">Blend the two threads</option>
                        <option value="overprint">Overprint, as ink does</option>
                        <option value="across">Let the across-band win</option>
                      </select>
                    </label>
                    <p className="hint">
                      Blending is what cloth does: half the threads at a crossing
                      are each colour, so two threads give a third tone and a
                      thread crossing itself stays itself. Overprinting multiplies
                      instead, which is what a second pass of ink does — right for
                      a design built from overlapping bands, and much darker.
                      Letting the across-band win adds no colours at all.
                    </p>
                  </>
                )}

                <p className="hint" data-testid="ground-colours">
                  {groundColours} {groundColours === 1 ? "colour" : "colours"} in
                  the ground, crossings included.
                </p>
              </>
            )}
            </>
            )}
          </Group>
  );

  return (
    <main className="app">
      <section className="preview" ref={paneRef}>
        <header>
          <div>
            <strong>{doc.name}</strong>
            {/* "4×3 repeat" read as though the design were a 4×3 repeat,
                rather than as a count of how many are on screen — and then an
                export of one tile looked like a cropped preview. The repeat's
                actual size is in the readout below; this says what is being
                looked at. */}
            <span className="muted" data-testid="tiles-shown">
              {" "}
              · showing{" "}
              {preview ? `${preview.cols}×${preview.rows}` : `${FIELD}×${FIELD}`} tiles
            </span>
          </div>
          <div className="actions">
            {exp.cmyk_profile && (
              <button
                className={proofing ? "primary" : ""}
                onClick={() => setProofing(!proofing)}
                title="Show the colours the press can actually reach"
              >
                {proofing ? "Proofing" : "Soft proof"}
              </button>
            )}
            <select
              className="garment-pick"
              value={wearing ?? ""}
              onChange={(e) => setWearing(e.target.value || null)}
              title="See the print at true scale on a garment"
            >
              <option value="">No garment</option>
              {garmentList.map((g) => (
                <option key={g.name} value={g.name}>
                  {g.name}
                </option>
              ))}
            </select>
            <button data-testid="new" onClick={onNew}>New…</button>
            <button data-testid="open" onClick={onOpen}>Open…</button>
            <button data-testid="save" onClick={onSave}>Save</button>
            <button data-testid="close" onClick={onClose} title="Back to the start screen">
              Close
            </button>
            <button className="primary" data-testid="export" onClick={() => setExporting(true)}>
              Export…
            </button>
          </div>
        </header>

        {wearing && worn && (
          <div className="worn">
            <img
              src={`data:image/png;base64,${worn.png_base64}`}
              alt={`The print on a ${wearing}`}
            />
            <div className="worn-caption">
              <strong>{wearing}</strong>
              <span className="muted">
                {Math.round(worn.across_mm / 10)} × {Math.round(worn.length_mm / 10)} cm
                {" · "}
                {worn.repeats_across.toFixed(1)} repeats across
              </span>
            </div>
          </div>
        )}

        <div className={busy ? "canvas working" : "canvas"}>
          {preview ? (
            <img
              data-testid="preview-image"
              src={`data:image/png;base64,${preview.png_base64}`}
              alt={`${preview.cols} by ${preview.rows} repeat of ${doc.name}`}
            />
          ) : (
            <p className="muted">Rendering…</p>
          )}
        </div>

        {error && <p className="error">{error}</p>}
        {/* What the ingest wanted to say. It used to say it to stderr from
            inside the engine, where nothing could show it. Dismissible
            because it is about artwork that *did* come in — a warning that
            cannot be put away is one that gets ignored. */}
        {notes.length > 0 && (
          <div className="notes" data-testid="ingest-notes">
            {notes.map((note, i) => (
              <p className="hint warn" key={i}>
                <strong>{note.about}</strong> — {note.says}
              </p>
            ))}
            <button data-testid="dismiss-notes" onClick={() => setNotes([])}>
              Dismiss
            </button>
          </div>
        )}

        {preview && !preview.window_repeats && (
          <p className="hint">
            This lattice has no small whole-tile window that repeats, so the
            preview's opposite edges will not meet. The exported tile is still
            seamless.
          </p>
        )}

        {preview && (
          <dl className="readout" data-testid="readout">
            <Readout label="Repeat" value={`${tile[0]} × ${tile[1]} mm`} />
            {doc.motifs.length > 0 && (
              <>
                <Readout label="Motifs" value={String(preview.placements)} />
                <Readout
                  label="Coverage"
                  value={`${Math.round(preview.achieved_density * 100)}% (asked ${Math.round(
                    density * 100,
                  )}%)`}
                />
              </>
            )}
            <Readout
              label="Colours"
              value={String(preview.colours)}
              hint="What a screen printer charges by"
            />
            {preview.alignment > preview.alignment_warns_above && (
              <Readout
                label="Tracking"
                value="worth a look"
                hint="The motifs' near neighbours line up more than most scatters do, which is what makes corridors appear across a repeat. Look at the field before printing, and reroll the seed if you see lanes. A low score is not a guarantee — this catches the obvious cases, not the faint ones."
              />
            )}
            {preview.size_limited_to < 0.999 && (
              <Readout
                label="Motif size"
                value={`limited to ${Math.round(preview.size_limited_to * 100)}%`}
                hint="Shrunk to fit the grid without overlapping. Use fewer rows or columns, or turn the guard off."
              />
            )}
          </dl>
        )}
      </section>

      {/* Over the whole window rather than under the preview. Exporting is a
          decision with a dozen settings in it and an end — a file written —
          and it was tucked below the image, where it pushed the preview out of
          view and read as one more panel of the same kind as the others. */}
      {exporting && (
        <Modal title="Export" onClose={() => setExporting(false)}>
          <ExportPanel
            request={exp}
            plan={plan}
            onChange={setExp}
            onCancel={() => setExporting(false)}
            onExport={onExport}
            onPickProfile={onPickProfile}
          />
        </Modal>
      )}

      <aside className="controls">
        {!hasArtwork && groundPanel}

        {/* Everything in Layout arranges motifs — where they sit, how many to
            a row, whether they may touch. A pattern with no artwork in it has
            none of that to decide, and a panel of controls that do nothing is
            worse than a shorter panel: they get moved, nothing happens, and
            the rest of the interface stops being believed.

            Reported from the app: the options at the top related only to
            motifs on a design that had none. */}
        {hasArtwork && (
        <Group title="Layout">
          <label className="check">
            <input
              type="checkbox"
              data-testid="prevent-overlap"
              checked={preventOverlap}
              onChange={(e) => setPreventOverlap(e.target.checked)}
            />
            <span>Prevent overlapping</span>
          </label>
          <p className="hint">
            {isMirrorLayout(layout)
              ? "Keeps motifs from intersecting — including with their own reflections, which is what stops a motif on a mirror line coming back doubled."
              : layout === "tossed"
                ? "Keeps motifs from intersecting. Asking for more coverage than fits simply stops adding motifs — the coverage readout shows what was reached."
                : "Keeps motifs from intersecting. A grid's cells are fixed, so motifs too large for them are shrunk to fit rather than drawn over each other."}
          </p>
          <label>
            Arrangement
            <select data-testid="layout" value={layout} onChange={(e) => setLayout(e.target.value)}>
              {LAYOUT_GROUPS.map((group) => (
                <optgroup key={group} label={group}>
                  {LAYOUTS.filter((l) => l.group === group).map((l) => (
                    <option key={l.id} value={l.id}>
                      {l.label}
                    </option>
                  ))}
                </optgroup>
              ))}
            </select>
          </label>
          {isMirrorLayout(layout) && (
            <p className="hint">
              Only part of the tile is designed — half for a single mirror, a
              quarter for a four-way — and the rest is its reflection. The
              repeat stays the size you set; a mirror divides it rather than
              enlarging it. Motifs are kept clear of the mirror lines, so a
              large motif in a small repeat leaves less room than it would
              strewn.
            </p>
          )}
          {!isScatterLayout(layout) && (
            <>
              <NumberField
                label="Rows"
            testId="grid-rows"
                unit="per tile"
                value={gridRows}
                min={1}
                max={24}
                step={1}
                onChange={setGridRows}
              />
              <NumberField
                label="Columns"
            testId="grid-cols"
                unit="per tile"
                value={gridCols}
                min={1}
                max={24}
                step={1}
                onChange={setGridCols}
              />
              {preview &&
                (preview.grid_cells[0] !== gridRows ||
                  preview.grid_cells[1] !== gridCols) && (
                  <p className="hint">
                    Drawn as {preview.grid_cells[0]}×{preview.grid_cells[1]}.
                    This arrangement shifts each row or column by a fraction of
                    a cell, and the shift has to come out whole over the tile
                    or the pattern steps at every join. The nearest size that
                    works is used.
                  </p>
                )}
            </>
          )}
        </Group>
        )}

        <Group title="Repeat">
          <label>
            Type
            <select data-testid="repeat-type" value={lattice} onChange={(e) => setLattice(e.target.value)}>
              {/* A ground lives in the tile and repeats with it, so it meets
                  itself only where the lattice steps by whole tiles. Choosing
                  an oblique one here used to be accepted by the control and
                  then refused by the engine, which is a worse way to learn it
                  than not being offered it. */}
              <option value="half-drop" disabled={hasGround}>
                Half-drop
              </option>
              <option value="block">Block</option>
              <option value="brick" disabled={hasGround}>
                Brick
              </option>
              <option value="quarter-drop" disabled={hasGround}>
                Quarter-drop
              </option>
            </select>
          </label>
          {hasGround && (
            <p className="hint" data-testid="ground-needs-block">
              A ground needs a block repeat. An oblique one steps it by part of
              a tile at every join, so bands stop running straight and anything
              drawn or grown meets an edge it was not made against.
            </p>
          )}
          {/* Cutting rather than previewing: this changes the design, where
              the garment picker in the header only shows it on one. A repeat
              is a physical size, so the size it is *for* belongs beside the
              millimetres rather than beside the preview. */}
          <label>
            Cut for
            <select
              data-testid="cut-for"
              value={drawnFor ?? ""}
              onChange={(e) => e.target.value && onCutFor(e.target.value)}
            >
              {drawnFor === null && <option value="">Not set</option>}
              {garmentList.map((g) => (
                <option key={g.name} value={g.name}>
                  {g.name}
                </option>
              ))}
            </select>
          </label>
          <p className="hint" data-testid="cut-for-hint">
            {drawnFor === null
              ? "A repeat reads completely differently on an adult shirt and on a three-year-old's. Choosing a size here records what these millimetres are for; it does not resize anything yet, because there is nothing to scale from."
              : `These measurements are for ${drawnFor}. Choosing another size scales the repeat and the motifs together, from this one — so going back and forth returns the design rather than drifting.`}
          </p>

          <NumberField
            label="Width"
            testId="tile-width"
            unit="mm"
            value={tile[0]}
            min={20}
            max={1200}
            step={5}
            onChange={(v) => setTile([v, tile[1]])}
          />
          <NumberField
            label="Height"
            testId="tile-height"
            unit="mm"
            value={tile[1]}
            min={20}
            max={1200}
            step={5}
            onChange={(v) => setTile([tile[0], v])}
          />
        </Group>

        {doc.scattered ? (
          <>
            {isScatterLayout(layout) && (
            <Group title="Scatter">
              <Slider
                label="Coverage"
                testId="coverage"
                value={density}
                min={0.02}
                max={0.9}
                step={0.01}
                format={(v) => `${Math.round(v * 100)}%`}
                onChange={setDensity}
              />
              <Slider
                label="Rotation"
            testId="rotation"
                value={jitter}
                min={0}
                max={180}
                step={1}
                format={(v) => `±${Math.round(v)}°`}
                onChange={setJitter}
              />
              {isMirrorLayout(layout) && (
                <>
                  <Slider
                    label="On the lines"
            testId="on-the-lines"
                    value={onAxisShare}
                    min={0}
                    max={1}
                    step={0.05}
                    format={(v) => `${Math.round(v * 100)}%`}
                    onChange={setOnAxisShare}
                  />
                  <p className="hint">
                    How often a motif is placed <em>on</em> a mirror line
                    rather than clear of one. A motif there is cut at the line
                    and completed by its own reflection, so it comes out
                    symmetrical — which is how a damask fills the corridor a
                    plain mirror leaves. At zero every motif stays clear of the
                    lines.
                  </p>
                </>
              )}
              <Slider
                label="Smallest"
            testId="smallest"
                value={scale[0]}
                min={0.4}
                max={1}
                step={0.01}
                format={(v) => `${v.toFixed(2)}×`}
                onChange={(v) => setScale([v, Math.max(v, scale[1])])}
              />
              <Slider
                label="Largest"
                value={scale[1]}
                min={1}
                max={2}
                step={0.01}
                format={(v) => `${v.toFixed(2)}×`}
                onChange={(v) => setScale([Math.min(scale[0], v), v])}
              />
            </Group>
            )}

            {!isScatterLayout(layout) && (
              <Group title="Variation">
                <Slider
                  label="Rotation"
                  testId="grid-rotation"
                  value={jitter}
                  min={0}
                  max={180}
                  step={1}
                  format={(v) => `±${Math.round(v)}°`}
                  onChange={setJitter}
                />
                <p className="hint">
                  A grid with no rotation stays strict, which is usually the
                  point of choosing one.
                </p>
              </Group>
            )}

            <Group title="Seed">
              <div className="seed">
                <input
                  type="number"
                  value={seed}
                  min={0}
                  onChange={(e) => setSeed(Number(e.target.value) || 0)}
                />
                <button onClick={() => setSeed(Math.floor(Math.random() * 1e9))}>
                  Reroll
                </button>
              </div>
              <p className="hint">
                {isScatterLayout(layout)
                  ? "The same seed and settings always give the same print, so one you like can be returned to."
                  : "On a grid the cells do not move — rerolling changes which motif sits where."}
              </p>
            </Group>

            <Group title="Motifs">
              {(motifs ?? doc.motifs).map((m) => (
                <MotifRow
                  key={m.id}
                  motif={m}
                  weight={weights[m.id] ?? 0}
                  size={sizes[m.id] ?? m.longest_edge_mm}
                  total={(motifs ?? doc.motifs).reduce(
                    (sum, o) => sum + (weights[o.id] ?? 0),
                    0,
                  )}
                  mixes={(motifs ?? doc.motifs).length > 1}
                  onWeight={(w) => setWeights({ ...weights, [m.id]: w })}
                  onSize={(v) => setSizes({ ...sizes, [m.id]: v })}
                />
              ))}
              {/* What sets the number of motifs is not on this panel, and
                  saying so here is the difference between a control that looks
                  broken and one that is understood. */}
              <p className="hint" data-testid="what-sets-the-count">
                {(motifs ?? doc.motifs).length > 1 &&
                  "Shares are relative to each other: six against one is six times as common, and raising them all together changes nothing. "}
                {isScatterLayout(layout)
                  ? "How many motifs there are is set by Coverage — and by their size, since coverage is a share of the fabric rather than a count, so smaller motifs means more of them."
                  : "How many motifs there are is Rows × Columns. On a grid the cells are fixed, so neither the size nor the share changes the count."}
              </p>
            </Group>
          </>
        ) : (
          /* A section headed "Motifs" on a design that has none is the very
             confusion this panel is meant to avoid. Where there is no artwork
             the explanation belongs at the top of the ground panel, which is
             both the first thing seen and the thing to do about it. */
          hasArtwork && (
            <Group title="Motifs">
              <p className="hint">
                This document places its motifs by hand, so the scatter
                controls do not apply. Its placements are used exactly as
                written.
              </p>
            </Group>
          )
        )}

        {/* The palette size re-reads the artwork and derives colours from it.
            With no artwork there is nothing to read: asking for sixteen
            returns the two it already had and changes nothing, which is the
            most confusing thing a control can do. */}
        {hasArtwork && (
        <Group title="Colours">
          <Slider
            label="Palette"
            testId="palette-size"
            value={colourIndex}
            min={0}
            max={COLOUR_COUNTS.length - 1}
            step={1}
            format={(v) => `${COLOUR_COUNTS[v]}`}
            ends={["Monotone", "Photographic"]}
            onChange={changeColourCount}
          />
          <p className="hint">
            How many colours the artwork is reduced to, the background
            included. A screen printer charges by the colour and a digital
            printer does not, so this is a commercial decision as much as a
            visual one. Changing it re-reads the artwork.
          </p>
          {dropped.length > 0 && (
            <p className="hint warn">
              Discarded {dropped.length === 1 ? "colourway" : "colourways"}{" "}
              {dropped.join(", ")} — a colourway names a colour per slot, and
              there is a different number of slots now.
            </p>
          )}
          {(motifs ?? doc.motifs)
            .filter((m) => m.ground_share > 0.08)
            .map((m) => (
              <p className="hint warn" key={m.id}>
                {Math.round(m.ground_share * 100)}% of <strong>{m.id}</strong>{" "}
                is the background colour. It will look solid on this ground and
                turn into a hole on any other. Usually there is no colour in
                the palette close to that part of the artwork — try more.
              </p>
            ))}
        </Group>
        )}

        <Group title="Background">
          {(() => {
            const colours = palette?.palette ?? doc.palette;
            const ground = colours[0] ?? "#ffffff";
            return (
              <>
                <label className="ground">
                  <input
                    type="color"
                    value={ground}
                    onChange={(e) =>
                      changePalette(() => setSlotColour(0, e.target.value))
                    }
                  />
                  <span className="swatch large" style={{ background: ground }} />
                  <span className="ground-value">{ground}</span>
                </label>
                <p className="hint">
                  The colour of the unprinted fabric. It is slot 0 of the
                  palette rather than a special case, which is why inverting a
                  print to a dark ground costs nothing — and why it recolours
                  with a colourway like everything else.
                </p>
              </>
            );
          })()}
        </Group>

        {hasArtwork && groundPanel}

        {hasArtwork && (
        <Group title="Motif colours">
          <div className="swatches" data-testid="motif-swatches">
            {(palette?.palette ?? doc.palette)
              .map((c, i) => [c, i] as const)
              .filter(([, i]) => i > 0 && !bandSlots.includes(i))
              .map(([c, i]) => (
              <label
                key={i + 1}
                className="swatch-edit"
                title={`Slot ${i + 1} · ${c}`}
              >
                <input
                  type="color"
                  value={c}
                  onChange={(e) =>
                    changePalette(() => setSlotColour(i + 1, e.target.value))
                  }
                />
                <span className="swatch" style={{ background: c }} />
              </label>
            ))}
          </div>
          <p className="hint">
            Motifs store slots, not colours, so changing one recolours every
            motif that uses it at once.
          </p>
        </Group>
        )}

        <Group title="Colourways">
          {/* These derive a whole palette from the one in hand, which has
              nothing to do with motifs — a tartan or a Truchet has a palette
              too. They lived under "Motif colours" and moved here, where what
              they make is what the panel is about. */}
          <div className="derive" data-testid="derive-colourway">
            <button onClick={() => changePalette(() => deriveColourway("hue", 40))}>
              Shift hue
            </button>
            <button onClick={() => changePalette(() => deriveColourway("lighter", 0.2))}>
              Lighter
            </button>
            <button onClick={() => changePalette(() => deriveColourway("darker", 0.2))}>
              Darker
            </button>
            <button onClick={() => changePalette(() => deriveColourway("invert", 1))}>
              Swap ground
            </button>
          </div>
          <p className="hint">
            Candidates to pick from rather than answers. Everything moves
            together, so a palette that worked keeps its relationships.
          </p>

          {(palette?.colourways ?? []).length > 0 && (
            <div className="colourways">
              {(palette?.colourways ?? []).map((name) => (
                <button
                  key={name}
                  className={
                    palette?.active_colourway === name ? "chip active" : "chip"
                  }
                  onClick={() => changePalette(() => useColourway(name))}
                >
                  {name}
                </button>
              ))}
            </div>
          )}
          <button
            onClick={() => {
              const name = window.prompt(
                "Name this colourway",
                `Colourway ${(palette?.colourways.length ?? 0) + 1}`,
              );
              if (name) changePalette(() => rememberColourway(name));
            }}
          >
            Save this colourway…
          </button>
          <p className="hint">
            A range sells in colourways, so they live in one document rather
            than in copies of it — which is also what stops them drifting apart.
          </p>
        </Group>

        {status && <p className="status">{status}</p>}
      </aside>
    </main>
  );
}

function Group({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="group">
      <h2>{title}</h2>
      {children}
    </section>
  );
}

function Readout({
  label,
  value,
  hint,
}: {
  label: string;
  value: string;
  hint?: string;
}) {
  // The anchor is derived from the label rather than passed in. A readout has
  // no behaviour to get wrong, so the handle only has to be findable — and one
  // derived from what it already says cannot drift out of step with it.
  const id = `readout-${label.toLowerCase().replace(/\s+/g, "-")}`;
  return (
    <div className="readout-item" title={hint}>
      <dt>{label}</dt>
      <dd data-testid={id}>{value}</dd>
    </div>
  );
}

/// The nearest offered count to whatever a document actually carries.
///
/// A document may have been written by hand, or by an older build, with a
/// count the slider does not offer. Snapping to the nearest keeps the control
/// honest about roughly where it is rather than jumping the palette to a
/// number nobody asked for.
function nearestColourIndex(colours: number): number {
  let best = 0;
  COLOUR_COUNTS.forEach((c, i) => {
    if (Math.abs(c - colours) < Math.abs(COLOUR_COUNTS[best] - colours)) best = i;
  });
  return best;
}

/// The ground drawn by rule: which figure, how big a cell, and in what.
///
/// Which controls appear depends on the figure, because for three of the six
/// depth does nothing and for three of them a line width does. Showing a
/// control that is ignored is worse than not showing it: a designer moves it,
/// sees no change, and stops trusting the panel.
function OrnamentEditor({
  ornament,
  grid,
  onChange,
}: {
  ornament: OrnamentSpec;
  /// What the engine actually fitted, which is rarely exactly what was asked
  /// for — the grid has to close over the tile.
  grid: FittedGrid | null;
  onChange: (o: OrnamentSpec) => void;
}) {
  const figure = figureIs(ornament.figure);
  const levels = ornament.levels;
  const shown = figure.recurses ? Math.min(levels.length, ornament.depth) : 1;

  return (
    <>
      <label>
        Figure
        <select
          data-testid="ornament-figure"
          value={ornament.figure}
          onChange={(e) => onChange({ ...ornament, figure: e.target.value })}
        >
          {FIGURES.map((f) => (
            <option key={f.id} value={f.id}>
              {f.label}
            </option>
          ))}
        </select>
      </label>

      <Slider
        label="Cell"
        testId="ornament-cell"
        value={ornament.cell_mm}
        min={4}
        max={120}
        step={1}
        format={(v) => `${v} mm`}
        ends={["Fine", "Large"]}
        onChange={(cell_mm) => onChange({ ...ornament, cell_mm })}
      />

      {figure.stroked && (
        <Slider
          label="Line"
          testId="ornament-stroke"
          value={ornament.stroke_mm}
          min={0.5}
          max={Math.max(2, ornament.cell_mm / 2)}
          step={0.5}
          format={(v) => `${v} mm`}
          ends={["Hairline", "Heavy"]}
          onChange={(stroke_mm) => onChange({ ...ornament, stroke_mm })}
        />
      )}

      {figure.recurses && (
        <Slider
          label="Depth"
          testId="ornament-depth"
          value={ornament.depth}
          min={1}
          max={5}
          step={1}
          format={(v) => (v === 1 ? "the figure itself" : `${v} levels`)}
          ends={["Plain", "Fine"]}
          onChange={(depth) => onChange({ ...ornament, depth })}
        />
      )}

      <Slider
        label="Variation"
        testId="ornament-variation"
        value={ornament.variation}
        min={0}
        max={0.6}
        step={0.05}
        format={(v) => (v === 0 ? "strict" : `${Math.round(v * 100)}%`)}
        ends={["Strict", "Drawn"]}
        onChange={(variation) => onChange({ ...ornament, variation })}
      />

      <div className="sequence" data-testid="ornament-levels">
        {levels.slice(0, shown).map((colour, i) => (
          <div className="sequence-row" key={i}>
            <label className="swatch-edit">
              <input
                type="color"
                data-testid={`figure-colour-${i}`}
                value={colour ?? "#ffffff"}
                disabled={colour === null}
                onChange={(e) =>
                  onChange({
                    ...ornament,
                    levels: levels.map((c, j) => (j === i ? e.target.value : c)),
                  })
                }
              />
              <span
                className="swatch"
                style={{
                  background: colour ?? "transparent",
                  borderStyle: colour === null ? "dashed" : "solid",
                }}
              />
            </label>
            <span className="hint">
              {!figure.recurses
                ? "The figure"
                : i === 0
                  ? "Outermost"
                  : `Level ${i + 1}`}
            </span>
            <button
              type="button"
              data-testid={`figure-fabric-${i}`}
              onClick={() =>
                onChange({
                  ...ornament,
                  levels: levels.map((c, j) =>
                    j === i ? (c === null ? "#3c78a0" : null) : c,
                  ),
                })
              }
            >
              {colour === null ? "Ink" : "Fabric"}
            </button>
          </div>
        ))}
      </div>

      {figure.recurses && levels.length < ornament.depth && (
        <button
          type="button"
          data-testid="ornament-add-level"
          onClick={() =>
            onChange({ ...ornament, levels: [...levels, "#3c78a0"] })
          }
        >
          Colour the next level
        </button>
      )}

      <div className="row">
        <button
          type="button"
          data-testid="ornament-reseed"
          disabled={ornament.variation === 0}
          onClick={() => onChange({ ...ornament, seed: ornament.seed + 1 })}
        >
          Draw another
        </button>
      </div>

      {grid && (
        <p className="hint" data-testid="ornament-grid">
          Drawn as {grid.cols} x {grid.rows} cells of{" "}
          {grid.cell_w_mm.toFixed(1)} x {grid.cell_h_mm.toFixed(1)} mm. The
          grid has to close over the repeat, so the cell size you ask for is
          rounded to the nearest one that does.
        </p>
      )}

      <p className="hint">
        Drawn by rule rather than grown, and the opposite of a texture in every
        way: all lattice, no accident. The line figures meet each cell edge at
        its middle, which is why neighbours join however they were turned — and
        why the repeat joins too.
      </p>
    </>
  );
}

/// The reaction-diffusion ground: what it grows, how big, and in what.
///
/// Fewer controls than the band editor because there is less to decide: the
/// growth and the scale do most of the work, and the seed is the only way to
/// ask for *a different one of the same*. No lattice control, because a grown
/// ground has no lattice in it.
function TextureEditor({
  texture,
  shares,
  onChange,
}: {
  texture: TextureSpec;
  /// What each level covered, measured from the field the engine actually
  /// grew. Shown because a level can come out empty — the thresholds are
  /// evenly spaced by concentration, and the field may not reach between two
  /// of them.
  shares: number[];
  onChange: (t: TextureSpec) => void;
}) {
  const levels = texture.levels;
  return (
    <>
      <label>
        Growth
        <select
          data-testid="texture-growth"
          value={texture.growth}
          onChange={(e) => onChange({ ...texture, growth: e.target.value })}
        >
          {GROWTHS.map((g) => (
            <option key={g.id} value={g.id}>
              {g.label}
            </option>
          ))}
        </select>
      </label>

      <Slider
        label="Scale"
        testId="texture-cells"
        value={texture.cells}
        min={48}
        max={384}
        step={16}
        format={(v) => `${v} cells across`}
        ends={["Large", "Fine"]}
        onChange={(cells) => onChange({ ...texture, cells })}
      />

      <Slider
        label="Settle"
        testId="texture-steps"
        value={texture.steps}
        min={200}
        max={6000}
        step={100}
        format={(v) => `${v} steps`}
        ends={["Broken", "Settled"]}
        onChange={(steps) => onChange({ ...texture, steps })}
      />

      <div className="sequence" data-testid="texture-levels">
        {levels.map((colour, i) => (
          <div className="sequence-row" key={i}>
            <label className="swatch-edit">
              <input
                type="color"
                data-testid={`level-colour-${i}`}
                value={colour ?? "#ffffff"}
                disabled={colour === null}
                onChange={(e) =>
                  onChange({
                    ...texture,
                    levels: levels.map((c, j) =>
                      j === i ? e.target.value : c,
                    ),
                  })
                }
              />
              <span
                className="swatch"
                style={{
                  background: colour ?? "transparent",
                  borderStyle: colour === null ? "dashed" : "solid",
                }}
              />
            </label>
            <span className="hint">
              {i === 0 ? "Sparsest" : i === levels.length - 1 ? "Densest" : "Between"}
              {shares[i] !== undefined && ` · ${Math.round(shares[i] * 100)}%`}
            </span>
            <button
              type="button"
              data-testid={`level-fabric-${i}`}
              onClick={() =>
                onChange({
                  ...texture,
                  levels: levels.map((c, j) =>
                    j === i ? (c === null ? "#3c78a0" : null) : c,
                  ),
                })
              }
            >
              {colour === null ? "Ink" : "Fabric"}
            </button>
            {levels.length > 2 && (
              <button
                type="button"
                data-testid={`level-remove-${i}`}
                onClick={() =>
                  onChange({
                    ...texture,
                    levels: levels.filter((_, j) => j !== i),
                  })
                }
              >
                Remove
              </button>
            )}
          </div>
        ))}
      </div>

      <div className="row">
        <button
          type="button"
          data-testid="texture-add-level"
          disabled={levels.length >= 5}
          onClick={() =>
            onChange({ ...texture, levels: [...levels, "#3c78a0"] })
          }
        >
          Add a tone
        </button>
        <button
          type="button"
          data-testid="texture-reseed"
          onClick={() => onChange({ ...texture, seed: texture.seed + 1 })}
        >
          Grow another
        </button>
      </div>

      <p className="hint">
        Two chemicals spreading across the tile and consuming each other. It is
        grown on a torus, so it meets itself at every edge — which is also why
        it needs a block repeat. Nothing is placed, so there is no lattice in
        it for the eye to find.
      </p>
    </>
  );
}

function Slider({
  label,
  value,
  min,
  max,
  step,
  format,
  onChange,
  ends,
  testId,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  format: (v: number) => string;
  onChange: (v: number) => void;
  /** Names for the two ends, where the scale is a register rather than a
   *  quantity and the numbers alone do not say what it is for. */
  ends?: [string, string];
  /** Stable handle for tests. See `docs/test-strategy.md`; anchors go on the
   *  control itself, not its wrapper, so a test drives what a person drives. */
  testId?: string;
}) {
  return (
    <label className="slider">
      <span className="row">
        <span>{label}</span>
        <output>{format(value)}</output>
      </span>
      <input
        type="range"
        data-testid={testId}
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(e) => onChange(Number(e.target.value))}
      />
      {ends && (
        <span className="row ends">
          <span>{ends[0]}</span>
          <span>{ends[1]}</span>
        </span>
      )}
    </label>
  );
}

function NumberField({
  label,
  unit,
  value,
  min,
  max,
  step,
  onChange,
  testId,
}: {
  label: string;
  unit: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
  testId?: string;
}) {
  return (
    <label className="number">
      <span>
        {label} <span className="muted">({unit})</span>
      </span>
      <input
        type="number"
        data-testid={testId}
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(e) => onChange(Number(e.target.value) || min)}
      />
    </label>
  );
}

function MotifRow({
  motif,
  weight,
  size,
  total,
  mixes,
  onWeight,
  onSize,
}: {
  motif: MotifEntry;
  weight: number;
  size: number;
  /// Every motif's weight added together, so a row can say what share of the
  /// mix it is rather than a number that means nothing on its own.
  total: number;
  /// Whether there is a mix at all. One motif is always the whole of it.
  mixes: boolean;
  onWeight: (w: number) => void;
  onSize: (v: number) => void;
}) {
  return (
    <div className="motif">
      <div className="motif-name">{motif.id}</div>
      {/* Shown as the share it works out to, not as the raw weight. It was
          labelled "How often", which reads as a frequency — and was reported
          as doing nothing, because it is relative: raising it alone takes from
          the others and raising them all together changes nothing at all.
          A percentage that moves when its neighbours move says that by
          itself. Hidden outright for a single motif, which is always the whole
          mix however the slider is set. */}
      {mixes && (
        <Slider
          label="Share"
          testId={`share-${motif.id}`}
          value={weight}
          min={0}
          max={8}
          step={0.5}
          format={(v) =>
            v === 0
              ? "off"
              : total > 0
                ? `${Math.round((v / total) * 100)}% of the mix`
                : "—"
          }
          onChange={onWeight}
        />
      )}
      <Slider
        label="Size"
        testId={`size-${motif.id}`}
        value={size}
        min={5}
        max={400}
        step={1}
        format={(v) => `${Math.round(v)} mm`}
        onChange={onSize}
      />
    </div>
  );
}

/// A modal dialogue: content over the whole window, with the rest of it inert.
///
/// Built here rather than with the browser's own `<dialog>`, which would give
/// the backdrop, the Escape key and a focus trap for nothing. The reason is
/// that jsdom — which the component tests run in — still does not implement
/// `showModal`, so a native dialogue could only be driven in a test behind a
/// shim that fakes the very thing being claimed: the tests would pass over a
/// dialogue that was never modal. Everything asserted here is really done.
///
/// Reconsider when jsdom implements it. This is thirty lines that the platform
/// would otherwise own, and platform focus handling is better than anybody's.
function Modal({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  const panel = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    // Where focus was before this opened, so it can be put back. A dialogue
    // that drops focus to the top of the document on close loses a keyboard
    // user their place entirely.
    const opener = document.activeElement as HTMLElement | null;
    panel.current?.focus();
    return () => opener?.focus?.();
  }, []);

  function focusable(): HTMLElement[] {
    return Array.from(
      panel.current?.querySelectorAll<HTMLElement>(
        "button, [href], input, select, textarea, [tabindex]:not([tabindex='-1'])",
      ) ?? [],
    ).filter((el) => !el.hasAttribute("disabled"));
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key !== "Tab") return;
    // Keep Tab inside. Without this the next Tab leaves for the controls
    // behind the backdrop, which are visible, unreachable by mouse, and still
    // in the tab order — the worst of the three states.
    const stops = focusable();
    if (stops.length === 0) return;
    const first = stops[0];
    const last = stops[stops.length - 1];
    const on = document.activeElement;
    if (e.shiftKey && (on === first || on === panel.current)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && on === last) {
      e.preventDefault();
      first.focus();
    }
  }

  return (
    <div className="backdrop" data-testid="modal-backdrop">
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        data-testid="modal"
        tabIndex={-1}
        ref={panel}
        onKeyDown={onKeyDown}
      >
        <header className="modal-head">
          <h2>{title}</h2>
          <button data-testid="modal-close" aria-label="Close" onClick={onClose}>
            ✕
          </button>
        </header>
        {children}
      </div>
    </div>
  );
}

function ExportPanel({
  request,
  plan,
  onChange,
  onCancel,
  onExport,
  onPickProfile,
}: {
  request: ExportRequest;
  plan: ExportPlan | null;
  onChange: (r: ExportRequest) => void;
  onCancel: () => void;
  onExport: () => void;
  onPickProfile: () => void;
}) {
  const set = <K extends keyof ExportRequest>(key: K, value: ExportRequest[K]) =>
    onChange({ ...request, [key]: value });

  return (
    <section className="export">
      <div className="export-row">
        <label>
          Format
          <select
            value={request.format}
            onChange={(e) => set("format", e.target.value as "png" | "tiff")}
          >
            <option value="tiff">TIFF (for a print bureau)</option>
            <option value="png">PNG (quick look)</option>
          </select>
        </label>

        <label>
          Resolution
          <select value={request.dpi} onChange={(e) => set("dpi", Number(e.target.value))}>
            <option value={150}>150 DPI</option>
            <option value={300}>300 DPI</option>
            <option value={72}>72 DPI</option>
            <option value={600}>600 DPI</option>
          </select>
        </label>

        <label>
          What to write
          <select
            value={request.shape}
            onChange={(e) => set("shape", e.target.value as "tile" | "sheet")}
          >
            <option value="tile">One tile — the printer repeats it</option>
            <option value="sheet">A repeated sheet — ready to print</option>
          </select>
        </label>

        {request.shape === "tile" && (
          <p className="hint" data-testid="one-tile-note">
            One tile is the whole design: it is seamless on all four sides, and
            the printer lays it edge to edge. It will look far emptier than the
            preview, which shows several side by side — same size on the cloth,
            fewer of them in the picture.
          </p>
        )}

        {request.shape === "sheet" && (
          <>
            <NumberField
              label="Across"
              unit="tiles"
              value={request.min_cols}
              min={1}
              max={24}
              step={1}
              onChange={(v) => set("min_cols", v)}
            />
            <NumberField
              label="Down"
              unit="tiles"
              value={request.min_rows}
              min={1}
              max={24}
              step={1}
              onChange={(v) => set("min_rows", v)}
            />
          </>
        )}
      </div>

      <div className="export-row">
        <label>
          Colour
          <select
            value={request.cmyk_profile ? "cmyk" : "rgb"}
            onChange={(e) =>
              e.target.value === "rgb"
                ? set("cmyk_profile", null)
                : onPickProfile()
            }
          >
            <option value="rgb">RGB — the bureau separates</option>
            <option value="cmyk">CMYK — converted here</option>
          </select>
        </label>
        {request.cmyk_profile && (
          <p className="hint profile">
            Using <code>{request.cmyk_profile.split("/").pop()}</code>{" "}
            <button className="link" onClick={onPickProfile}>
              change
            </button>
          </p>
        )}
      </div>

      {request.cmyk_profile === null && (
        <p className="hint">
          Many digital textile bureaux prefer RGB and separate it themselves,
          because they know their press. Ask before converting.
        </p>
      )}

      {request.format === "tiff" && (
        <label className="check">
          <input
            type="checkbox"
            checked={request.compress}
            onChange={(e) => set("compress", e.target.checked)}
          />
          <span>Compress (LZW — lossless, about a third the size)</span>
        </label>
      )}

      {plan && (
        <>
          <dl className="readout">
            <Readout
              label="Physical size"
              value={`${Math.round(plan.width_mm)} × ${Math.round(plan.height_mm)} mm`}
            />
            <Readout label="Pixels" value={`${plan.width_px} × ${plan.height_px}`} />
            {request.shape === "sheet" && (
              <Readout label="Tiles" value={`${plan.cols} × ${plan.rows}`} />
            )}
            <Readout label="About" value={`${plan.megabytes.toFixed(1)} MB`} />
          </dl>

          {plan.width_verdict === "wide" && (
            <p className="hint">
              {(plan.width_mm / 1000).toFixed(2)} m wide — past a standard{" "}
              {(request.roll_width_mm / 1000).toFixed(1)} m fabric roll. Wide-format
              printers take this; most do not.
            </p>
          )}
          {plan.width_verdict === "too-wide" && (
            <p className="error">
              {(plan.width_mm / 1000).toFixed(2)} m wide — past grand format, so this is
              unlikely to be printable as one piece. Export the tile instead and let the
              printer repeat it.
            </p>
          )}
          {request.shape === "sheet" &&
            (plan.cols !== request.min_cols || plan.rows !== request.min_rows) && (
              <p className="hint">
                Written as {plan.cols} × {plan.rows} tiles rather than {request.min_cols} ×{" "}
                {request.min_rows}: this repeat needs that many for the sheet itself to
                tile.
              </p>
            )}
        </>
      )}

      <div className="export-actions">
        <button onClick={onCancel}>Cancel</button>
        {/* Not "Choose a file": there is no file to choose. This names one
            that does not exist yet, which is what every platform calls Save
            as. */}
        <button
          className="primary"
          data-testid="save-as"
          onClick={onExport}
          disabled={!plan}
        >
          Save as…
        </button>
      </div>
    </section>
  );
}
