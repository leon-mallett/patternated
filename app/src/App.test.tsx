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
import type { GroundSpec, Opened, PaletteSized, Preview } from "./api";

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
};

vi.mock("./api", async (importOriginal) => {
  const real = await importOriginal<typeof import("./api")>();
  return {
    ...real,
    openDocument: async () => engine.opened,
    newFromImages: async () => engine.opened,
    apply: async (params: unknown) => {
      engine.applied.push(params);
      return engine.preview;
    },
    garments: async () => [],
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
      return {
        ...spec,
        style: painted ? "check" : "none",
        width_mm: spec.bands[0]?.width_mm ?? 18,
        gap_mm: spec.bands[1]?.width_mm ?? 18,
        slots: painted ? engine.groundSlots : [],
        ground_colours: painted ? engine.groundSlots.length : 0,
        palette: painted
          ? [...engine.opened.palette, "#9a8f7e", "#6b6154"]
          : engine.opened.palette,
        lattice: "block",
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
  dialog.confirmed = true;
  dialog.chosen = "/patterns/cherries.json";
});

afterEach(cleanup);

describe("the start screen", () => {
  it("offers both ways in", () => {
    // The defect this guards was reported from the running app: the shell
    // could open a document and appeared to have no way to create one.
    render(<App />);
    expect(screen.getByText("New pattern from artwork…")).toBeTruthy();
    expect(screen.getByText("Open a saved pattern…")).toBeTruthy();
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
    expect(engine.grounds.at(-1)?.direction).toBe("both");
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
    const spec = engine.grounds.at(-1);
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
    expect(engine.grounds.at(-1)?.bands).toEqual([]);
  });
});
