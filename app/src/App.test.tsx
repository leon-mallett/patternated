// @vitest-environment jsdom
//
// The interface, driven by anchor rather than by pixel.
//
// Mocked at *our* boundary — `./api`, the one module every Tauri command goes
// through — rather than at the framework's internals. That keeps the surface
// tiny and the tests stable: a test says which commands it cares about and the
// rest stay real.
//
// What these catch that the 45 Rust tests beneath them cannot: which controls
// a layout shows, whether a warning appears when the engine reports a problem,
// and whether a confirmation actually gates a destructive action. All of that
// lives in the component and none of it is reachable from the command layer.
//
// What they do not catch is layout. Every control here can be present, labelled
// and correctly enabled while sitting on top of its neighbour.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { makeMotif, makeOpened, makePreview } from "./test/factories";
import { DEFAULT_ORNAMENT, DEFAULT_TEXTURE } from "./api";
import type { ExportPlan, GroundSpec, Opened, PaletteSized, Preview } from "./api";

/** What the fake engine will answer with, rewritten per test. */
const engine = {
  opened: makeOpened(),
  preview: makePreview(),
  closeOutcome: { closed: true, never_saved: false, changed: false },
  sized: null as PaletteSized | null,
  applied: [] as unknown[],
  paletteSizes: [] as number[],
  closeCalls: [] as boolean[],
  grounds: [] as GroundSpec[],
  groundSlots: [3, 4],
  levelShares: [0.7, 0.3],
  plan: {
    width_mm: 240,
    height_mm: 180,
    width_px: 2835,
    height_px: 2126,
    cols: 1,
    rows: 1,
    width_verdict: "fits",
    megabytes: 24,
    suggested_name: "cherries-tile-300dpi.tif",
  } as ExportPlan,
  exported: [] as string[],
  cuts: [] as string[],
  cut: {
    factor: 0.65,
    tile_mm: [156, 117] as [number, number],
    motifs: [makeMotif({ longest_edge_mm: 34 })],
    drawn_for: "T-shirt, age 3–4",
  },
  fittedGrid: { cols: 10, rows: 8, cell_w_mm: 24, cell_h_mm: 22.5 } as {
    cols: number;
    rows: number;
    cell_w_mm: number;
    cell_h_mm: number;
  } | null,
};

vi.mock("./api", async (importOriginal) => {
  const real = await importOriginal<typeof import("./api")>();
  return {
    ...real,
    openDocument: async () => engine.opened,
    newFromImages: async () => engine.opened,
    newPlain: async () => engine.opened,
    cutFor: async (garment: string) => {
      engine.cuts.push(garment);
      return engine.cut;
    },
    exportPlan: async () => engine.plan,
    exportFile: async (path: string) => {
      engine.exported.push(path);
      return path;
    },
    apply: async (params: unknown) => {
      engine.applied.push(params);
      return engine.preview;
    },
    // The real chart, abridged. An empty list let a garment control render
    // with nothing in it and still pass, which is a test of nothing.
    garments: async () => [
      { name: "T-shirt, age 3–4", chest_mm: 330, length_mm: 440 },
      { name: "T-shirt, age 9–10", chest_mm: 400, length_mm: 580 },
      { name: "T-shirt, adult M", chest_mm: 510, length_mm: 740 },
    ],
    mockup: async () => ({
      png_base64: "",
      width: 1,
      height: 1,
      repeats_across: 1,
      chest_mm: 1,
      length_mm: 1,
    }),
    closeDocument: async (discard: boolean) => {
      engine.closeCalls.push(discard);
      return discard
        ? { closed: true, never_saved: false, changed: false }
        : engine.closeOutcome;
    },
    setGround: async (spec: GroundSpec) => {
      engine.grounds.push(spec);
      const painted = spec.bands.some((b) => b.colour !== null);
      // The engine applies a reflection once and returns the expanded
      // sequence, and `mirrored` is an instruction rather than a stored mode.
      // The fake has to do that too: a mock that answers differently from the
      // thing it stands for turns a component test into a test of the mock.
      const bands =
        spec.mirrored && spec.bands.length >= 3
          ? [...spec.bands, ...spec.bands.slice(1, -1).reverse()]
          : spec.bands;
      return {
        ...spec,
        bands,
        mirrored: false,
        style: painted ? "check" : "none",
        width_mm: spec.bands[0]?.width_mm ?? 18,
        gap_mm: spec.bands[1]?.width_mm ?? 18,
        slots: painted ? engine.groundSlots : [],
        ground_colours: painted ? engine.groundSlots.length : 0,
        palette: painted
          ? [...engine.opened.palette, "#9a8f7e", "#6b6154"]
          : engine.opened.palette,
        lattice: "block",
        level_shares: engine.levelShares,
        grid: engine.fittedGrid,
      };
    },
    setPaletteSize: async (colours: number) => {
      engine.paletteSizes.push(colours);
      return (
        engine.sized ?? {
          palette: engine.opened.palette,
          palette_colours: colours,
          colourways: [],
          active_colourway: null,
          colourways_dropped: [],
          motifs: engine.opened.motifs,
        }
      );
    },
  };
});

/** The only other thing the component reaches outside itself. */
const dialog = { confirmed: true, chosen: "/patterns/cherries.json" as unknown };
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: async () => dialog.chosen,
  save: async () => dialog.chosen,
  confirm: async () => dialog.confirmed,
}));

