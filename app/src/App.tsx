import { useCallback, useEffect, useRef, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  apply,
  exportPng,
  newFromImages,
  openDocument,
  saveDocument,
  saveTarget,
  type MotifEntry,
  type Opened,
  type Params,
  type Preview,
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
  const [sizes, setSizes] = useState<Record<string, number>>({});

  const paneRef = useRef<HTMLDivElement | null>(null);
  const timer = useRef<number | undefined>(undefined);
  // Guards against a slow render landing after a newer one and overwriting it.
  const generation = useRef(0);

  const refresh = useCallback(async () => {
    if (!doc) return;
    const mine = ++generation.current;
    setBusy(true);
    const params: Params = {
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
    };
    try {
      const next = await apply(params);
      if (mine === generation.current) {
        setPreview(next);
        setError(null);
      }
    } catch (e) {
      if (mine === generation.current) setError(String(e));
    } finally {
      if (mine === generation.current) setBusy(false);
    }
  }, [
    doc,
    tile,
    lattice,
    density,
    jitter,
    scale,
    seed,
    weights,
    sizes,
    layout,
    preventOverlap,
    gridRows,
    gridCols,
  ]);

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

  async function onExport() {
    const target = await saveDialog({
      title: "Export the tile",
      defaultPath: `${doc?.name ?? "pattern"}-${Math.round(tile[0])}mm-300dpi.png`,
      filters: [{ name: "PNG", extensions: ["png"] }],
    });
    if (typeof target !== "string") return;
    try {
      // The tile alone, at print resolution: what a bureau is sent.
      await exportPng(target, 300, 1, 1);
      setStatus(`Exported the tile at 300 DPI`);
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
            <button onClick={onNew}>New…</button>
            <button onClick={onOpen}>Open…</button>
            <button onClick={onSave}>Save</button>
            <button className="primary" onClick={onExport}>
              Export tile…
            </button>
          </div>
        </header>

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

        <Group title="Palette">
          <div className="swatches">
            {doc.palette.map((c, i) => (
              <span key={i} className="swatch" style={{ background: c }} title={`Slot ${i}${i === 0 ? " (ground)" : ""} · ${c}`} />
            ))}
          </div>
          <p className="hint">
            Motifs store slots, not colours, so a colourway is a palette swap.
            Editing palettes arrives with export, in M5.
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
