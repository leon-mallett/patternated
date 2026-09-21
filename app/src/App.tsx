import { useCallback, useEffect, useRef, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  apply,
  deriveColourway,
  exportFile,
  exportPlan,
  garments,
  mockup,
  newFromImages,
  openDocument,
  proof,
  rememberColourway,
  saveDocument,
  saveTarget,
  setSlotColour,
  useColourway,
  type MotifEntry,
  type Opened,
  type Params,
  type Preview,
  type ExportPlan,
  type ExportRequest,
  type PaletteState,
  type GarmentOption,
  type MockupView,
  LAYOUTS,
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
  // Guards against a slow render landing after a newer one and overwriting it.
  const generation = useRef(0);

  const currentParams = useCallback(
    (): Params => ({
      tile_mm: tile,
      lattice,
      density,
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
    setJitter(opened.rotation_jitter_deg);
    setScale(opened.scale_range);
    setSeed(opened.seed);
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

  async function onNew() {
    const chosen = await openDialog({
      title: "Choose the artwork for this pattern",
      multiple: true,
      filters: [{ name: "Artwork", extensions: ["png"] }],
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
        </div>
        <p className="hint start-hint">
          Start with one or more PNG images, each showing a single motif. They
          can have a transparent background or sit on plain paper — the
          background is measured away either way. A palette is worked out from
          the artwork itself.
        </p>
        <p className="hint start-hint">
          A <strong>saved pattern</strong> is a small <code>.json</code> file
          recording the repeat, the motifs it uses and where they sit. There are
          examples under <code>examples/</code> in the project folder.
        </p>
        {error && <p className="error">{error}</p>}
      </main>
    );
  }

  return (
    <main className="app">
      <section className="preview" ref={paneRef}>
        <header>
          <div>
            <strong>{doc.name}</strong>
            <span className="muted">
              {" "}
              · {preview ? `${preview.cols}×${preview.rows}` : `${FIELD}×${FIELD}`} repeat
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
            <button onClick={onNew}>New…</button>
            <button onClick={onOpen}>Open…</button>
            <button onClick={onSave}>Save</button>
            <button className="primary" onClick={() => setExporting(true)}>
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
                {Math.round(worn.chest_mm / 10)} × {Math.round(worn.length_mm / 10)} cm
                {" · "}
                {worn.repeats_across.toFixed(1)} repeats across the chest
              </span>
            </div>
          </div>
        )}

        <div className={busy ? "canvas working" : "canvas"}>
          {preview ? (
            <img
              src={`data:image/png;base64,${preview.png_base64}`}
              alt={`${preview.cols} by ${preview.rows} repeat of ${doc.name}`}
            />
          ) : (
            <p className="muted">Rendering…</p>
          )}
        </div>

        {error && <p className="error">{error}</p>}

        {exporting && (
          <ExportPanel
            request={exp}
            plan={plan}
            onChange={setExp}
            onCancel={() => setExporting(false)}
            onExport={onExport}
            onPickProfile={onPickProfile}
          />
        )}

        {preview && !preview.window_repeats && (
          <p className="hint">
            This lattice has no small whole-tile window that repeats, so the
            preview's opposite edges will not meet. The exported tile is still
            seamless.
          </p>
        )}

        {preview && (
          <dl className="readout">
            <Readout label="Repeat" value={`${tile[0]} × ${tile[1]} mm`} />
            <Readout label="Motifs" value={String(preview.placements)} />
            <Readout
              label="Coverage"
              value={`${Math.round(preview.achieved_density * 100)}% (asked ${Math.round(
                density * 100,
              )}%)`}
            />
            <Readout
              label="Colours"
              value={String(preview.colours)}
              hint="What a screen printer charges by"
            />
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

      <aside className="controls">
        <Group title="Layout">
          <label className="check">
            <input
              type="checkbox"
              checked={preventOverlap}
              onChange={(e) => setPreventOverlap(e.target.checked)}
            />
            <span>Prevent overlapping</span>
          </label>
          <p className="hint">
            {layout === "tossed"
              ? "Keeps motifs from intersecting. Asking for more coverage than fits simply stops adding motifs — the coverage readout shows what was reached."
              : "Keeps motifs from intersecting. A grid's cells are fixed, so motifs too large for them are shrunk to fit rather than drawn over each other."}
          </p>
          <label>
            Arrangement
            <select value={layout} onChange={(e) => setLayout(e.target.value)}>
              {LAYOUTS.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.label}
                </option>
              ))}
            </select>
          </label>
          {layout !== "tossed" && (
            <>
              <NumberField
                label="Rows"
                unit="per tile"
                value={gridRows}
                min={1}
                max={24}
                step={1}
                onChange={setGridRows}
              />
              <NumberField
                label="Columns"
                unit="per tile"
                value={gridCols}
                min={1}
                max={24}
                step={1}
                onChange={setGridCols}
              />
            </>
          )}
        </Group>

        <Group title="Repeat">
          <label>
            Type
            <select value={lattice} onChange={(e) => setLattice(e.target.value)}>
              <option value="half-drop">Half-drop</option>
              <option value="block">Block</option>
              <option value="brick">Brick</option>
              <option value="quarter-drop">Quarter-drop</option>
            </select>
          </label>
          <NumberField
            label="Width"
            unit="mm"
            value={tile[0]}
            min={20}
            max={1200}
            step={5}
            onChange={(v) => setTile([v, tile[1]])}
          />
          <NumberField
            label="Height"
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
            {layout === "tossed" && (
            <Group title="Scatter">
              <Slider
                label="Coverage"
                value={density}
                min={0.02}
                max={0.9}
                step={0.01}
                format={(v) => `${Math.round(v * 100)}%`}
                onChange={setDensity}
              />
              <Slider
                label="Rotation"
                value={jitter}
                min={0}
                max={180}
                step={1}
                format={(v) => `±${Math.round(v)}°`}
                onChange={setJitter}
              />
              <Slider
                label="Smallest"
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

            {layout !== "tossed" && (
              <Group title="Variation">
                <Slider
                  label="Rotation"
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
                {layout === "tossed"
                  ? "The same seed and settings always give the same print, so one you like can be returned to."
                  : "On a grid the cells do not move — rerolling changes which motif sits where."}
              </p>
            </Group>

            <Group title="Motifs">
              {doc.motifs.map((m) => (
                <MotifRow
                  key={m.id}
                  motif={m}
                  weight={weights[m.id] ?? 0}
                  size={sizes[m.id] ?? m.longest_edge_mm}
                  onWeight={(w) => setWeights({ ...weights, [m.id]: w })}
                  onSize={(v) => setSizes({ ...sizes, [m.id]: v })}
                />
              ))}
              <p className="hint">
                Relative, not percentages. Six against one is six times as
                common. Zero leaves a motif out.
              </p>
            </Group>
          </>
        ) : (
          <Group title="Motifs">
            <p className="hint">
              This document places its motifs by hand, so the scatter controls do
              not apply. Its placements are used exactly as written.
            </p>
          </Group>
        )}

        <Group title="Colour">
          <div className="swatches">
            {(palette?.palette ?? doc.palette).map((c, i) => (
              <label
                key={i}
                className="swatch-edit"
                title={`Slot ${i}${i === 0 ? " — the ground" : ""} · ${c}`}
              >
                <input
                  type="color"
                  value={c}
                  onChange={(e) =>
                    changePalette(() => setSlotColour(i, e.target.value))
                  }
                />
                <span className="swatch" style={{ background: c }} />
              </label>
            ))}
          </div>
          <p className="hint">
            Motifs store slots, not colours, so changing one recolours every
            motif at once. Slot 0 is the ground — inverting a print to a dark
            background is an ordinary edit here.
          </p>

          <div className="derive">
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
        </Group>

        <Group title="Colourways">
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

function Readout({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="readout-item" title={hint}>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
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
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  format: (v: number) => string;
  onChange: (v: number) => void;
}) {
  return (
    <label className="slider">
      <span className="row">
        <span>{label}</span>
        <output>{format(value)}</output>
      </span>
      <input
        type="range"
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(e) => onChange(Number(e.target.value))}
      />
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
}: {
  label: string;
  unit: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
}) {
  return (
    <label className="number">
      <span>
        {label} <span className="muted">({unit})</span>
      </span>
      <input
        type="number"
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
  onWeight,
  onSize,
}: {
  motif: MotifEntry;
  weight: number;
  size: number;
  onWeight: (w: number) => void;
  onSize: (v: number) => void;
}) {
  return (
    <div className="motif">
      <div className="motif-name">{motif.id}</div>
      <Slider
        label="How often"
        value={weight}
        min={0}
        max={8}
        step={0.5}
        format={(v) => (v === 0 ? "off" : v.toFixed(1))}
        onChange={onWeight}
      />
      <Slider
        label="Size"
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
        <button className="primary" onClick={onExport} disabled={!plan}>
          Choose a file…
        </button>
      </div>
    </section>
  );
}