import App from "./App";

/** Open a document, which is where every test below starts. */
async function openApp(over: Partial<Opened> = {}, preview: Partial<Preview> = {}) {
  engine.opened = makeOpened(over);
  engine.preview = makePreview(preview);
  const user = userEvent.setup();
  render(<App />);
  await user.click(screen.getByText("Open a saved pattern…"));
  await screen.findByTestId("close");
  return user;
}

beforeEach(() => {
  engine.opened = makeOpened();
  engine.preview = makePreview();
  engine.closeOutcome = { closed: true, never_saved: false, changed: false };
  engine.sized = null;
  engine.applied = [];
  engine.paletteSizes = [];
  engine.closeCalls = [];
  engine.grounds = [];
  engine.groundSlots = [3, 4];
  engine.levelShares = [0.7, 0.3];
  engine.exported = [];
  engine.cuts = [];
  engine.fittedGrid = { cols: 10, rows: 8, cell_w_mm: 24, cell_h_mm: 22.5 };
  dialog.confirmed = true;
  dialog.chosen = "/patterns/cherries.json";
});

afterEach(cleanup);

describe("the start screen", () => {
  it("offers every way in", () => {
    // The defect this guards was reported from the running app: the shell
    // could open a document and appeared to have no way to create one.
    render(<App />);
    expect(screen.getByText("New pattern from artwork…")).toBeTruthy();
    expect(screen.getByText("Open a saved pattern…")).toBeTruthy();
    // And the third: a stripe, check or tartan has no artwork in it, so
    // demanding a PNG would put those patterns out of reach entirely.
    expect(screen.getByTestId("new-plain")).toBeTruthy();
  });

  it("starts a pattern with no artwork", async () => {
    engine.opened = makeOpened({ motifs: [], scattered: false });
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByTestId("new-plain"));

    await screen.findByTestId("close");
    // The ground panel is the whole design here, and the scatter note says so
    // rather than claiming the motifs were placed by hand.
    expect(screen.getByTestId("ground-preset")).toBeTruthy();
    expect(screen.getByText(/no artwork in it/)).toBeTruthy();
  });

  it("offers nothing that only arranges motifs when there are none", async () => {
    // Reported from the app: a pattern made entirely of a ground still led
    // with a panel of controls for placing motifs — an arrangement, an
    // overlap guard, rows and columns. None of them can do anything here, and
    // a control that does nothing teaches a designer to distrust the rest.
    engine.opened = makeOpened({ motifs: [], scattered: false });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByTestId("new-plain"));
    await screen.findByTestId("close");

    for (const id of ["layout", "prevent-overlap", "grid-rows", "grid-cols"]) {
      expect(screen.queryByTestId(id)).toBeNull();
    }
    // Nor the two readouts that count them.
    const readout = await screen.findByTestId("readout");
    expect(readout.textContent).not.toContain("Coverage");
    expect(readout.textContent).not.toContain("Motifs");
    // What the design *is* made of is still there.
    expect(screen.getByTestId("ground-kind")).toBeTruthy();
    expect(readout.textContent).toContain("Repeat");
  });

  it("leads with the ground when the ground is the design", async () => {
    // A panel that opens with the same controls whatever the document is asks
    // the designer to skip past the half that cannot apply. Here the ground is
    // the whole design, so it comes first — and the controls that only reduce
    // or edit motif colours are not there at all, because there is no artwork
    // to reduce and no motif slot to edit.
    engine.opened = makeOpened({ motifs: [], scattered: false });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByTestId("new-plain"));
    await screen.findByTestId("close");

    expect(sections()[0]).toBe("Ground");

    expect(screen.queryByTestId("palette-size")).toBeNull();
    expect(screen.queryByTestId("motif-swatches")).toBeNull();
    // But a ground has a palette like anything else, so the things that
    // derive one from it stay.
    expect(screen.getByTestId("derive-colourway")).toBeTruthy();
  });

  it("composes a whole panel for the workflow, not one panel for both", async () => {
    // The composition itself, pinned. Each section here acts on a design made
    // entirely of a ground; nothing in it is about arranging motifs, reducing
    // their colours or editing them, because there are none to arrange.
    engine.opened = makeOpened({ motifs: [], scattered: false });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByTestId("new-plain"));
    await screen.findByTestId("close");

    expect(sections()).toEqual(["Ground", "Repeat", "Background", "Colourways"]);
    expect(screen.getByTestId("ground-is-the-design")).toBeTruthy();
  });

  it("still offers them when there is artwork to arrange", async () => {
    // The other half, so the test above cannot be satisfied by hiding the
    // panel from everybody.
    await openApp();
    expect(screen.getByTestId("layout")).toBeTruthy();
    expect(screen.getByTestId("prevent-overlap")).toBeTruthy();
    // And the ground sits under the background there, where it is what the
    // motifs are strewn on rather than the design itself.
    expect(sections().indexOf("Ground")).toBeGreaterThan(
      sections().indexOf("Background"),
    );
    expect((await screen.findByTestId("readout")).textContent).toContain("Coverage");
  });

  it("is where closing puts you back", async () => {
    const user = await openApp();
    await user.click(screen.getByTestId("close"));
    await screen.findByText("New pattern from artwork…");
    expect(screen.queryByTestId("close")).toBeNull();
  });
});

describe("closing with unsaved work", () => {
  it("asks first, and does not close when refused", async () => {
    engine.closeOutcome = { closed: false, never_saved: false, changed: true };
    dialog.confirmed = false;
    const user = await openApp();

    await user.click(screen.getByTestId("close"));

    await waitFor(() => expect(engine.closeCalls).toEqual([false]));
    // Still open: the refusal was honoured rather than closed anyway.
    expect(screen.getByTestId("close")).toBeTruthy();
  });

  it("closes when the warning is accepted", async () => {
    engine.closeOutcome = { closed: false, never_saved: true, changed: true };
    dialog.confirmed = true;
    const user = await openApp();

    await user.click(screen.getByTestId("close"));

    await screen.findByText("New pattern from artwork…");
    // Asked without discarding, then told to discard — never the other way.
    expect(engine.closeCalls).toEqual([false, true]);
  });
});

describe("which controls a layout shows", () => {
  it("gives a scatter its coverage and a grid its rows", async () => {
    const user = await openApp();
    expect(screen.getByTestId("coverage")).toBeTruthy();
    expect(screen.queryByTestId("grid-rows")).toBeNull();

    await user.selectOptions(screen.getByTestId("layout"), "brick");

    await screen.findByTestId("grid-rows");
    expect(screen.getByTestId("grid-cols")).toBeTruthy();
    expect(screen.queryByTestId("coverage")).toBeNull();
  });

  it("offers the mirror-line share only for a mirror", async () => {
    const user = await openApp();
    expect(screen.queryByTestId("on-the-lines")).toBeNull();

    await user.selectOptions(screen.getByTestId("layout"), "mirror");
    await screen.findByTestId("on-the-lines");

    // A mirror is a scatter, so coverage stays.
    expect(screen.getByTestId("coverage")).toBeTruthy();
    expect(screen.queryByTestId("grid-rows")).toBeNull();
  });
});

describe("what the engine reports, the interface says", () => {
  it("names a motif that has dissolved into the background", async () => {
    // 18% was the peeled banana against a six-colour palette: invisible on the
    // default ground and a hole the moment the ground changed.
    await openApp({ motifs: [makeMotif({ id: "peeled", ground_share: 0.18 })] });

    const warning = await screen.findByText(/18% of/);
    expect(within(warning).getByText("peeled")).toBeTruthy();
  });

  it("stays quiet when no motif has", async () => {
    await openApp({ motifs: [makeMotif({ ground_share: 0.02 })] });
    expect(screen.queryByText(/of the background colour/)).toBeNull();
  });

  it("says when a grid was drawn at a different size than asked", async () => {
    const user = await openApp({}, { grid_cells: [6, 4] });
    await user.selectOptions(screen.getByTestId("layout"), "diagonal");

    // Asked for 3 x 3; the engine reports it drew 6 x 4 so the diagonal closes.
    expect(await screen.findByText(/Drawn as 6×4/)).toBeTruthy();
  });

  it("says nothing when the grid is the size asked for", async () => {
    const user = await openApp({}, { grid_cells: [3, 3] });
    await user.selectOptions(screen.getByTestId("layout"), "square");
    await screen.findByTestId("grid-rows");
    expect(screen.queryByText(/Drawn as/)).toBeNull();
  });
});

describe("the palette size control", () => {
  it("asks the engine for the count the slider names", async () => {
    await openApp();
    // The slider steps through a list of counts, so its own value is an index.
    // Index 3 is five colours — this is the test that would catch the control
    // handing the engine the index instead of the count.
    fireEvent.change(screen.getByTestId("palette-size"), { target: { value: "3" } });

    await waitFor(() => expect(engine.paletteSizes).toEqual([5]));
  });

  it("names the colourways a resize discarded", async () => {
    engine.sized = {
      palette: ["#faf6ee", "#000000"],
      palette_colours: 2,
      colourways: [],
      active_colourway: null,
      colourways_dropped: ["Night", "Bleach"],
      motifs: [makeMotif()],
    };
    await openApp();
    fireEvent.change(screen.getByTestId("palette-size"), { target: { value: "0" } });

    const note = await screen.findByText(/Discarded colourways/);
    expect(note.textContent).toContain("Night, Bleach");
  });
});

describe("the preview", () => {
  it("shows what the engine drew", async () => {
    await openApp();
    const image = (await screen.findByAltText(/repeat of cherries/i)) as HTMLImageElement;
    expect(image.src).toContain(engine.preview.png_base64);
  });

  it("reports the coverage actually reached, not the one asked for", async () => {
    await openApp({ density: 0.5 }, { achieved_density: 0.31 });
    const readout = await screen.findByTestId("readout-coverage");
    expect(readout.textContent).toContain("31%");
    expect(readout.textContent).toContain("asked 50%");
  });
});

/** The ground the engine was last sent.
 *
 *  A wait rather than a read. Ground edits are coalesced — growing a texture
 *  is expensive enough that a drag would otherwise queue one simulation per
 *  step — so the send lands a moment after the control moves. A test that read
 *  it straight away would be asserting on the edit before last. */
async function sentGround(): Promise<GroundSpec> {
  const before = engine.grounds.length;
  await waitFor(() => expect(engine.grounds.length).toBeGreaterThan(before - 1));
  await waitFor(() => expect(engine.grounds.at(-1)).toBeTruthy());
  return engine.grounds.at(-1)!;
}

/** The panel's sections, in the order they are shown. */
function sections(): string[] {
  return Array.from(
    document.querySelectorAll(".controls section h2, .controls section h3"),
  ).map((h) => h.textContent ?? "");
}

describe("what the ingest wants to say", () => {
  it("shows what the engine noticed about the artwork", async () => {
    // These warnings existed all along and went to stderr from inside the
    // engine, where this app could not see them. The two they most often
    // carry — a motif dissolving into the fabric colour, a shadow kept as ink
    // — are the faults most likely to reach a printer unnoticed.
    await openApp({
      notes: [
        { about: "cherry", says: "18% of this motif became the ground colour." },
      ],
    });
    const shown = await screen.findByTestId("ingest-notes");
    expect(shown.textContent).toContain("cherry");
    expect(shown.textContent).toContain("became the ground colour");
  });

  it("says nothing when there is nothing to say", async () => {
    await openApp({ notes: [] });
    expect(screen.queryByTestId("ingest-notes")).toBeNull();
  });

  it("can be put away, because the artwork did come in", async () => {
    // A warning that cannot be dismissed is one that gets ignored.
    const user = await openApp({
      notes: [{ about: "cherry", says: "A shadow was kept as ink." }],
    });
    await user.click(screen.getByTestId("dismiss-notes"));
    await waitFor(() => expect(screen.queryByTestId("ingest-notes")).toBeNull());
  });
});

describe("cutting for a garment", () => {
  it("records the size when there is nothing to scale from", async () => {
    // A document that has never been told has no factor: there is no ratio
    // without two sizes, and resizing somebody's work on first contact with a
    // control would be the worst possible introduction to it.
    await openApp({ drawn_for: null });
    const menu = screen.getByTestId("cut-for") as HTMLSelectElement;
    expect(menu.value).toBe("");
    expect(screen.getByTestId("cut-for-hint").textContent).toContain(
      "does not resize anything yet",
    );
  });

  it("scales the repeat and the motifs together", async () => {
    // The spec's requirement and the failure it names: scaling the tile alone
    // leaves one enormous motif on a small garment.
    const user = await openApp({ drawn_for: "T-shirt, adult M" });
    await user.selectOptions(screen.getByTestId("cut-for"), "T-shirt, age 3–4");

    await waitFor(() => expect(engine.cuts).toEqual(["T-shirt, age 3–4"]));
    // The repeat the engine returned, not the one that was there.
    await waitFor(() =>
      expect((screen.getByTestId("tile-width") as HTMLInputElement).value).toBe("156"),
    );
    // And the motif size came back with it.
    await waitFor(() =>
      expect((screen.getByTestId("size-cherry") as HTMLInputElement).value).toBe("34"),
    );
  });

  it("says what these millimetres are for once it knows", async () => {
    await openApp({ drawn_for: "T-shirt, adult M" });
    const hint = screen.getByTestId("cut-for-hint").textContent ?? "";
    expect(hint).toContain("T-shirt, adult M");
    expect(hint).toContain("returns the design rather than drifting");
  });
});

describe("tracking", () => {
  it("says nothing about a scatter that does not track", async () => {
    // The warning has to be rare to be read. Most scatters are fine, and a
    // badge on all of them is a badge on none.
    await openApp({}, { alignment: 0.2, alignment_warns_above: 0.8 });
    expect((await screen.findByTestId("readout")).textContent).not.toContain(
      "Tracking",
    );
  });

  it("says a pattern is worth a look when its neighbours line up", async () => {
    // A ranking rather than a verdict, and the wording says so: this catches
    // the obvious cases and not the faint ones, so a low score is not a
    // guarantee and the readout must not imply one.
    await openApp({}, { alignment: 1.1, alignment_warns_above: 0.8 });
    const readout = await screen.findByTestId("readout");
    expect(readout.textContent).toContain("Tracking");
    expect(readout.textContent).toContain("worth a look");
  });

  it("takes the threshold from the engine rather than holding its own", async () => {
    // The number came out of a six-hundred-pattern calibration. An interface
    // with its own copy would drift from it the first time it was retuned.
    await openApp({}, { alignment: 1.1, alignment_warns_above: 2.0 });
    expect((await screen.findByTestId("readout")).textContent).not.toContain(
      "Tracking",
    );
  });
});

describe("the motif mix", () => {
  it("does not offer a share when there is only one motif", async () => {
    // A single motif is the whole mix whatever the slider says. It was
    // labelled "How often", sat beside Size, and was reported as doing
    // nothing — which was true, and could not have been otherwise.
    await openApp();
    expect(screen.getByTestId("size-cherry")).toBeTruthy();
    expect(screen.queryByTestId("share-cherry")).toBeNull();
  });

  it("shows a share as its share of the mix", async () => {
    // A raw weight of 6 means nothing on its own. A percentage that moves when
    // its neighbours move says what kind of control this is without a
    // sentence of explanation.
    await openApp({
      motifs: [
        makeMotif({ id: "cherry", weight: 1 }),
        makeMotif({ id: "lemon", weight: 3 }),
      ],
    });

    const row = (id: string) =>
      screen.getByTestId(`share-${id}`).closest("label")!.textContent ?? "";
    expect(row("cherry")).toContain("25%");
    expect(row("lemon")).toContain("75%");

    fireEvent.change(screen.getByTestId("share-cherry"), {
      target: { value: "3" },
    });
    await waitFor(() => expect(row("cherry")).toContain("50%"));
    // The neighbour's share moved without its slider being touched, which is
    // the whole point of showing it this way.
    expect(row("lemon")).toContain("50%");
  });

  it("says what does set the count, and it is not on this panel", async () => {
    // Reported from the app: a designer dropped the motif size and raised how
    // often it appeared, and the number on the fabric did not move. On a grid
    // neither of those can move it — the count is rows times columns.
    const user = await openApp();
    expect((await screen.findByTestId("what-sets-the-count")).textContent).toContain(
      "Coverage",
    );

    await user.selectOptions(screen.getByTestId("layout"), "square");

    await waitFor(() =>
      expect(screen.getByTestId("what-sets-the-count").textContent).toContain(
        "Rows × Columns",
      ),
    );
  });
});

describe("exporting", () => {
  /** Open the export dialogue and wait for it. */
  async function openExport() {
    const user = await openApp();
    await screen.findByTestId("preview-image");
    await user.click(screen.getByTestId("export"));
    await screen.findByTestId("modal");
    return user;
  }

  it("opens over the window rather than under the preview", async () => {
    // It was tucked below the image, where it pushed the preview out of view
    // and read as one more panel of the same kind as the controls. Exporting
    // is a decision with a dozen settings and an end — a file written.
    await openExport();
    const modal = screen.getByTestId("modal");
    expect(modal.getAttribute("aria-modal")).toBe("true");
    expect(modal.getAttribute("role")).toBe("dialog");
    // The controls are inside it, and the preview is still on screen rather
    // than pushed below a panel that grew under it.
    expect(modal.contains(screen.getByTestId("save-as"))).toBe(true);
    expect(modal.contains(screen.getByTestId("preview-image"))).toBe(false);
  });

  it("names a file that does not exist yet", async () => {
    // "Choose a file" describes picking one of the files there are. This one
    // names a new one, which is what every platform calls Save as.
    await openExport();
    expect(screen.getByTestId("save-as").textContent).toContain("Save as");
  });

  it("writes the file the save dialogue named, and then closes", async () => {
    // The whole point of the button. It offers the plan's suggested name,
    // which is where the format and resolution end up in the filename.
    const user = await openExport();
    dialog.chosen = "/prints/cherries-tile-300dpi.tif";

    await user.click(screen.getByTestId("save-as"));

    await waitFor(() =>
      expect(engine.exported).toEqual(["/prints/cherries-tile-300dpi.tif"]),
    );
    await waitFor(() => expect(screen.queryByTestId("modal")).toBeNull());
  });

  it("writes nothing if the save dialogue is dismissed", async () => {
    const user = await openExport();
    dialog.chosen = null;

    await user.click(screen.getByTestId("save-as"));

    await waitFor(() => expect(screen.getByTestId("modal")).toBeTruthy());
    expect(engine.exported).toEqual([]);
  });

  it("closes on Escape, on Cancel and on the close button", async () => {
    for (const shut of [
      async (user: ReturnType<typeof userEvent.setup>) =>
        user.keyboard("{Escape}"),
      async (user: ReturnType<typeof userEvent.setup>) =>
        user.click(screen.getByText("Cancel")),
      async (user: ReturnType<typeof userEvent.setup>) =>
        user.click(screen.getByTestId("modal-close")),
    ]) {
      const user = await openExport();
      await shut(user);
      await waitFor(() => expect(screen.queryByTestId("modal")).toBeNull());
      cleanup();
    }
  });

  it("takes focus when it opens and gives it back when it closes", async () => {
    // A dialogue that leaves focus behind it is unreachable from the keyboard,
    // and one that drops focus to the top of the document on close loses a
    // keyboard user their place.
    const user = await openExport();
    expect(screen.getByTestId("modal").contains(document.activeElement)).toBe(true);

    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByTestId("export")),
    );
  });

  it("keeps tabbing inside itself", async () => {
    // The controls behind the backdrop are visible and unreachable by mouse.
    // Leaving them in the tab order is the worst of the three states.
    const user = await openExport();
    const modal = screen.getByTestId("modal");
    for (let i = 0; i < 24; i++) {
      await user.tab();
      expect(modal.contains(document.activeElement)).toBe(true);
    }
  });
});

describe("a ground drawn by rule", () => {
  /** Switch the ground panel to the ornament editor and wait for it. */
  async function openOrnament() {
    const user = await openApp();
    await user.selectOptions(screen.getByTestId("ground-kind"), "ornament");
    await screen.findByTestId("ornament-figure");
    await waitFor(() => expect(engine.grounds.at(-1)?.kind).toBe("ornament"));
    return user;
  }

  it("offers the ornament editor instead of the other two", async () => {
    await openOrnament();
    expect(screen.queryByTestId("ground-preset")).toBeNull();
    expect(screen.queryByTestId("texture-growth")).toBeNull();
    const sent = await sentGround();
    expect(sent.kind).toBe("ornament");
    expect(sent.bands).toEqual([]);
  });

  it("sends the whole ornament when one control changes", async () => {
    const user = await openOrnament();
    await user.selectOptions(screen.getByTestId("ornament-figure"), "carpet");

    await waitFor(() => {
      const last = engine.grounds.at(-1);
      expect(last?.ornament.figure).toBe("carpet");
      expect(last?.ornament.cell_mm).toBe(DEFAULT_ORNAMENT.cell_mm);
      expect(last?.ornament.levels.length).toBeGreaterThan(0);
    });
  });

  it("shows only the controls the chosen figure answers to", async () => {
    // Three of the six figures ignore depth and three ignore a line width.
    // A control that is shown and ignored is worse than one that is absent:
    // it gets moved, nothing happens, and the panel stops being believed.
    const user = await openOrnament();
    expect(screen.getByTestId("ornament-stroke")).toBeTruthy();
    expect(screen.queryByTestId("ornament-depth")).toBeNull();

    await user.selectOptions(screen.getByTestId("ornament-figure"), "carpet");

    await screen.findByTestId("ornament-depth");
    expect(screen.queryByTestId("ornament-stroke")).toBeNull();
  });

  it("says what grid it drew, not the cell size that was asked for", async () => {
    // The grid has to close over the repeat, so the request is a wish.
    await openOrnament();
    engine.fittedGrid = { cols: 9, rows: 8, cell_w_mm: 26.7, cell_h_mm: 22.5 };

    fireEvent.change(screen.getByTestId("ornament-cell"), {
      target: { value: "26" },
    });

    await waitFor(() => expect(engine.grounds.at(-1)?.ornament.cell_mm).toBe(26));
    const note = await screen.findByTestId("ornament-grid");
    expect(note.textContent).toContain("9 x 8");
    expect(note.textContent).toContain("26.7");
  });

  it("will not offer another drawing of a strict figure", async () => {
    // `variation` is what the seed is for. At zero the figure is the classical
    // one and there is no other drawing of it to ask for.
    const user = await openOrnament();
    const reseed = screen.getByTestId("ornament-reseed") as HTMLButtonElement;
    expect(reseed.disabled).toBe(true);

    fireEvent.change(screen.getByTestId("ornament-variation"), {
      target: { value: "0.3" },
    });
    await waitFor(() =>
      expect(
        (screen.getByTestId("ornament-reseed") as HTMLButtonElement).disabled,
      ).toBe(false),
    );

    const before = engine.grounds.at(-1)!.ornament;
    await user.click(screen.getByTestId("ornament-reseed"));
    await waitFor(() => {
      const after = engine.grounds.at(-1)!.ornament;
      expect(after.seed).not.toBe(before.seed);
      expect({ ...after, seed: 0 }).toEqual({ ...before, seed: 0 });
    });
  });

  it("goes back to bands without carrying the ornament into them", async () => {
    const user = await openOrnament();
    await user.selectOptions(screen.getByTestId("ground-kind"), "bands");
    await screen.findByTestId("ground-preset");
    await waitFor(() => expect(engine.grounds.at(-1)?.kind).toBe("bands"));
    expect(screen.queryByTestId("ornament-figure")).toBeNull();
  });
});

describe("a grown ground", () => {
  /** Switch the ground panel to the texture editor and wait for it. */
  async function openTexture() {
    const user = await openApp();
    await user.selectOptions(screen.getByTestId("ground-kind"), "texture");
    await screen.findByTestId("texture-growth");
    await waitFor(() => expect(engine.grounds.at(-1)?.kind).toBe("texture"));
    return user;
  }

  it("offers the texture editor instead of the band sequence", async () => {
    // The two are alternatives, not layers: a pattern has one ground. Showing
    // both editors would suggest otherwise.
    const user = await openTexture();
    expect(screen.queryByTestId("ground-preset")).toBeNull();
    expect(screen.queryByTestId("ground-sequence")).toBeNull();
    const sent = await sentGround();
    expect(sent.kind).toBe("texture");
    expect(sent.bands).toEqual([]);
    expect(user).toBeTruthy();
  });

  it("sends the whole texture when one control changes", async () => {
    // Same rebuild-from-scratch discipline as the band sequence: a change that
    // sent only its own field would drop the rest of the parameters.
    const user = await openTexture();
    await user.selectOptions(screen.getByTestId("texture-growth"), "maze");

    await waitFor(() => {
      const last = engine.grounds.at(-1);
      expect(last?.texture.growth).toBe("maze");
      expect(last?.texture.cells).toBe(DEFAULT_TEXTURE.cells);
      expect(last?.texture.levels.length).toBe(2);
    });

    fireEvent.change(screen.getByTestId("texture-cells"), {
      target: { value: "96" },
    });
    await waitFor(() => {
      const last = engine.grounds.at(-1);
      expect(last?.texture.cells).toBe(96);
      expect(last?.texture.growth).toBe("maze");
    });
  });

  it("grows another of the same by moving the seed, not the parameters", async () => {
    // The control a designer reaches for when the look is right and the
    // arrangement is not. If it changed anything else it would not be that.
    const user = await openTexture();
    const before = engine.grounds.at(-1)!.texture;

    await user.click(screen.getByTestId("texture-reseed"));

    await waitFor(() => {
      const after = engine.grounds.at(-1)!.texture;
      expect(after.seed).not.toBe(before.seed);
      expect({ ...after, seed: 0 }).toEqual({ ...before, seed: 0 });
    });
  });

  it("turns a level between ink and fabric", async () => {
    // How the same growth gives light-on-dark as well as dark-on-light. The
    // fabric is the absence of a colour, not a colour that matches it.
    const user = await openTexture();
    expect((await sentGround()).texture.levels[0]).toBeNull();

    await user.click(screen.getByTestId("level-fabric-0"));
    await waitFor(() =>
      expect(engine.grounds.at(-1)?.texture.levels[0]).not.toBeNull(),
    );

    await user.click(screen.getByTestId("level-fabric-1"));
    await waitFor(() =>
      expect(engine.grounds.at(-1)?.texture.levels[1]).toBeNull(),
    );
  });

  it("adds tones and reports what each one covered", async () => {
    // The share comes from the field the engine grew, not from the request:
    // a level can fall between two concentrations the field never reaches.
    const user = await openTexture();
    engine.levelShares = [0.6, 0.39, 0.01];

    await user.click(screen.getByTestId("texture-add-level"));

    await waitFor(() => expect(engine.grounds.at(-1)?.texture.levels.length).toBe(3));
    const rows = await screen.findByTestId("texture-levels");
    expect(rows.textContent).toContain("60%");
    expect(rows.textContent).toContain("1%");
  });

  it("goes back to bands without carrying the texture into them", async () => {
    const user = await openTexture();
    await user.selectOptions(screen.getByTestId("ground-kind"), "bands");
    await screen.findByTestId("ground-preset");
    await waitFor(() => expect(engine.grounds.at(-1)?.kind).toBe("bands"));
    expect(screen.queryByTestId("texture-growth")).toBeNull();
  });
});

describe("the ground", () => {
  it("fills the sequence from a preset and shows it row by row", async () => {
    const user = await openApp();
    expect(screen.queryByTestId("ground-sequence")).toBeNull();

    await user.selectOptions(screen.getByTestId("ground-preset"), "check");

    await screen.findByTestId("ground-sequence");
    // Two rows: the ink and the gap.
    expect(screen.getByTestId("band-width-0")).toBeTruthy();
    expect(screen.getByTestId("band-width-1")).toBeTruthy();
    expect(screen.queryByTestId("band-width-2")).toBeNull();
    expect((await sentGround()).direction).toBe("both");
  });

  it("sends the whole sequence when one row changes", async () => {
    // The ground is rebuilt from the sequence each time, so a row edit that
    // sent only its own row would drop every other band.
    const user = await openApp();
    await user.selectOptions(screen.getByTestId("ground-preset"), "check");
    await screen.findByTestId("band-width-0");

    fireEvent.change(screen.getByTestId("band-width-0"), { target: { value: "5" } });

    await waitFor(() => {
      const last = engine.grounds.at(-1);
      expect(last?.bands.length).toBe(2);
      expect(last?.bands[0].width_mm).toBe(5);
      // The row that was not touched keeps its own width and colour. A check
      // is two threads rather than a thread and a gap, so there is a second
      // colour here to lose.
      expect(last?.bands[1].width_mm).toBe(16);
      expect(last?.bands[1].colour).toBe("#f2f0ea");
    });
  });

  it("adds and removes bands, which is what makes a tartan reachable", async () => {
    const user = await openApp();
    await user.selectOptions(screen.getByTestId("ground-preset"), "check");
    await screen.findByTestId("band-add");

    await user.click(screen.getByTestId("band-add"));
    await waitFor(() => expect(engine.grounds.at(-1)?.bands.length).toBe(3));
    await screen.findByTestId("band-width-2");

    await user.click(screen.getByTestId("band-remove-0"));
    await waitFor(() => expect(engine.grounds.at(-1)?.bands.length).toBe(2));
  });

  it("turns a band into a gap and back", async () => {
    // A gap is the absence of a colour rather than a colour matching the
    // ground, so that recolouring the fabric takes the gaps with it.
    const user = await openApp();
    await user.selectOptions(screen.getByTestId("ground-preset"), "check");
    await screen.findByTestId("band-gap-0");

    await user.click(screen.getByTestId("band-gap-0"));
    await waitFor(() => expect(engine.grounds.at(-1)?.bands[0].colour).toBeNull());

    await user.click(screen.getByTestId("band-gap-0"));
    await waitFor(() =>
      expect(engine.grounds.at(-1)?.bands[0].colour).not.toBeNull(),
    );
  });

  it("brings a tartan in mirrored, with its crossings overprinted", async () => {
    const user = await openApp();
    await user.selectOptions(screen.getByTestId("ground-preset"), "tartan");

    await screen.findByTestId("ground-sequence");
    const spec = await sentGround();
    expect(spec?.bands.length).toBe(4);
    expect(spec?.mirrored).toBe(true);
    expect(spec?.crossings).toBe("blend");
    expect(spec?.direction).toBe("both");
  });

  it("asks to mirror once, and never asks again", async () => {
    // Reflecting writes the reflected bands into the list rather than
    // remembering a mode. If the request kept its flag, the next edit would
    // reflect the already-reflected sequence.
    const user = await openApp();
    await user.selectOptions(screen.getByTestId("ground-preset"), "tartan");
    await screen.findByTestId("ground-sequence");

    fireEvent.change(screen.getByTestId("band-width-0"), { target: { value: "9" } });

    await waitFor(() => expect(engine.grounds.at(-1)?.mirrored).toBe(false));
  });

  it("will not mirror a sequence too short to have two pivots", async () => {
    const user = await openApp();
    await user.selectOptions(screen.getByTestId("ground-preset"), "check");
    await screen.findByTestId("ground-sequence");
    // Two bands reflected about both of them is the two bands.
    expect((screen.getByTestId("ground-mirror") as HTMLButtonElement).disabled).toBe(true);
  });

  it("asks about crossings only where bands actually cross", async () => {
    const user = await openApp();
    await user.selectOptions(screen.getByTestId("ground-preset"), "stripe");
    await screen.findByTestId("ground-sequence");

    // A stripe runs one way, so nothing crosses and there is nothing to ask.
    expect(screen.queryByTestId("ground-crossings")).toBeNull();

    await user.selectOptions(screen.getByTestId("ground-direction"), "both");
    expect(await screen.findByTestId("ground-crossings")).toBeTruthy();
  });

  it("shows the preset that was chosen, not Custom", async () => {
    // Reported from the app: picking Pinstripe immediately showed Custom, so
    // an intentional choice looked as though it had not taken. Every preset
    // but Plain did it, because the menu read "has any bands" as "has been
    // edited".
    const user = await openApp();
    const menu = () => screen.getByTestId("ground-preset") as HTMLSelectElement;

    for (const id of ["stripe", "pinstripe", "check", "windowpane", "tartan"]) {
      await user.selectOptions(menu(), id);
      await waitFor(() => expect(menu().value).toBe(id));
    }
  });

  it("says Custom only once the sequence has been edited away", async () => {
    const user = await openApp();
    const menu = () => screen.getByTestId("ground-preset") as HTMLSelectElement;
    await user.selectOptions(menu(), "check");
    await waitFor(() => expect(menu().value).toBe("check"));
    // Not offered while the sequence still is a preset.
    expect(
      Array.from(menu().options).some((o) => o.value === ""),
    ).toBe(false);

    fireEvent.change(screen.getByTestId("band-width-0"), { target: { value: "5" } });

    await waitFor(() => expect(menu().value).toBe(""));
    expect(menu().selectedOptions[0].textContent).toBe("Custom");
  });

  it("does not lose a reflection to a quick second edit", async () => {
    // Texture edits are coalesced because growing one is expensive. Bands are
    // not, and this is why: `mirrored` is a one-shot instruction answered by
    // returning the reflected sequence, so coalescing a reflect with the edit
    // that followed would cancel the pending send and the reflection would
    // never reach the engine at all.
    const user = await openApp();
    await user.selectOptions(screen.getByTestId("ground-preset"), "tartan");
    fireEvent.change(screen.getByTestId("band-width-0"), { target: { value: "9" } });

    await waitFor(() =>
      expect(engine.grounds.some((g) => g.mirrored)).toBe(true),
    );
  });

  it("stops offering the repeats a ground cannot take", async () => {
    // A ground repeats with the tile, so it meets itself only where the
    // lattice steps by whole tiles. The engine refuses the others; the control
    // used to offer them anyway and let a designer find out from an error.
    const user = await openApp();
    const type = () => screen.getByTestId("repeat-type") as HTMLSelectElement;
    const option = (value: string) =>
      Array.from(type().options).find((o) => o.value === value)!;

    expect(option("half-drop").disabled).toBe(false);
    expect(screen.queryByTestId("ground-needs-block")).toBeNull();

    await user.selectOptions(screen.getByTestId("ground-preset"), "check");
    await screen.findByTestId("ground-needs-block");

    expect(option("half-drop").disabled).toBe(true);
    expect(option("brick").disabled).toBe(true);
    expect(option("quarter-drop").disabled).toBe(true);
    expect(option("block").disabled).toBe(false);
  });

  it("says how many colours the ground costs", async () => {
    const user = await openApp();
    engine.groundSlots = [3, 4, 5, 6, 7, 8];
    await user.selectOptions(screen.getByTestId("ground-preset"), "tartan");

    const note = await screen.findByTestId("ground-colours");
    expect(note.textContent).toContain("6 colours");
  });

  it("puts the ground's own colours beside it, not among the motif colours", async () => {
    const user = await openApp();
    const colours = (el: HTMLElement) =>
      el.querySelectorAll('input[type="color"]').length;
    expect(colours(screen.getByTestId("motif-swatches"))).toBe(2);

    await user.selectOptions(screen.getByTestId("ground-preset"), "check");
    await screen.findByTestId("ground-sequence");

    // The engine reported slots 3 and 4 for the bands, and they stay out of
    // the motif swatches where nobody would connect them to the stripes.
    await waitFor(() =>
      expect(colours(screen.getByTestId("motif-swatches"))).toBe(2),
    );
  });

  it("takes the ground away again", async () => {
    const user = await openApp();
    await user.selectOptions(screen.getByTestId("ground-preset"), "check");
    await screen.findByTestId("ground-sequence");

    await user.selectOptions(screen.getByTestId("ground-preset"), "none");

    await waitFor(() => expect(screen.queryByTestId("ground-sequence")).toBeNull());
    await waitFor(() => expect(engine.grounds.at(-1)?.bands).toEqual([]));
  });
});
