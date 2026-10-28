<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import {
    report,
    CLASSES,
    describeMultiplet,
    letterOf,
    nucleiOf,
    placesFor,
    sharedCouplings,
    type SharedCoupling,
  } from './multiplets';
  import { findSolvent } from './solvents';
  import { isSolvent } from './impurities';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { onMount, tick } from 'svelte';
  import {
    addIntegral,
    mergePeaks,
    removeIntegral,
    removeMultiplet,
    removePeak,
    setIntegral,
    setPeakLabel,
    shiftAnalysis,
    addMultiplet,
    setMultipletKind,
    setMultipletJs,
    parseJs,
    setMultipletName,
    setMultipletNuclei,
    toggleAssignment,
    assignCrossPeak,
    clearAssignments,
    classOf,
    setMultipletWidth,
    pairedIntegral,
    pairedMultiplet,
    setMultipletRange,
    setMultipletShape,
    setMultipletShift,
    couplingsBetween,
    mergeMultiplets,
    splitMultiplet,
  } from './analysis';
  import AnalysisPanel from './AnalysisPanel.svelte';
  import { peakFitSummary } from './quantification';
  import { integralsCsv, peaksCsv } from './csv';
  import Palette from './Palette.svelte';
  import FontChoice from './FontChoice.svelte';
  import MoleculeView from './MoleculeView.svelte';
  import { ACS_1996, type StructureStyle } from './structure';
  import { looksLikeMolfile, MOLFILE_TYPE, molfileText, molfileUrl } from './molecule';
  import { remember as keepSetting, remembered } from './settings';
  import PageSetupEditor from './PageSetupEditor.svelte';
  import {
    PAGE_SETUP,
    PLOT_TEXT,
    pixelsPerMm,
    plotSize,
    textSizesOf,
    type PageSetup,
  } from './page';
  import type { PaletteCommand } from './palette';
  import {
    bundle,
    isProjectFile,
    parseProject,
    PROJECT_EXTENSIONS,
    shortPaths,
    structureFile,
    toProject,
    withExtension,
    type Project,
  } from './project';
  import {
    autoPhase,
    autoPeaks,
    fitPeaks,
    findPeaks,
    fitMultiplet,
    largestPeak,
    nearestPeak,
    closeDocument,
    fetchMetadata,
    nearestPeak2d,
    redo2d,
    setPeaks2d,
    undo2d,
    openPath,
    projectLayout,
    readProject,
    redo as redoChange,
    saveFile,
    setAnalysis,
    setOperations,
    setState,
    undo as undoChange,
    type Analysis,
    type CrossPeak,
    type Document2DInfo,
    type DocumentInfo,
    type Operation,
    type PhaseParams,
    signalRegions,
    readBytes,
    clipboardStructure,
    readMolecule,
    type Molecule,
    writeProject,
    savePdf,
    type Attachment,
    keepSession,
    lastSession,
  } from './api';
  import { isEditable, isMac, windowOpen } from './keys';
  import {
    DEFAULT_WHITTAKER,
    findPhase,
    isBaseline,
    withBaseline,
    withPhase,
    withReference,
    type BaselineOperation,
  } from './operations';
  import Plot from './Plot.svelte';
  import Plot2D from './Plot2D.svelte';
  import { SHORTCUTS } from './shortcuts';
  import Menu, { type MenuItem } from './Menu.svelte';
  import {
    Copy,
    RotateCcw as Reset,
    ClipboardPaste,
    Columns2,
    ImagePlus,
    Rows3,
    X,
    Activity,
    Waypoints,
    Brackets,
    ChartSpline,
    ChartNoAxesColumn,
    PanelsLeftBottom,
    ArrowLeft,
    Pipette,
    Crop,
    Replace,
    ChevronLeft,
    ChevronRight,
    ClipboardCopy,
    Crosshair,
    Eraser,
    Eye,
    Hand,
    Files,
    FileText,
    Layers,
    Link,
    Merge,
    Sparkles,
    ListChecks,
    Magnet,
    Maximize2,
    Minus,
    MousePointer2,
    MousePointerClick,
    Move,
    MoveVertical,
    Plus,
    RotateCcw,
    Ruler,
    Scale,
    ScanLine,
    Scissors,
    Sigma,
    SlidersHorizontal,
    Tag,
    Target,
    WandSparkles,
    ZoomIn,
    ZoomOut,
    ChevronDown,
    Ellipsis,
    Download,
    FolderOpen,
    PanelLeft,
    PanelRight,
    Redo2,
    Save,
    Search,
    Undo2,
  } from '@lucide/svelte';
  import { AXIS_LABEL, pressPan, pressZoom, toolHint, type Tool } from './tools';
  import { colorAt, DEFAULT_STACK_ANGLE, fixed, nucleusLabel, type Moved } from './traces';
  import { clamp, type Axis } from './view';
  import Processing from './Processing.svelte';
  import {
    lastPpm,
    NO_ADJUSTMENT,
    PALETTE,
    STACK_MODES,
    type Adjustment,
    type StackMode,
  } from './traces';
  import {
    BUILT_IN,
    CUSTOM,
    CUSTOM_COLOURS,
    contrastWarnings,
    DEFAULT_CUSTOM,
    isTheme,
    PALETTES,
    type ThemeName,
  } from './palettes';

  const RECENT_KEY = 'recent';
  const THEME_KEY = 'theme';
  const FONT_KEY = 'font';
  /** The interface can also follow the system's own typeface. */
  const SYSTEM_FONT = 'System';

  const RECENT_MAX = 10;
  const NOTICE_MS = 6000;
  const commandKey = isMac ? '⌘' : 'Ctrl+';
  /** Observed noise factor; the noise estimates can differ. */
  const DEFAULT_NOISE_FACTOR = 10;

  let docs = $state<DocumentInfo[]>([]);
  /** 2D experiments, kept apart from the spectra so that nothing made for a
   *  spectrum - stacks, analysis, processing - is ever handed one. */
  let docs2d = $state<Document2DInfo[]>([]);
  let activeId = $state<number>();
  let errors = $state<string[]>([]);
  /** A second failure joins the first rather than replacing it, so a batch that
   *  fails in several places says so; the same message twice is one line. */
  function fail(message: string) {
    if (!errors.includes(message)) errors = [...errors, message];
  }
  const SHOWN_ERRORS = 3;
  let listOpen = $state(true);
  let panelOpen = $state(true);
  /** The side panel shows one thing at a time rather than everything at once. */
  let tab = $state<
    'processing' | 'peaks' | 'integrals' | 'multiplets' | 'view' | 'metadata' | 'stack'
  >('processing');
  /** The plot laid out on a page of a fixed size, as it will be printed: the
   *  same plot, at the size of the page, so the screen and the file agree. */
  const PAGE_KEY = 'opennmr.page';
  const STRUCTURE_KEY = 'opennmr.structure';
  /** How the structure is drawn: ACS 1996 unless the fields were changed. */
  let structureStyle = $state<StructureStyle>(remembered(STRUCTURE_KEY, ACS_1996));
  $effect(() => keepSetting(STRUCTURE_KEY, structureStyle));
  const acsStyle = $derived(
    (Object.keys(ACS_1996) as (keyof StructureStyle)[]).every(
      (key) => structureStyle[key] === ACS_1996[key],
    ),
  );
  let pageSetup = $state<PageSetup>(remembered(PAGE_KEY, PAGE_SETUP));
  const PLOT_TEXT_KEY = 'opennmr.plotText';
  /** How the plot letters its peaks, integrals and multiplets on screen, as the
   *  page does in the figure; an empty colour follows the spectrum. */
  let plotText = $state(
    remembered(PLOT_TEXT_KEY, {
      peakSize: PLOT_TEXT.peaks,
      integralSize: PLOT_TEXT.integrals,
      multipletSize: PLOT_TEXT.multiplets,
      peakColour: '',
      integralColour: '',
      multipletColour: '',
    }),
  );
  $effect(() => keepSetting(PLOT_TEXT_KEY, plotText));
  const screenSizes = $derived({
    peaks: plotText.peakSize,
    integrals: plotText.integralSize,
    multiplets: plotText.multipletSize,
  });
  const screenInk = $derived(
    Object.fromEntries(
      [
        ['peaks', plotText.peakColour],
        ['integrals', plotText.integralColour],
        ['multiplets', plotText.multipletColour],
      ].filter(([, colour]) => colour !== ''),
    ),
  );
  let pageMode = $state(false);
  /** Multiplet names on the plot with no room of their own, which the figure says. */
  let crowdedNames = $state(0);
  $effect(() => keepSetting(PAGE_KEY, pageSetup));
  /** The room the page is shown in, and how much it is enlarged to fill it. */
  let deskWidth = $state(0);
  let deskHeight = $state(0);
  const pageScale = $derived.by(() => {
    const perMm = pixelsPerMm(pageSetup);
    const [w, h] = [pageSetup.width * perMm, pageSetup.height * perMm];
    return Math.max(0.05, Math.min((deskWidth - 48) / w, (deskHeight - 48) / h));
  });
  /** The page's colours and type, which the plot draws with in place of the
   *  theme's: a page is printed as it is set, whatever the screen is. */
  const pageInk = $derived({
    fg: pageSetup.peakColour,
    accent: pageSetup.peakColour,
    muted: pageSetup.axisColour,
    integral: pageSetup.integralColour,
    integrals: pageSetup.integralColour,
    bg: pageSetup.background,
    axis: pageSetup.axisColour,
    peaks: pageSetup.peakColour,
    multiplets: pageSetup.multipletColour,
    names: pageSetup.nameColour,
    font: `'${pageSetup.font}', Helvetica, Arial, sans-serif`,
  });
  const META_KEY = 'opennmr.metadata';
  /** Which parameter rows are worth keeping, by label. None means show them all,
   * since a dataset carries several hundred and most are housekeeping. */
  let metaKeys = $state<string[]>(
    (() => {
      try {
        const value: unknown = JSON.parse(localStorage.getItem(META_KEY) ?? '[]');
        return Array.isArray(value) ? value.filter((v) => typeof v === 'string') : [];
      } catch {
        return [];
      }
    })(),
  );
  let choosingMeta = $state(false);

  /** Drags a width: either column, or the line inside the metadata table. The
   * panel grows as the pointer goes left, which is what `sign` is for. Saved when
   * the drag ends rather than at every pixel. */
  function dragWidth(
    e: PointerEvent,
    from: number,
    apply: (px: number) => void,
    sign: 1 | -1,
    save: () => void,
  ) {
    const grip = e.currentTarget as HTMLElement;
    const start = e.clientX;
    const move = (m: PointerEvent) => apply(from + sign * (m.clientX - start));
    const up = () => {
      grip.removeEventListener('pointermove', move);
      grip.removeEventListener('pointerup', up);
      save();
    };
    grip.setPointerCapture(e.pointerId);
    grip.addEventListener('pointermove', move);
    grip.addEventListener('pointerup', up);
    e.preventDefault();
  }

  const COLUMNS_KEY = 'opennmr.columns';
  const savedColumns = remembered(COLUMNS_KEY, { list: 220, panel: 360 });
  let listWidth = $state(clamp(savedColumns.list, 160, 420));
  let panelWidth = $state(clamp(savedColumns.panel, 220, 560));
  const setListWidth = (px: number) => (listWidth = clamp(px, 160, 420));
  const setPanelWidth = (px: number) => (panelWidth = clamp(px, 220, 560));
  const saveColumns = () => keepSetting(COLUMNS_KEY, { list: listWidth, panel: panelWidth });

  function nudgeColumn(apply: (px: number) => void, from: number, by: number) {
    apply(from + by);
    saveColumns();
  }

  const OPENING_KEY = 'opennmr.opening';
  /** What is done to a spectrum as it is opened. The phase the spectrometer
   * recorded is applied either way; this asks for one to be worked out when the
   * dataset carries none. */
  const openedWith = remembered(OPENING_KEY, { phase: true, baseline: true });
  let phaseOnOpen = $state(openedWith.phase);
  let baselineOnOpen = $state(openedWith.baseline);
  $effect(() => keepSetting(OPENING_KEY, { phase: phaseOnOpen, baseline: baselineOnOpen }));

  /** The baseline correction a spectrum opens with and Auto baseline applies:
   *  the Whittaker smoother, with its two parameters as the settings keep them. */
  const BASELINE_KEY = 'opennmr.baseline';
  let baselineDefault = $state(
    remembered(BASELINE_KEY, {
      smoothness: DEFAULT_WHITTAKER.op === 'baseline_whittaker' ? DEFAULT_WHITTAKER.lambda : 1e5,
      filterHz: DEFAULT_WHITTAKER.op === 'baseline_whittaker' ? DEFAULT_WHITTAKER.filter_hz : 7.3,
      autoFilter: true,
    }),
  );
  $effect(() => keepSetting(BASELINE_KEY, baselineDefault));
  const defaultBaseline = (): BaselineOperation => ({
    op: 'baseline_whittaker',
    lambda: baselineDefault.smoothness,
    filter_hz: baselineDefault.filterHz,
    auto_filter: baselineDefault.autoFilter,
  });

  const DECIMALS_KEY = 'opennmr.decimals';
  const MULTIPLET_LABEL_KEY = 'opennmr.multipletLabel';
  /** What a multiplet is called on the plot: its shift, the name in the table, or both. */
  let multipletLabel = $state<'shift' | 'name' | 'both'>(
    (['shift', 'name', 'both'] as const).find(
      (v) => v === localStorage.getItem(MULTIPLET_LABEL_KEY),
    ) ?? 'shift',
  );
  $effect(() => localStorage.setItem(MULTIPLET_LABEL_KEY, multipletLabel));
  /** Decimal places for every shift in ppm, so the tables and the plot agree. */
  let decimals = $state(clamp(Number(localStorage.getItem(DECIMALS_KEY)) || 2, 0, 4));
  $effect(() => localStorage.setItem(DECIMALS_KEY, String(decimals)));

  const META_WIDTH_KEY = 'opennmr.metadata.width';
  /** How much of the table the parameter names take, since some are long and a
   * spectrometer's shorthand is not. */
  let metaLabelWidth = $state<number>(
    (() => {
      const saved = Number(localStorage.getItem(META_WIDTH_KEY));
      return Number.isFinite(saved) && saved > 0 ? saved : 120;
    })(),
  );

  /** Drags the line between the two columns, within what the panel can hold. */
  const setMetaWidth = (px: number) => (metaLabelWidth = clamp(px, 60, 220));
  const saveMetaWidth = () => localStorage.setItem(META_WIDTH_KEY, String(metaLabelWidth));

  function nudgeMetaWidth(by: number) {
    setMetaWidth(metaLabelWidth + by);
    saveMetaWidth();
  }

  function toggleMeta(label: string) {
    metaKeys = metaKeys.includes(label)
      ? metaKeys.filter((k) => k !== label)
      : [...metaKeys, label];
    localStorage.setItem(META_KEY, JSON.stringify(metaKeys));
  }

  function showAllMeta() {
    metaKeys = [];
    choosingMeta = false;
    localStorage.setItem(META_KEY, '[]');
  }

  /** How the active spectrum was recorded, read when the tab asks for it. */
  let recorded = $state<[string, string][]>([]);
  const shownMeta = $derived(
    choosingMeta || metaKeys.length === 0
      ? recorded
      : recorded.filter(([label]) => metaKeys.includes(label)),
  );
  let dropping = $state(false);
  /** Manual offsets and scaling per spectrum, for lining a series up. */
  let display = $state<Record<number, Adjustment>>({});
  /** How the plot was left: what it shows and how picking behaves, kept between
   *  sessions so each start is where the last one ended. */
  const PLOT_KEY = 'opennmr.plot';
  const plotSettings = remembered(PLOT_KEY, {
    showLabels: true,
    showCurves: true,
    showIntegrals: true,
    showMultiplets: true,
    showSimulation: false,
    showAll: true,
    snap: true,
    noiseFactor: DEFAULT_NOISE_FACTOR,
    stackAngle: DEFAULT_STACK_ANGLE,
    autoScale: false,
    clipped: false,
    baselineGap: 5,
  });
  // The stack's own controls, kept beside the plot.
  let stackAngle = $state(plotSettings.stackAngle);
  let autoScale = $state(plotSettings.autoScale);
  let adjusting = $state(false);
  let clipped = $state(plotSettings.clipped);
  /** Label the peaks of every spectrum of a stack, not only the active one. */
  let showAll = $state(plotSettings.showAll);
  /** Colours chosen by hand, by document id; the palette decides the rest. */
  let colors = $state<Record<number, string>>({});
  /** Which colour of the palette each spectrum holds. Given out when a spectrum
   * first appears and kept while it is open, so hiding or closing another one
   * never repaints it. */
  let slots = $state<Record<number, number>>({});
  /** Saved arrangements of spectra, as stacked and superimposed pages are: a
   * view holds ids, so editing a spectrum reaches every view of it. */
  let views = $state<{ id: number; name: string; ids: number[]; stack: StackMode }[]>([]);
  /** What the plot shows: one spectrum on its own, or a saved arrangement of
   * several. Ticking a checkbox only chooses a spectrum for a new view and draws
   * nothing, so a stack can be put together without first superimposing it. */
  let page = $state<{ kind: 'doc' | 'view' | '2d'; id: number }>();
  let selected = $state<number[]>([]);
  const currentView = $derived.by(() => {
    const at = page;
    return at?.kind === 'view' ? views.find((v) => v.id === at.id) : undefined;
  });
  const viewId = $derived(currentView?.id);
  /** The ids drawn. They follow the page rather than being kept by hand, so
   * nothing can be shown that the page does not hold. */
  const visible = $derived.by(() => {
    if (currentView) return currentView.ids.filter((id) => docs.some((d) => d.id === id));
    // A 2D on screen leaves no spectrum active for the ribbon to act on.
    if (page?.kind === '2d') return [];
    const wanted = page?.kind === 'doc' ? page.id : activeId;
    const one = docs.find((d) => d.id === wanted) ?? docs[docs.length - 1];
    return one ? [one.id] : [];
  });
  /** The arrangement belongs to the view being shown; one spectrum needs none. */
  const stack = $derived<StackMode>(currentView?.stack ?? 'overlay');
  const shown2d = $derived.by(() => {
    const at = page;
    return at?.kind === '2d' ? docs2d.find((d) => d.id === at.id) : undefined;
  });
  /** Draw the open 1D spectra of a 2D's nuclei beside its contours. */
  let showProjections = $state(true);
  /** What is chosen for each side of each 2D, F2's and F1's: a spectrum by
   *  id, `null` for the 2D's own projection, nothing for automatic. */
  let besides = $state<Record<number, (number | null | undefined)[]>>({});
  /** The open spectrum a side shows unless told otherwise, as 1D spectra are
   *  attached to a 2D: of the nucleus of that side,
   *  recorded on the same spectrometer (within a MHz of the 2D's frequency
   *  for that nucleus), the last opened. Open spectra only - folders are never
   *  looked in, since a folder of experiments may hold other samples. */
  function autoSide(d: Document2DInfo, dim: 0 | 1): DocumentInfo | undefined {
    return docs
      .filter(
        (x) => x.nucleus === d.nuclei[dim] && Math.abs(x.frequencyMhz - d.frequenciesMhz[dim]) < 1,
      )
      .at(-1);
  }
  /** A side of a 2D: the spectrum chosen for it, or the 2D's own projection
   *  when that is chosen, or by default the open spectrum `autoSide` finds and
   *  failing that the projection - what the 2D shows until told otherwise. */
  function sideOf(d: Document2DInfo, dim: 0 | 1): DocumentInfo | 'own' {
    const choice = besides[d.id]?.[dim];
    if (choice === null) return 'own';
    const chosen = choice === undefined ? undefined : docs.find((x) => x.id === choice);
    return chosen ?? autoSide(d, dim) ?? 'own';
  }
  /** The sides of a 2D as a project keeps them: positions in its spectra. */
  const sidesOf = (d: Document2DInfo) =>
    ([0, 1] as const).map((dim): number | 'own' | null => {
      const choice = besides[d.id]?.[dim];
      if (choice === null) return 'own';
      const k = docs.findIndex((x) => x.id === choice);
      return k < 0 ? null : k;
    });
  const projections = $derived<
    [DocumentInfo | 'own' | undefined, DocumentInfo | 'own' | undefined]
  >(shown2d && showProjections ? [sideOf(shown2d, 0), sideOf(shown2d, 1)] : [undefined, undefined]);
  /** Peak labels are drawn. They are kept either way, so this only hides them. */
  let showLabels = $state(plotSettings.showLabels);
  /** Integral curves are drawn. The range and its value are shown either way. */
  let showCurves = $state(plotSettings.showCurves);
  /** Picking takes the top of the nearest line. Off, it takes the pointer where
   * it is - which Shift is meant to do while held, and does not always arrive. */
  let snap = $state(plotSettings.snap);
  /** Multiplet ranges are marked, with what each is called. */
  let showMultiplets = $state(plotSettings.showMultiplets);
  /** Integrals are drawn at all: curves, brackets and values. */
  let showIntegrals = $state(plotSettings.showIntegrals);

  /** Puts every annotation back, whichever of them was put away. */
  function showEverything() {
    showIntegrals = true;
    showLabels = true;
    showCurves = true;
    showMultiplets = true;
    showAll = true;
  }

  $effect(() => {
    const missing = docs.filter((d) => slots[d.id] === undefined);
    if (missing.length === 0) return;
    let next = Math.max(-1, ...Object.values(slots)) + 1;
    slots = { ...slots, ...Object.fromEntries(missing.map((d) => [d.id, next++])) };
  });

  let readout = $state('');
  /** Names given by hand, by document id; the folder decides the rest. */
  let names = $state<Record<number, string>>({});
  let font = $state(loadFont());

  function loadFont(): string {
    try {
      return localStorage.getItem(FONT_KEY) ?? SYSTEM_FONT;
    } catch {
      return SYSTEM_FONT;
    }
  }

  $effect(() => {
    const family =
      font === SYSTEM_FONT ? '' : `'${font}', system-ui, -apple-system, 'Segoe UI', sans-serif`;
    document.documentElement.style.setProperty('--font', family);
    try {
      localStorage.setItem(FONT_KEY, font);
    } catch {
      // A private window can refuse storage; the choice then lasts this session.
    }
  });

  const named = (doc: DocumentInfo) => names[doc.id] ?? doc.name;
  /** The choices for one side of the 2D on screen, the one in use ticked. */
  function sideItems(dim: 0 | 1): MenuItem[] {
    const d = shown2d;
    if (!d) return [];
    const current = sideOf(d, dim);
    const picked = besides[d.id]?.[dim];
    const choose = (choice: number | null | undefined) => () => {
      const pair = [...(besides[d.id] ?? [undefined, undefined])];
      pair[dim] = choice;
      besides = { ...besides, [d.id]: pair };
      showProjections = true;
    };
    const tick = (on: boolean, label: string) => (on ? `✓ ${label}` : label);
    const auto = autoSide(d, dim);
    return [
      {
        label: tick(picked === undefined, `Automatic (${auto ? named(auto) : 'projection'})`),
        title: 'An open spectrum of this nucleus from the same spectrometer, else the projection',
        run: choose(undefined),
      },
      {
        label: tick(picked === null, 'Projection of the 2D'),
        run: choose(null),
      },
      ...docs
        .filter((x) => x.nucleus === d.nuclei[dim])
        .map((x, k) => ({
          label: tick(picked !== undefined && current !== 'own' && current.id === x.id, named(x)),
          separator: k === 0,
          run: choose(x.id),
        })),
    ];
  }
  // One tool bar drives every pane, so the tools live here rather than in a plot.
  let mode = $state<Tool>('pointer');
  let zoomAxis = $state<Axis>('x');
  let panAxis = $state<Axis>('xy');
  let crosshair = $state(false);
  let noiseFactor = $state(plotSettings.noiseFactor);
  let recent = $state(loadRecent());
  let theme = $state<ThemeName>(loadTheme());
  let systemDark = $state(matchMedia('(prefers-color-scheme: dark)').matches);
  const COLOURS_KEY = 'opennmr.colours';
  const savedCustom = remembered(COLOURS_KEY, { dark: true, colors: {} as Record<string, string> });
  /** A scheme of one's own: every colour the app paints itself with. */
  let custom = $state<Record<string, string>>({ ...DEFAULT_CUSTOM, ...savedCustom.colors });
  let customDark = $state(savedCustom.dark);
  const saveColours = () => keepSetting(COLOURS_KEY, { dark: customDark, colors: custom });

  /** Choosing a colour is choosing your own scheme, so it takes effect at once. */
  function setColour(name: string, value: string) {
    custom = { ...custom, [name]: value };
    theme = CUSTOM;
    saveColours();
  }

  const dark = $derived(
    PALETTES[theme]?.dark ??
      (theme === CUSTOM ? customDark : theme === 'dark' || (theme === 'system' && systemDark)),
  );
  const traceColors = $derived(PALETTES[theme]?.traces ?? (dark ? PALETTE.dark : PALETTE.light));
  /** The palette's colour for each spectrum, before any chosen by hand. */
  const autoColors = $derived<Record<number, string>>(
    Object.fromEntries(
      docs.map((d) => [d.id, colorAt(slots[d.id] ?? 0, Math.max(docs.length, 1), traceColors)]),
    ),
  );
  const traceColorOf = $derived<Record<number, string>>({ ...autoColors, ...colors });
  let paletteOpen = $state(false);
  let settingsDialog = $state<HTMLDialogElement>();
  /** The multiplet whose window is open, where what it is can be said by hand. */
  let editing = $state<number>();

  /** A typed number, whichever separator was used for it. The fields are text so
   * that what is shown is what was written rather than what the machine's locale
   * would render, and a comma is what half the world types for a point. */
  const decimal = (typed: string) => Number(typed.replace(',', '.'));
  /** Draw the lines each multiplet would have, over the signal itself. */
  let showSimulation = $state(plotSettings.showSimulation);
  /** Where a baseline sits when a view is fitted, in % of the height up from the
   *  foot: the same for every spectrum, so a series lines up without moving each. */
  let baselineGap = $state(plotSettings.baselineGap);
  $effect(() =>
    keepSetting(PLOT_KEY, {
      showLabels,
      showCurves,
      showIntegrals,
      showMultiplets,
      showSimulation,
      showAll,
      snap,
      noiseFactor,
      stackAngle,
      autoScale,
      clipped,
      baselineGap,
    }),
  );
  // A multiplet is gone into in the side panel, which is there to say what the
  // selection is: as a window of its own it floated over the panel, the one
  // window of its kind. Opening one shows the panel; another stage leaves it.
  $effect(() => {
    if (editing !== undefined) panelOpen = true;
  });
  /** Opened as a modal, which is what dims the app and gives Escape its meaning. */
  const openSettings = () => settingsDialog?.showModal();
  let settingsTab = $state<'general' | 'processing' | 'analysis'>('general');
  /** The page's settings are beside the page itself, so a change is seen. */
  function openPageSetup() {
    pageMode = true;
    panelOpen = true;
  }
  let helpDialog = $state<HTMLDialogElement>();
  /** One plot, keyed `SINGLE`, or in split mode one per spectrum by its id;
   *  exports use the active one. */
  let panes = $state<Record<number, ReturnType<typeof Plot> | undefined>>({});
  const SINGLE = -1;
  /** Spectra drawn in the plot, bottom to top, with the active one on top. */
  // Document order, not active last: a stack must not reorder itself under the
  // pointer when a row is selected.
  const shown = $derived(docs.filter((d) => visible.includes(d.id)));
  const active = $derived(
    shown.find((d) => d.id === activeId) ?? shown[shown.length - 1] ?? undefined,
  );

  /** Anything larger is not worth holding on to; a structure copied out of
   * ChemDraw is tens of kilobytes. */
  const STRUCTURE_MAX = 4_000_000;
  /** A structure for each spectrum, by document, and only for as long as the
   * spectrum is open - as its peaks and integrals are. */
  let structures = $state<Record<number, string>>({});
  /** Whose structure is shown: the spectrum worked on, or, with a 2D on
   *  screen, the ¹H spectrum beside it - the same compound, and the molecule
   *  its cross peaks are assigned on. */
  const holder = $derived.by(() => {
    if (active || !shown2d) return active;
    const side = sideOf(shown2d, 0);
    return side === 'own' ? undefined : side;
  });
  const structure = $derived(holder ? structures[holder.id] : undefined);
  /** What a molfile structure reads as, by document: its hydrogens are what a
   * signal can be assigned to. A picture has none. */
  let molecules = $state<Record<number, Molecule>>({});
  const molecule = $derived(holder ? molecules[holder.id] : undefined);

  /** Where a paste lands: the web view gives the event to whatever can be typed
   * into, so the pane keeps a layer that can be. */
  let paster = $state<HTMLElement>();

  const STRUCTURE_TYPES: Record<string, string> = {
    pdf: 'application/pdf',
    png: 'image/png',
    jpg: 'image/jpeg',
    jpeg: 'image/jpeg',
    gif: 'image/gif',
    tif: 'image/tiff',
    tiff: 'image/tiff',
    svg: 'image/svg+xml',
    mol: MOLFILE_TYPE,
    sdf: MOLFILE_TYPE,
  };

  /** Keeps `url` as the structure of document `id`. */
  async function keepUrl(id: number, url: string) {
    if (url.length > STRUCTURE_MAX) {
      fail('That structure is too large to keep. Try a smaller one.');
      return;
    }
    // Read before it is kept, so a file that is not a molecule leaves the last
    // structure as it was.
    const text = molfileText(url);
    let read: Molecule | undefined;
    if (text !== undefined) {
      try {
        read = await readMolecule(text);
      } catch (e) {
        fail(String(e));
        return;
      }
    }
    const others = Object.entries(molecules).filter(([key]) => Number(key) !== id);
    molecules = Object.fromEntries(read ? [...others, [id, read]] : others);
    const before = structures[id];
    structures = { ...structures, [id]: url };
    // Assignments are atom positions, so another molecule's are not this one's.
    // Loading a project comes through here with nothing yet, and keeps its own.
    const doc = docs.find((d) => d.id === id);
    if (before !== undefined && before !== url && doc) {
      void clearAssigned(doc);
      void clearAssigned2d(doc);
    }
  }

  /** Lets go of the atoms the cross peaks of every 2D beside `doc` were given,
   *  when the molecule they were positions in is gone. */
  async function clearAssigned2d(doc: DocumentInfo) {
    for (const d of docs2d) {
      const side = sideOf(d, 0);
      if (side === 'own' || side.id !== doc.id) continue;
      if (!d.peaks.some((p) => p.atoms.length)) continue;
      try {
        const updated = await setPeaks2d(
          d.id,
          d.peaks.map((p) => ({ ...p, atoms: [] })),
        );
        docs2d = docs2d.map((x) => (x.id === updated.id ? updated : x));
      } catch (e) {
        fail(String(e));
      }
    }
  }

  function keepStructure(id: number, type: string, bytes: ArrayBuffer) {
    const reader = new FileReader();
    reader.onload = () => void keepUrl(id, String(reader.result));
    reader.readAsDataURL(new Blob([bytes], { type }));
  }

  /** Reads a structure off disk, for a clipboard that offers a path rather than
   * the drawing itself. */
  async function structureFromPath(id: number, file: string) {
    try {
      const type = STRUCTURE_TYPES[file.split('.').pop()?.toLowerCase() ?? ''];
      if (!type) {
        fail(`Not a structure this can read: ${file}`);
        return;
      }
      keepStructure(id, type, await readBytes(file));
    } catch (e) {
      fail(String(e));
    }
  }

  /** A file URL as a path, which is what the back end reads. */
  const asPath = (url: string | undefined) =>
    url?.startsWith('file:') ? decodeURI(url.replace(/^file:\/\//, '')) : undefined;

  /** Reads the drawing off the pasteboard itself, which is where it is: the web
   * view hands a page no flavours at all when the copy came from another
   * application, which is what ChemDraw is. */
  async function pasteFromPasteboard() {
    const id = holder?.id;
    if (id === undefined) return;
    try {
      const url = await clipboardStructure();
      if (url) await keepUrl(id, url);
      else fail('Nothing on the clipboard that can be kept as a structure.');
    } catch (e) {
      fail(String(e));
    }
  }

  function pasteStructure(e: ClipboardEvent) {
    // Typing into a field is not pasting a structure; the pane's own layer is the
    // exception, since that is what a paste is given to.
    if (e.target !== paster && isEditable(e.target)) return;
    const id = holder?.id;
    const data = e.clipboardData;
    if (id === undefined) return;
    // Nothing is ever put into the layer: what is pasted becomes the structure.
    if (e.target === paster) e.preventDefault();

    // A molfile copied as text is the structure itself, so it comes before any picture.
    const text = data?.getData('text/plain') ?? '';
    if (looksLikeMolfile(text)) {
      e.preventDefault();
      void keepUrl(id, molfileUrl(text));
      return;
    }

    const file = [...(data?.items ?? [])].find((entry) => entry.kind === 'file')?.getAsFile();
    if (file) {
      e.preventDefault();
      const reader = new FileReader();
      reader.onload = () => void keepUrl(id, String(reader.result));
      reader.readAsDataURL(file);
      return;
    }

    const src = /<img[^>]+src="([^"]+)"/i.exec(data?.getData('text/html') ?? '')?.[1];
    if (src?.startsWith('data:')) {
      e.preventDefault();
      void keepUrl(id, src);
      return;
    }
    const listed = (data?.getData('text/uri-list') ?? '')
      .split(/\s+/)
      .find((line) => line.startsWith('file:'));
    const onDisk = asPath(src) ?? asPath(listed);
    if (onDisk) {
      e.preventDefault();
      void structureFromPath(id, onDisk);
      return;
    }
    // Measured: a copy from another application leaves the page nothing at all.
    void pasteFromPasteboard();
  }

  /** For when the clipboard will not give it up: the same structure off disk. */
  async function chooseStructure() {
    const id = holder?.id;
    if (id === undefined) return;
    try {
      const file = await open({
        title: 'Choose a structure',
        filters: [{ name: 'Structure', extensions: Object.keys(STRUCTURE_TYPES) }],
      });
      if (!file) return;
      const type = STRUCTURE_TYPES[file.split('.').pop()?.toLowerCase() ?? ''];
      if (!type) return;
      keepStructure(id, type, await readBytes(file));
    } catch (e) {
      fail(String(e));
    }
  }

  /** The signal each atom is assigned to, by the name it is shown with. */
  const assignedTo = $derived.by(() => {
    const names: Record<number, string> = {};
    if (shown2d) {
      // A cross peak is named by its number, as on the plot and in the list.
      shown2d.peaks.forEach((p, k) => p.atoms.forEach((atom) => (names[atom] = String(k + 1))));
      return names;
    }
    active?.analysis.multiplets.forEach((m, i) =>
      m.atoms?.forEach((atom) => (names[atom] = m.name ?? letterOf(i))),
    );
    return names;
  });

  function dropStructure() {
    const doc = holder;
    if (!doc) return;
    const gone = ([key]: [string, unknown]) => Number(key) !== doc.id;
    structures = Object.fromEntries(Object.entries(structures).filter(gone));
    molecules = Object.fromEntries(Object.entries(molecules).filter(gone));
    void clearAssigned(doc);
    void clearAssigned2d(doc);
  }

  $effect(() => {
    const id = active?.id ?? shown2d?.id;
    if (tab !== 'metadata' || id === undefined) return;
    fetchMetadata(id).then(
      (rows) => (recorded = rows),
      (e: unknown) => fail(String(e)),
    );
  });
  /** The entry whose name is being typed. Renaming is a second, deliberate
   * gesture, so that clicking an entry only goes to it. */
  let renaming = $state<{ kind: 'doc' | 'view'; id: number }>();
  const isRenaming = (kind: 'doc' | 'view', id: number) =>
    renaming?.kind === kind && renaming.id === id;

  /** Puts the cursor in a name as soon as it appears. */
  function takeCursor(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  function finishRename(kind: 'doc' | 'view', id: number, value: string) {
    renaming = undefined;
    if (!value.trim()) return;
    if (kind === 'doc') names = { ...names, [id]: value.trim() };
    else renameView(id, value);
  }

  /** Enter keeps a name, Escape puts the old one back; neither reaches the row. */
  function nameKeys(e: KeyboardEvent & { currentTarget: HTMLInputElement }, original: string) {
    e.stopPropagation();
    if (e.key === 'Enter') e.currentTarget.blur();
    else if (e.key === 'Escape') {
      e.currentTarget.value = original;
      e.currentTarget.blur();
    }
  }

  /** How a view says it is arranged, in the list. */
  const ARRANGEMENT: Record<StackMode, string> = {
    overlay: 'superimposed',
    stacked: 'stacked',
    split: 'split panes',
  };
  const threshold = $derived(active ? active.noise * noiseFactor : 0);
  const split = $derived(stack === 'split' && shown.length > 1);
  /** What each plot on screen draws: the whole of `shown` in one, or in split
   *  mode one spectrum each, which is its own active one and has its own view. */
  const paneList = $derived(
    !active
      ? []
      : split && !pageMode
        ? shown.map((doc) => ({
            key: doc.id,
            docs: [doc],
            active: doc,
            palette: [traceColorOf[doc.id] ?? traceColors[0]],
            stack: 'overlay' as const,
            threshold: doc.noise * noiseFactor,
            focused: doc.id === activeId,
          }))
        : [
            {
              key: SINGLE,
              docs: shown,
              active,
              palette: traceColors,
              stack,
              threshold,
              focused: true,
            },
          ],
  );

  function loadTheme(): ThemeName {
    try {
      const value: unknown = localStorage.getItem(THEME_KEY);
      return isTheme(value) ? value : 'system';
    } catch {
      return 'system';
    }
  }

  $effect(() => {
    const root = document.documentElement;
    const palette = PALETTES[theme];
    // Every colour a scheme set before goes, whatever it was called: a role a
    // newer build no longer sets (the green it once had for what is live) stayed
    // on the page from the scheme chosen before, until the window was reloaded.
    for (const name of [...root.style]) {
      if (name.startsWith('--') && name !== '--font') root.style.removeProperty(name);
    }
    // No attribute means "follow the system", which is what the stylesheet asks for.
    if (theme === CUSTOM) {
      root.dataset.theme = customDark ? 'dark' : 'light';
      for (const [name] of CUSTOM_COLOURS) root.style.setProperty(name, custom[name]);
    } else if (palette) {
      root.dataset.theme = palette.dark ? 'dark' : 'light';
      for (const [name, value] of Object.entries(palette.colors))
        root.style.setProperty(name, value);
    } else if (theme === 'system') delete root.dataset.theme;
    else root.dataset.theme = theme;
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      // Private windows can refuse storage; the choice then lasts this session.
    }
  });

  function loadRecent(): string[] {
    try {
      const value: unknown = JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]');
      return Array.isArray(value) ? value.filter((p) => typeof p === 'string') : [];
    } catch {
      return [];
    }
  }

  /** Closes a spectrum and forgets everything held about it, so opening its
   * folder again reads it afresh rather than restoring its old analysis. */
  async function closeDoc(id: number) {
    try {
      await closeDocument(id);
    } catch (e) {
      fail(String(e));
    }
    docs = docs.filter((d) => d.id !== id);
    selected = selected.filter((s) => s !== id);
    if (page?.kind === 'doc' && page.id === id) page = undefined;
    display = Object.fromEntries(Object.entries(display).filter(([key]) => Number(key) !== id));
    colors = Object.fromEntries(Object.entries(colors).filter(([key]) => Number(key) !== id));
    structures = Object.fromEntries(
      Object.entries(structures).filter(([key]) => Number(key) !== id),
    );
    molecules = Object.fromEntries(Object.entries(molecules).filter(([key]) => Number(key) !== id));
    if (activeId === id) activeId = visible[visible.length - 1] ?? docs[docs.length - 1]?.id;
  }

  /** Keeps the ticked spectra as a view, in the order they were opened, and
   * shows it. They need never have been on screen together. */
  function saveView(mode: StackMode) {
    const ids = docs.filter((d) => selected.includes(d.id)).map((d) => d.id);
    if (ids.length < 2) return;
    const id = Math.max(0, ...views.map((v) => v.id)) + 1;
    const kind = mode === 'stacked' ? 'Stack' : mode === 'split' ? 'Split' : 'Superimposed';
    views = [...views, { id, name: `${kind} of ${ids.length} spectra`, ids, stack: mode }];
    showView(id);
  }

  /** Goes to a view: the same spectra, arranged the same way. */
  function showView(id: number) {
    const view = views.find((v) => v.id === id);
    const open = view?.ids.filter((docId) => docs.some((d) => d.id === docId)) ?? [];
    if (open.length === 0) return;
    page = { kind: 'view', id };
    if (activeId === undefined || !open.includes(activeId)) activeId = open[open.length - 1];
  }

  /** Goes to one spectrum on its own, which is then the one worked on. */
  function showDoc(id: number) {
    page = { kind: 'doc', id };
    activeId = id;
  }

  /** A name a view is called by, as a spectrum is renamed in the same list. */
  function renameView(id: number, name: string) {
    views = views.map((v) => (v.id === id ? { ...v, name: name.trim() || v.name } : v));
  }

  let lastTicked = $state<number>();

  /** Ticking a spectrum chooses it for a new view; the plot does not change. */
  function toggleSelected(id: number) {
    selected = selected.includes(id) ? selected.filter((s) => s !== id) : [...selected, id];
    lastTicked = id;
  }

  /** A plain click goes to a spectrum, since that is what the list is for.
   * Ctrl or Cmd ticks it instead, without leaving the page, and Shift ticks
   * everything between it and the one ticked before. */
  function pickRow(e: MouseEvent, id: number) {
    const from = docs.findIndex((d) => d.id === lastTicked);
    const to = docs.findIndex((d) => d.id === id);
    if (e.shiftKey && from >= 0 && to >= 0) {
      const span = docs.slice(Math.min(from, to), Math.max(from, to) + 1).map((d) => d.id);
      selected = [...new Set([...selected, ...span])];
      lastTicked = id;
    } else if (e.metaKey || e.ctrlKey) {
      toggleSelected(id);
    } else {
      showDoc(id);
    }
  }

  /** Clicking a trace works on that spectrum without leaving the page. */
  function activate(id: number) {
    activeId = id;
  }

  /** Opens every folder given, each as its own spectra.
   *
   * What goes wrong in one folder is kept rather than written over by the next,
   * and the view goes to the first spectrum of the whole batch rather than to the
   * first of whichever folder happened to be last. */
  async function loadFolders(paths: string[]) {
    const problems: string[] = [];
    const fresh: DocumentInfo[] = [];
    const fresh2d: Document2DInfo[] = [];
    for (const path of paths) {
      try {
        const opened = await openPath(path, phaseOnOpen, baselineOnOpen ? defaultBaseline() : null);
        problems.push(...opened.errors);
        const known = new Set([...docs, ...fresh, ...docs2d, ...fresh2d].map((d) => d.id));
        fresh.push(...opened.documents.filter((d) => !known.has(d.id)));
        fresh2d.push(...opened.documents2d.filter((d) => !known.has(d.id)));
        remember(path);
      } catch (e) {
        problems.push(String(e));
      }
    }
    errors = [...new Set(problems)];
    docs = [...docs, ...fresh];
    docs2d = [...docs2d, ...fresh2d];
    const first = fresh[0]?.id;
    if (first !== undefined) showDoc(first);
    else if (fresh2d[0]) page = { kind: '2d', id: fresh2d[0].id };
  }

  /** Closes a 2D experiment. */
  async function closeDoc2d(id: number) {
    try {
      await closeDocument(id);
    } catch (e) {
      fail(String(e));
    }
    docs2d = docs2d.filter((d) => d.id !== id);
    if (page?.kind === '2d' && page.id === id) page = undefined;
  }

  /** A project file or a data folder, whichever `path` is: dropped, or reopened
   *  from the recent list, which keeps both. */
  const load = (path: string) => (isProjectFile(path) ? openProject(path) : loadFolders([path]));

  function remember(path: string) {
    recent = [path, ...recent.filter((p) => p !== path)].slice(0, RECENT_MAX);
    localStorage.setItem(RECENT_KEY, JSON.stringify(recent));
  }

  async function chooseFolder() {
    const chosen = await open({
      directory: true,
      multiple: true,
      title: 'Open Bruker experiment or dataset folders',
    });
    if (!chosen) return;
    await loadFolders(Array.isArray(chosen) ? chosen : [chosen]);
  }

  let analysisBusy = $state(false);

  /** Runs a change on the active document and shows its result. */
  async function change(request: (id: number) => Promise<DocumentInfo>) {
    if (!active || analysisBusy) return;
    try {
      const updated = await request(active.id);
      docs = docs.map((d) => (d.id === updated.id ? updated : d));
      errors = [];
    } catch (e) {
      fail(String(e));
    }
  }

  const apply = (ops: Operation[]) => change((id) => setOperations(id, ops));
  const updateAnalysis = (analysis: Analysis) => change((id) => setAnalysis(id, analysis));

  /** One change, so a single undo brings the assignments back with the structure. */
  async function clearAssigned(doc: DocumentInfo) {
    const cleared = clearAssignments(doc.analysis);
    if (cleared === doc.analysis) return;
    // Straight to the document rather than through `change`, which acts on the
    // active spectrum: with a 2D on screen there is none, and the ¹H spectrum
    // beside it kept atoms of a molecule that was gone.
    try {
      const updated = await setAnalysis(doc.id, cleared);
      docs = docs.map((d) => (d.id === updated.id ? updated : d));
    } catch (e) {
      fail(String(e));
    }
  }

  /** Hydrogens picked on the structure and waiting for a signal. */
  let armed = $state<number[]>([]);
  /** The multiplet the pointer is over on the plot, and the atom it is over on the
   * structure: each lights up what the other is assigned to. */
  let hoverSignal = $state<number>();
  let hoverAtom = $state<number>();
  /** The cross peak the pointer is over on a 2D. */
  let hoverPeak = $state<number>();
  const linkedAtoms = $derived(
    shown2d
      ? hoverPeak === undefined
        ? []
        : (shown2d.peaks[hoverPeak]?.atoms ?? [])
      : hoverSignal === undefined
        ? []
        : (active?.analysis.multiplets[hoverSignal]?.atoms ?? []),
  );
  /** The cross peak given the atom the pointer is over, on a 2D. */
  const markedPeak = $derived.by(() => {
    const atom = hoverAtom;
    if (atom === undefined || !shown2d) return undefined;
    const at = shown2d.peaks.findIndex((p) => p.atoms.includes(atom));
    return at < 0 ? undefined : at;
  });
  /** The multiplet lit on the plot: the one whose hydrogens the pointer is over,
   *  or else the one whose window is open, which is the one being worked on. */
  const markedSignal = $derived.by(() => {
    const atom = hoverAtom;
    if (atom === undefined) return editing;
    const at = active?.analysis.multiplets.findIndex((m) => m.atoms?.includes(atom));
    return at === undefined || at < 0 ? undefined : at;
  });
  // A structure or a spectrum that is gone leaves nothing to be waiting for.
  $effect(() => {
    if (!molecule || (!active && !shown2d)) armed = [];
  });

  /** A click on a structure's hydrogens. With a multiplet's window open they go to
   * it, or come away from it; otherwise they are picked and wait for the signal
   * they belong to to be clicked on the spectrum, in either order being the same
   * two clicks. */
  function pickAtom(atom: number) {
    if (!molecule) return;
    const group = classOf(molecule, atom);
    if (shown2d) {
      armed = armed.includes(atom) ? [] : group;
      return;
    }
    if (!active) return;
    const open = editing === undefined ? undefined : active.analysis.multiplets[editing];
    if (open && editing !== undefined) {
      void updateAnalysis(toggleAssignment(active.analysis, editing, molecule, group));
      return;
    }
    armed = armed.includes(atom) ? [] : group;
  }

  /** The cross peak clicked while atoms were picked is the one they are given
   *  to; given the same atoms again, it lets go of them. */
  function assignArmedPeak(index: number) {
    const doc = shown2d;
    if (!doc || armed.length === 0) return;
    const peaks = assignCrossPeak(doc.peaks, index, armed);
    const given = peaks[index]?.atoms.length ? 'assigned' : 'unassigned';
    void change2d((id) => setPeaks2d(id, peaks));
    say(`Cross peak ${index + 1} ${given} · ${commandKey}Z undoes it`);
    armed = [];
  }

  /** The signal clicked while hydrogens were picked is the one they are given to. */
  function assignArmed(index: number) {
    if (!active || !molecule || armed.length === 0) return;
    const h = armed.reduce((sum, i) => sum + (molecule.atoms[i]?.h ?? 0), 0);
    const m = active.analysis.multiplets[index];
    void updateAnalysis(toggleAssignment(active.analysis, index, molecule, armed, true));
    say(`${h} H given to ${m?.name ?? letterOf(index)} · ${commandKey}Z undoes it`);
    armed = [];
  }

  // Escape lets go of picked hydrogens before it does anything else.
  $effect(() => {
    if (armed.length === 0) return;
    const escape = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.stopPropagation();
      armed = [];
    };
    window.addEventListener('keydown', escape, true);
    return () => window.removeEventListener('keydown', escape, true);
  });

  /** Fits shift, couplings and width of multiplet `index` to the spectrum and
   *  keeps them as its own, which Cmd+Z takes back. */
  async function fitEditing(index: number) {
    if (!active) return;
    try {
      const fit = await fitMultiplet(active.id, index);
      if (!fit) {
        say('A multiplet read as m has no line pattern to fit; choose its class first');
        return;
      }
      let next = setMultipletShift(active.analysis, index, fit.ppm);
      next = setMultipletJs(next, index, fit.jsHz);
      next = setMultipletWidth(next, index, fit.widthHz);
      await updateAnalysis(next);
      const js = fit.jsHz.map((j) => j.toFixed(2)).join(', ');
      say(
        `Fitted: ${fixed(fit.ppm, 3)} ppm${js ? `, J = ${js} Hz` : ''}, lines ${fit.widthHz.toFixed(2)} Hz · ${commandKey}Z undoes it`,
      );
    } catch (e) {
      fail(String(e));
    }
  }

  /** `doc`'s analysis with every multiplet fitted to the spectrum, and how many
   *  were: an m has no line pattern and is left as it is. */
  async function fitEvery(doc: DocumentInfo) {
    let next = doc.analysis;
    let fitted = 0;
    for (let i = 0; i < doc.analysis.multiplets.length; i++) {
      const fit = await fitMultiplet(doc.id, i);
      if (!fit) continue;
      next = setMultipletShift(next, i, fit.ppm);
      next = setMultipletJs(next, i, fit.jsHz);
      next = setMultipletWidth(next, i, fit.widthHz);
      fitted++;
    }
    return { next, fitted };
  }

  /** Fits every multiplet of the active spectrum as one change, so one Cmd+Z
   *  takes all of it back. An m has no line pattern and is left as it is. */
  async function fitAll() {
    if (!active) return;
    const doc = active;
    try {
      const { next, fitted } = await fitEvery(doc);
      if (fitted === 0) {
        say('No multiplet has a line pattern to fit; give them a class first');
        return;
      }
      await updateAnalysis(next);
      const left = doc.analysis.multiplets.length - fitted;
      say(
        `Fitted ${fitted} ${fitted === 1 ? 'multiplet' : 'multiplets'}${left ? `, ${left} read as m left as they were` : ''} · ${commandKey}Z undoes it`,
      );
    } catch (e) {
      fail(String(e));
    }
  }

  /** Quotes a coupling two multiplets share as one number, their average. */
  function matchCoupling(at: number, shared: SharedCoupling) {
    if (!active) return;
    const infos = active.multipletInfo;
    const mean = (infos[at].jsHz[shared.j] + shared.hz) / 2;
    const replaced = (js: number[], k: number) => js.map((j, i) => (i === k ? mean : j));
    let next = setMultipletJs(active.analysis, at, replaced(infos[at].jsHz, shared.j));
    next = setMultipletJs(next, shared.other, replaced(infos[shared.other].jsHz, shared.k));
    void updateAnalysis(next);
  }
  const undo = () => (shown2d ? change2d(undo2d) : change(undoChange));
  const redo = () => (shown2d ? change2d(redo2d) : change(redoChange));

  /** Runs a change on the 2D on screen and shows its result. */
  async function change2d(request: (id: number) => Promise<Document2DInfo>) {
    if (!shown2d) return;
    try {
      const updated = await request(shown2d.id);
      docs2d = docs2d.map((d) => (d.id === updated.id ? updated : d));
      errors = [];
    } catch (e) {
      fail(String(e));
    }
  }

  /** Picks the cross peak a click points at: its top, found nearby, or with
   *  Snap off the point clicked. */
  async function pickCrossPeak(at: [number, number], radius: [number, number]) {
    const doc = shown2d;
    if (!doc) return;
    try {
      const top = snap ? await nearestPeak2d(doc.id, at, radius) : null;
      const picked: CrossPeak = { at: top ?? at, atoms: [] };
      await change2d((id) => setPeaks2d(id, [...doc.peaks, picked]));
    } catch (e) {
      fail(String(e));
    }
  }

  const removeCrossPeak = (index: number) =>
    shown2d &&
    change2d((id) =>
      setPeaks2d(
        id,
        shown2d.peaks.filter((_, k) => k !== index),
      ),
    );
  const applyPhase = (phase: PhaseParams) => active && apply(withPhase(active.operations, phase));
  const applyBaselinePoints = (baseline: BaselineOperation) =>
    active && apply(withBaseline(active.operations, baseline));
  /** Names a peak, which shifts the axis, and everything already found on it.
   *
   * The step replaces any earlier reference, so only the difference between the
   * two moves the analysis; otherwise a second reference would count twice. */
  async function reference(fromPpm: number, toPpm: number) {
    if (!active) return;
    const earlier = active.operations.find((o) => o.op === 'reference');
    const applied = earlier?.op === 'reference' ? earlier.to_ppm - earlier.from_ppm : 0;
    const delta = toPpm - fromPpm - applied;
    const moved = shiftAnalysis(active.analysis, delta);
    await apply(withReference(active.operations, fromPpm, toPpm));
    if (delta !== 0) await updateAnalysis(moved);
  }
  const integrate = (from: number, to: number) =>
    active && updateAnalysis(addIntegral(active.analysis, from, to));
  const addMultipletRange = (from: number, to: number) =>
    active && updateAnalysis(addMultiplet(active.analysis, from, to));
  /** How far each annotation has been moved down, by spectrum: its labels, its
   * integral curve, the bracket under it, and its multiplet rule. */
  let shifts = $state<Record<number, Partial<Record<Moved, number>>>>({});
  const shiftAnnotation = (id: number, what: Moved, dy: number) =>
    (shifts = { ...shifts, [id]: { ...shifts[id], [what]: dy } });

  const REMOVE = { peak: removePeak, integral: removeIntegral, multiplet: removeMultiplet };
  const drop = (kind: keyof typeof REMOVE, index: number) =>
    active && updateAnalysis(REMOVE[kind](active.analysis, index));
  const labelPeak = (index: number, label: string | null) =>
    active && updateAnalysis(setPeakLabel(active.analysis, index, label));
  /** An integral and the multiplet read off the same signal are drawn as one row
   * and move as one thing: dragging either end carries both, in a single change,
   * so one undo puts the whole row back. The pair is found before anything is
   * written, since each setter reorders only its own list. */
  const moveMultiplet = (index: number, from: number, to: number) => {
    if (!active) return;
    const near = Math.abs(active.deltaPpm);
    const integral = pairedIntegral(active.analysis, index, near);
    let changed = setMultipletRange(active.analysis, index, from, to);
    if (integral >= 0) changed = setIntegral(changed, integral, from, to);
    updateAnalysis(changed);
  };
  const moveIntegral = (index: number, from: number, to: number) => {
    if (!active) return;
    const near = Math.abs(active.deltaPpm);
    const multiplet = pairedMultiplet(active.analysis, index, near);
    let changed = setIntegral(active.analysis, index, from, to);
    if (multiplet >= 0) changed = setMultipletRange(changed, multiplet, from, to);
    updateAnalysis(changed);
  };
  const moveTrace = (id: number, dx: number, dy: number) =>
    (display = { ...display, [id]: { ...(display[id] ?? NO_ADJUSTMENT), dx, dy } });
  const mapAdjustments = (change: (a: Adjustment) => Adjustment) =>
    (display = Object.fromEntries(Object.entries(display).map(([id, a]) => [id, change(a)])));
  const resetShifts = () => mapAdjustments((a) => ({ ...a, dx: 0, dy: 0 }));
  const resetIntensities = () => mapAdjustments((a) => ({ ...a, scale: 1 }));
  const adjusted = $derived(
    Object.values(display).some((a) => a.dx !== 0 || a.dy !== 0 || a.scale !== 1),
  );
  const scaleTrace = (id: number, factor: number) => {
    const current = display[id] ?? NO_ADJUSTMENT;
    display = { ...display, [id]: { ...current, scale: Math.max(0.05, current.scale * factor) } };
  };

  /** A line in the hint bar for a few seconds: what an automatic step just did. */
  let notice = $state<string>();
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  function say(text: string) {
    notice = text;
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = undefined), NOTICE_MS);
  }

  async function autoPhaseActive() {
    if (!active || analysisBusy) return;
    const { id, operations } = active;
    try {
      const previous = findPhase(operations);
      const phase = await autoPhase(id, {
        p0: previous?.p0 ?? 0,
        p1: previous?.p1 ?? 0,
        pivot: previous?.pivot ?? (await largestPeak(id)),
      });
      await apply(withPhase(operations, phase));
      if (errors.length === 0)
        say(
          `Phase set to PH0 ${phase.p0.toFixed(1)}°, PH1 ${phase.p1.toFixed(1)}° · ${commandKey}Z undoes it · Shift+P to adjust by hand`,
        );
    } catch (e) {
      fail(String(e));
    }
  }

  async function autoBaseline() {
    if (analysisBusy) return;
    panelOpen = true;
    tab = 'processing';
    if (!active) return;
    if (active.operations.some(isBaseline)) {
      say('This spectrum already has a baseline correction; change it in the panel on the right.');
      return;
    }
    await apply(withBaseline(active.operations, defaultBaseline()));
    if (errors.length === 0)
      say(`Baseline corrected · ${commandKey}Z undoes it · the method can be changed in the panel`);
  }

  async function addPeaks(found: Promise<number[]>) {
    if (!active) return;
    const { analysis, deltaPpm } = active;
    try {
      await updateAnalysis(mergePeaks(analysis, await found, Math.abs(deltaPpm)));
    } catch (e) {
      fail(String(e));
    }
  }

  /** Picks in every spectrum on screen, each against its own noise, merging what
   * is found into that spectrum's own peaks. Auto peaks and a dragged region are
   * the same thing over a different range, so they share this. */
  async function pickInEvery(found: (doc: DocumentInfo) => Promise<number[]>) {
    if (analysisBusy) return;
    for (const doc of shown) {
      try {
        const merged = mergePeaks(doc.analysis, await found(doc), Math.abs(doc.deltaPpm));
        const updated = await setAnalysis(doc.id, merged);
        docs = docs.map((d) => (d.id === updated.id ? updated : d));
      } catch (e) {
        fail(String(e));
      }
    }
  }

  /** The threshold line belongs to the active spectrum, so picking across a stack
   * reads each spectrum against its own noise instead of holding them all to one
   * spectrum's line. */
  const pickingEvery = $derived(showAll && shown.length > 1);

  const pickRange = (from: number, to: number) => {
    if (!active) return;
    if (pickingEvery)
      void pickInEvery((doc) => findPeaks(doc.id, from, to, doc.noise * noiseFactor));
    else void addPeaks(findPeaks(active.id, from, to, threshold));
  };

  function pickPoint(ppm: number, exact: boolean, radius: number) {
    if (!active) return;
    if (pickingEvery) {
      // The same shift in every spectrum, each snapping to its own line unless
      // the pointer is being taken exactly where it is.
      void pickInEvery(async (doc) => {
        const at = exact ? ppm : await nearestPeak(doc.id, ppm, radius);
        return at === null ? [] : [at];
      });
      return;
    }
    const peak = exact ? Promise.resolve(ppm) : nearestPeak(active.id, ppm, radius);
    void addPeaks(peak.then((p) => (p === null ? [] : [p])));
  }

  /** Asks where to save, and writes there with `write`. */
  async function saveWithDialog(
    defaultPath: string,
    filter: string,
    extension: string,
    write: (path: string) => Promise<unknown>,
  ) {
    try {
      const path = await save({
        defaultPath,
        filters: [{ name: filter, extensions: [extension] }],
      });
      if (!path) return undefined;
      const written = withExtension(path, extension);
      await write(written);
      return written;
    } catch (e) {
      fail(String(e));
      return undefined;
    }
  }

  /** `saveWithDialog` for bytes worked out here. */
  const saveBytes = (
    defaultPath: string,
    filter: string,
    extension: string,
    content: () => Uint8Array | Promise<Uint8Array>,
  ) =>
    saveWithDialog(defaultPath, filter, extension, async (path) => saveFile(path, await content()));

  const encoder = new TextEncoder();
  const fileName = (suffix: string) =>
    `${(active?.name ?? 'spectrum').replaceAll(/[\\/:*?"<>|\s]+/g, '_')}${suffix}`;

  function exportView(kind: 'svg' | 'png') {
    const view = panes[split ? (activeId ?? SINGLE) : SINGLE];
    if (!view) return;
    const content = () => (kind === 'svg' ? encoder.encode(view.exportSvg()) : view.exportPng());
    void saveBytes(fileName(`.${kind}`), kind.toUpperCase(), kind, content);
  }

  function exportCsv(kind: 'peaks' | 'integrals') {
    const doc = active;
    if (!doc) return;
    const text = kind === 'peaks' ? peaksCsv(doc) : integralsCsv(doc);
    void saveBytes(fileName(`-${kind}.csv`), 'CSV', 'csv', () => encoder.encode(text));
  }

  const fileOf = (path: string) => path.split(/[\\/]/).pop() ?? path;
  const safeName = (name: string) => name.replaceAll(/[\\/:*?"<>|\s]+/g, '_');
  const pageName = () => safeName(currentView?.name ?? (active ? named(active) : 'spectrum'));

  /** The pane that is printed: the one plot there is on a page. */
  const printed = () => panes[SINGLE];

  /** The page as it is drawn now; page mode is turned on first if it was off,
   *  so what is written is what would be seen. */
  async function pageSvg(): Promise<string | undefined> {
    if (!pageMode) {
      pageMode = true;
      await tick();
    }
    const plot = printed();
    if (!plot) return undefined;
    await plot.settled();
    return plot.exportPage(pageSetup.width, pageSetup.height, pageSetup.margin);
  }

  async function exportPageSvg() {
    const svg = await pageSvg();
    if (svg) void saveBytes(`${pageName()}.svg`, 'SVG', 'svg', () => encoder.encode(svg));
  }

  /** Whether a PDF carries the project, so that opening it gives the work back. */
  const EXPORT_KEY = 'opennmr.export';
  let attachProject = $state(remembered(EXPORT_KEY, { project: true }).project);
  $effect(() => keepSetting(EXPORT_KEY, { project: attachProject }));

  async function exportPdf() {
    const svg = await pageSvg();
    if (!svg) return;
    const project = attachProject ? await projectAttachment() : null;
    const path = await saveWithDialog(`${pageName()}.pdf`, 'PDF', 'pdf', (to) =>
      savePdf(to, [svg], project),
    );
    if (path) say(`Saved ${fileOf(path)}`);
  }

  /** Many spectra printed alike: each shown in turn on the page, with the range
   *  and zoom the plot has, and written as it is drawn. */
  let exportDialog = $state<HTMLDialogElement>();
  let exportIds = $state<number[]>([]);
  let exportOne = $state(true);
  let exporting = $state(false);

  function openExportMany() {
    exportIds = docs.map((d) => d.id);
    exportDialog?.showModal();
  }

  async function exportMany() {
    const chosen = docs.filter((d) => exportIds.includes(d.id));
    if (chosen.length === 0) return;
    const folder = exportOne
      ? undefined
      : await open({ directory: true, title: 'Where to save the PDFs' });
    if (!exportOne && typeof folder !== 'string') return;
    exporting = true;
    const back = page;
    try {
      const project = attachProject ? await projectAttachment() : null;
      const pages: string[] = [];
      for (const d of chosen) {
        showDoc(d.id);
        await tick();
        const svg = await pageSvg();
        if (!svg) continue;
        if (exportOne) pages.push(svg);
        else await savePdf(`${folder}/${safeName(named(d))}.pdf`, [svg], project);
      }
      page = back;
      if (exportOne) {
        const path = await saveWithDialog('spectra.pdf', 'PDF', 'pdf', (to) =>
          savePdf(to, pages, project),
        );
        if (path) say(`Saved ${pages.length} pages in ${fileOf(path)}`);
      } else {
        say(`Saved ${chosen.length} PDFs in ${fileOf(String(folder))}`);
      }
      exportDialog?.close();
    } catch (e) {
      fail(String(e));
    } finally {
      page = back;
      exporting = false;
    }
  }

  /** The project file being worked on: opened, or last saved to. Save writes
   *  there without asking, as any document application does; asking every time
   *  made a new file of every save. */
  let projectPath = $state<string>();
  /** Where that project's data was unpacked, a temporary folder. */
  let projectRoot = $state<string>();
  // Nothing open is nothing to save over: a project's file must not be written
  // with whatever is opened next.
  $effect(() => {
    if (docs.length + docs2d.length === 0) projectPath = projectRoot = undefined;
  });

  /** A session the app was running when it stopped without closing, offered on
   *  the start screen until something else is opened. */
  let previousSession = $state<string | null>(null);
  /** Until it has been read, nothing may be kept over it. */
  let sessionRead = $state(false);
  /** How long the session has to stay still before it is kept. */
  const SESSION_DELAY_MS = 2000;

  // The session is kept a little after every change, as a project that points
  // to its data on disk rather than carrying it: copying every FID again (a 2D
  // is a hundred megabytes) at each change would be most of the work done.
  $effect(() => {
    const open = docs.length + docs2d.length;
    if (!sessionRead || (open === 0 && previousSession)) return;
    const manifest = open ? sessionManifest() : null;
    const timer = setTimeout(
      () => keepSession(manifest).catch((e) => fail(String(e))),
      SESSION_DELAY_MS,
    );
    return () => clearTimeout(timer);
  });

  function sessionManifest(): string {
    const others = visible.filter((id) => id !== activeId);
    const showing = currentView ? views.indexOf(currentView) : undefined;
    const project = toProject(docs, activeId, others, stack, views, showing, pageSetup);
    return JSON.stringify({
      ...project,
      documents: project.documents.map((entry, k) => {
        const id = docs[k].id;
        return { ...entry, name: names[id], color: colors[id], structure: structures[id] };
      }),
      documents2d: docs2d.map((d) => ({ path: d.path, peaks: d.peaks, sides: sidesOf(d) })),
      session: { projectPath, root: projectRoot },
    } satisfies Project);
  }

  /** Opens the session kept before the app stopped. Data unpacked from a project
   *  lies in a temporary folder the system may have emptied since, so the project
   *  it came from is unpacked again and those paths pointed there. */
  async function restoreSession() {
    const text = previousSession;
    previousSession = null;
    if (!text) return;
    try {
      const project = parseProject(text);
      const { projectPath: file, root: old } = project.session ?? {};
      const fresh = file && old ? await readProject(file).catch(() => undefined) : undefined;
      const to = fresh?.root;
      const move = <T extends { path: string }>(d: T): T =>
        old && to && d.path.startsWith(old) ? { ...d, path: to + d.path.slice(old.length) } : d;
      await applyProject(
        {
          ...project,
          documents: project.documents.map(move),
          documents2d: project.documents2d?.map(move),
        },
        null,
      );
      projectPath = file;
      projectRoot = to ?? old;
      say('Restored the session from before openNMR stopped');
    } catch (e) {
      fail(String(e));
    }
  }

  /** Writes the project to `path`, with the data of every spectrum and its
   *  structure in it, so it opens on any computer. */
  async function writeProjectTo(path: string) {
    const { manifest, ids, extras } = await projectAttachment();
    await writeProject(path, manifest, ids, extras);
  }

  /** The project as it is saved: its `project.json`, the documents whose data go
   *  with it, and their structures. A PDF carries it too. */
  async function projectAttachment(): Promise<Attachment> {
    // The format lists what is shown besides the active spectrum.
    const others = visible.filter((id) => id !== activeId);
    const showing = currentView ? views.indexOf(currentView) : undefined;
    const ids = docs.map((d) => d.id);
    const inner = await projectLayout([...ids, ...docs2d.map((d) => d.id)]);
    const extras: { name: string; dataUrl: string }[] = [];
    const entries = ids.map((id, k) => {
      const url = structures[id];
      const structure = url ? structureFile(k, url) : undefined;
      if (url && structure) extras.push({ name: structure, dataUrl: url });
      return { path: inner[k], name: names[id], color: colors[id], structure };
    });
    const project = {
      ...bundle(toProject(docs, activeId, others, stack, views, showing, pageSetup), entries),
      documents2d: docs2d.map((d, j) => ({
        path: inner[ids.length + j],
        source: d.path,
        peaks: d.peaks,
        sides: sidesOf(d),
      })),
    };
    return {
      manifest: JSON.stringify(project, null, 2),
      ids: [...ids, ...docs2d.map((d) => d.id)],
      extras,
    };
  }

  /** Saves to the project's own file, or asks for one when there is none yet or
   *  `as` asks for another. */
  async function saveProject(as = false) {
    if (docs.length + docs2d.length === 0) return;
    if (projectPath && !as) {
      try {
        await writeProjectTo(projectPath);
        say(`Saved ${fileOf(projectPath)}`);
      } catch (e) {
        fail(String(e));
      }
      return;
    }
    const name = active ? named(active) : 'project';
    const path = await saveWithDialog(
      `${name}.${PROJECT_EXTENSIONS[0]}`,
      'openNMR project',
      PROJECT_EXTENSIONS[0],
      writeProjectTo,
    );
    if (path) {
      projectPath = path;
      remember(path);
      say(`Saved ${fileOf(path)}`);
    }
  }

  async function openProject(path?: string) {
    try {
      const file =
        path ??
        (await open({
          title: 'Open an openNMR project',
          filters: [{ name: 'openNMR project', extensions: PROJECT_EXTENSIONS }],
        }));
      if (!file) return;
      const { manifest, root } = await readProject(file);
      const project = parseProject(manifest);
      remember(file);
      // A PDF gives the project back but is not where it is saved: Save would
      // otherwise write a project over the figure.
      projectPath = file.toLowerCase().endsWith('.pdf') ? undefined : file;
      projectRoot = root ?? undefined;
      await applyProject(project, root);
    } catch (e) {
      fail(String(e));
    }
  }

  /** Opens what `project` holds, its data under `root` when it carries it. */
  async function applyProject(project: Project, root: string | null) {
    // A project that carries its data names places inside itself; one written
    // before that names places on the disk it was saved on.
    const at = (inner: string) => (root ? `${root}/${inner}` : inner);
    const restored: DocumentInfo[] = [];
    for (const entry of project.documents) {
      // The saved steps replace whatever the spectrum would open with, so the
      // automatic phase and baseline are not worked out only to be thrown away:
      // they were most of the time a project took to load.
      const opened = await openPath(at(entry.path), false, null);
      const doc = opened.documents[0];
      if (!doc) throw new Error(opened.errors[0] ?? `No spectrum found at ${entry.path}`);
      restored.push(await setState(doc.id, entry.operations, entry.analysis));
      if (entry.name) names = { ...names, [doc.id]: entry.name };
      if (entry.color) colors = { ...colors, [doc.id]: entry.color };
      const drawing = entry.structure;
      const type = STRUCTURE_TYPES[drawing?.split('.').pop()?.toLowerCase() ?? ''];
      if (drawing?.startsWith('data:')) await keepUrl(doc.id, drawing);
      else if (drawing && root && type) keepStructure(doc.id, type, await readBytes(at(drawing)));
    }
    const ids = new Set(restored.map((d) => d.id));
    docs = [...docs.filter((d) => !ids.has(d.id)), ...restored];
    const restored2d: Document2DInfo[] = [];
    for (const entry of project.documents2d ?? []) {
      const opened = await openPath(at(entry.path), false, null);
      const doc = opened.documents2d[0];
      if (!doc) throw new Error(opened.errors[0] ?? `No 2D experiment found at ${entry.path}`);
      // Cross peaks were bare `[F2, F1]` pairs before they could be assigned.
      const peaks = (entry.peaks ?? []).map((p: CrossPeak | [number, number]) =>
        Array.isArray(p) ? { at: p, atoms: [] } : p,
      );
      restored2d.push(peaks.length ? await setPeaks2d(doc.id, peaks) : doc);
      // The spectra chosen for its sides come back with it, as a dataset's
      // external projections are kept.
      const sides = (entry.sides ?? []).map((k) =>
        k === 'own' ? null : k === null ? undefined : restored[k]?.id,
      );
      if (sides.length) besides = { ...besides, [doc.id]: sides };
    }
    const ids2d = new Set(restored2d.map((d) => d.id));
    docs2d = [...docs2d.filter((d) => !ids2d.has(d.id)), ...restored2d];
    activeId = restored[project.active]?.id ?? activeId;
    const others = project.overlay.flatMap((i) => restored[i]?.id ?? []);
    views = (project.views ?? []).map((view, i) => ({
      id: i + 1,
      name: view.name,
      ids: view.documents.flatMap((at) => restored[at]?.id ?? []),
      stack: view.stacked,
    }));
    // A project brings the page it was printed on.
    if (project.page) pageSetup = { ...PAGE_SETUP, ...project.page };
    // Older projects stored a boolean, and one version had a cascade mode.
    const saved = project.stacked;
    const savedStack: StackMode =
      typeof saved === 'string' && (STACK_MODES as string[]).includes(saved)
        ? (saved as StackMode)
        : saved
          ? 'stacked'
          : 'overlay';
    const ours = [...new Set([...(activeId === undefined ? [] : [activeId]), ...others])];
    const showing = project.view ?? undefined;
    page = undefined;
    if (showing !== undefined && views[showing]) {
      showView(views[showing].id);
    } else if (ours.length > 1) {
      // A project written before pages kept what it showed in `overlay`, which
      // is a view in all but name.
      const id = Math.max(0, ...views.map((v) => v.id)) + 1;
      const kind = savedStack === 'stacked' ? 'Stack' : 'Superimposed';
      views = [
        ...views,
        { id, name: `${kind} of ${ours.length} spectra`, ids: ours, stack: savedStack },
      ];
      showView(id);
    } else if (activeId !== undefined) {
      showDoc(activeId);
    } else if (restored2d[0]) {
      page = { kind: '2d', id: restored2d[0].id };
    }
    errors = [];
  }

  /** Sends a shortcut to the plot, so palette entries behave exactly like their keys. */
  const press =
    (key: string, init: KeyboardEventInit = {}) =>
    () =>
      window.dispatchEvent(new KeyboardEvent('keydown', { key, ...init }));
  const withCommand = { metaKey: isMac, ctrlKey: !isMac };

  const stackable = $derived(stack === 'stacked' && shown.length > 1);

  const tabList1d = $derived<[typeof tab, string][]>([
    ['processing', 'Processing'],
    ['peaks', 'Peaks'],
    ['integrals', 'Integrals'],
    ['multiplets', 'Multiplets'],
    ['view', 'View'],
    ...(stackable ? ([['stack', 'Stack']] as [typeof tab, string][]) : []),
    ['metadata', 'Metadata'],
  ]);
  const tabList = $derived<[typeof tab, string][]>(
    shown2d
      ? [
          ['peaks', 'Peaks'],
          ['view', 'View'],
          ['metadata', 'Metadata'],
        ]
      : tabList1d,
  );
  /** Tabs that are not a stage of the work: set apart after the stages. */
  const SECONDARY_TABS: (typeof tab)[] = ['view', 'stack', 'metadata'];
  /** Tabs with something of their own in the side panel. On the others (View,
   *  Stack) it keeps what the last of these showed, so the plot keeps its width
   *  and the panel its place: it had vanished, and the plot jumped. */
  const PANEL_TABS: (typeof tab)[] = ['processing', 'peaks', 'integrals', 'multiplets', 'metadata'];
  let lastPanelTab = $state<typeof tab>('processing');
  $effect(() => {
    if (PANEL_TABS.includes(tab)) lastPanelTab = tab;
  });
  const panelTab = $derived.by(() => {
    const wanted = PANEL_TABS.includes(tab) ? tab : lastPanelTab;
    // A 2D has its peaks and its metadata to show, nothing else yet.
    if (shown2d && wanted !== 'peaks' && wanted !== 'metadata') return 'metadata';
    return wanted;
  });
  // Another stage leaves the multiplet gone into for that stage's own part.
  $effect(() => {
    void panelTab;
    editing = undefined;
  });
  /** A 2D has no processing or analysis to offer yet; nothing is shown for what
   *  it cannot do, and the page it would be printed on is not drawn for it. */
  $effect(() => {
    if (!shown2d) return;
    if (tab !== 'view' && tab !== 'peaks' && tab !== 'metadata') tab = 'view';
    if (!['pointer', 'zoom', 'zoomOut', 'pan', 'peakByPeak'].includes(mode)) mode = 'pointer';
    pageMode = false;
  });
  /** The tab pattern's keys: arrows move and select, Home and End jump, and only
   *  the chosen tab is in the Tab order. */
  function tabKey(e: KeyboardEvent) {
    const at = tabList.findIndex(([id]) => id === tab);
    const next =
      e.key === 'ArrowRight'
        ? (at + 1) % tabList.length
        : e.key === 'ArrowLeft'
          ? (at + tabList.length - 1) % tabList.length
          : e.key === 'Home'
            ? 0
            : e.key === 'End'
              ? tabList.length - 1
              : -1;
    if (next < 0) return;
    e.preventDefault();
    tab = tabList[next][0];
    document.getElementById(`tab-${tab}`)?.focus();
  }

  /** Integrates every signal above the threshold, in every spectrum on screen:
   * automatic integration. */
  async function integrateEverywhere() {
    if (analysisBusy) return;
    analysisBusy = true;
    let added = 0;
    let failed = false;
    try {
      for (const original of shown) {
        let doc = original;
        let amend = false;
        try {
          const threshold = doc.noise * noiseFactor;
          if (doc.analysis.peaks.length === 0) {
            doc = await autoPeaks(doc.id, threshold, doc.analysis.peakMethod ?? 'standard');
            amend = true;
            docs = docs.map((d) => (d.id === doc.id ? doc : d));
          }
          if (
            doc.analysis.integrationMethod === 'fitted' &&
            !doc.analysis.peakFit &&
            doc.analysis.peaks.length
          ) {
            doc = await fitPeaks(doc.id, threshold, amend);
            amend = true;
            docs = docs.map((d) => (d.id === doc.id ? doc : d));
          }
          const regions = await signalRegions(doc.id, doc.firstPpm, lastPpm(doc), threshold);
          const current = docs.find((d) => d.id === doc.id);
          if (
            !current ||
            current.revision !== doc.revision ||
            JSON.stringify(current.analysis) !== JSON.stringify(doc.analysis)
          )
            throw new Error('The spectrum changed during automatic integration; try again.');
          let analysis = doc.analysis;
          for (const [from, to] of regions) {
            analysis = addIntegral(analysis, from, to);
          }
          added += analysis.integrals.length - doc.analysis.integrals.length;
          if (analysis !== doc.analysis) {
            const updated = await setAnalysis(doc.id, analysis, amend);
            docs = docs.map((d) => (d.id === updated.id ? updated : d));
          }
        } catch (e) {
          failed = true;
          fail(String(e));
        }
      }
      if (!failed)
        say(`Added ${added} ${added === 1 ? 'integral' : 'integrals'} · ${commandKey}Z undoes it`);
    } finally {
      analysisBusy = false;
    }
  }

  /** Takes every integral off every spectrum on screen. */
  async function clearIntegralsEverywhere() {
    if (analysisBusy) return;
    for (const doc of shown) {
      try {
        const updated = await setAnalysis(doc.id, {
          ...doc.analysis,
          integrals: [],
          reference: null,
        });
        docs = docs.map((d) => (d.id === updated.id ? updated : d));
      } catch (e) {
        fail(String(e));
      }
    }
  }

  /** Reads the integrals already drawn as multiplets, so a range chosen by hand
   * need not be chosen twice. */
  async function multipletsFromIntegrals() {
    if (!active) return;
    try {
      let analysis = active.analysis;
      for (const [from, to] of active.analysis.integrals) {
        analysis = addMultiplet(analysis, from, to);
      }
      const updated = await setAnalysis(active.id, analysis);
      docs = docs.map((d) => (d.id === updated.id ? updated : d));
    } catch (e) {
      fail(String(e));
    }
  }

  /** Reads every signal above the threshold as a multiplet, in every spectrum on
   * screen, leaving out what is not the sample. */
  async function multipletsEverywhere() {
    if (analysisBusy) return;
    for (const doc of shown) {
      try {
        const threshold = doc.noise * noiseFactor;
        const regions = await signalRegions(doc.id, doc.firstPpm, lastPpm(doc), threshold);
        const solvent = findSolvent(doc.solvent, doc.nucleus);
        let analysis = doc.analysis;
        for (const [from, to] of regions) {
          if (isSolvent(from, to, solvent)) continue;
          analysis = addMultiplet(analysis, from, to);
        }
        let updated = await setAnalysis(doc.id, analysis);
        // Read and then fitted, as they are asked for together: what a class and
        // its couplings say is best checked against the spectrum at once. One
        // step, so one Cmd+Z takes both back.
        const { next, fitted } = await fitEvery(updated);
        if (fitted > 0) updated = await setAnalysis(doc.id, next, true);
        docs = docs.map((d) => (d.id === updated.id ? updated : d));
      } catch (e) {
        fail(String(e));
      }
    }
  }

  /** Takes every multiplet off every spectrum on screen. */
  async function clearMultipletsEverywhere() {
    if (analysisBusy) return;
    for (const doc of shown) {
      try {
        const updated = await setAnalysis(doc.id, { ...doc.analysis, multiplets: [] });
        docs = docs.map((d) => (d.id === updated.id ? updated : d));
      } catch (e) {
        fail(String(e));
      }
    }
  }

  /** The spectrum written up as the line that goes into a paper. */
  async function copyReport() {
    if (!active) return;
    try {
      await navigator.clipboard.writeText(report(active, decimals));
    } catch (e) {
      fail(String(e));
    }
  }

  /** A multiplet waiting to be told something off the spectrum: which shift it
   * sits at, or which lines the coupling is between. */
  let picking = $state<{
    at: number;
    want: 'shift' | 'js' | 'from' | 'to' | 'split';
    peaks: number[];
  }>();

  /** Leaves the window and goes to the spectrum, where the answer is. */
  function startPicking(at: number, want: 'shift' | 'js' | 'from' | 'to' | 'split') {
    picking = { at, want, peaks: [] };
    mode = 'pickValue';
  }

  function stopPicking(keep: boolean) {
    const asked = picking;
    picking = undefined;
    mode = 'pointer';
    if (!asked || !active) {
      return;
    }
    if (keep && asked.peaks.length > 0) {
      const analysis = active.analysis;
      const last = asked.peaks[asked.peaks.length - 1];
      updateAnalysis(
        asked.want === 'split'
          ? splitMultiplet(analysis, asked.at, last)
          : asked.want === 'shift'
            ? setMultipletShift(analysis, asked.at, last)
            : asked.want === 'from' || asked.want === 'to'
              ? setMultipletRange(
                  analysis,
                  asked.at,
                  asked.want === 'from' ? last : (analysis.multiplets[asked.at]?.from ?? 0),
                  asked.want === 'to' ? last : (analysis.multiplets[asked.at]?.to ?? 0),
                )
              : setMultipletJs(
                  analysis,
                  asked.at,
                  couplingsBetween(asked.peaks, active.frequencyMhz),
                ),
      );
    }
    editing = asked.at;
  }

  /** Picks peaks in every spectrum on screen, each against its own noise, so a
   * stack can be read across. Peaks belong to a spectrum, so picking for one
   * never gave the others any. */
  async function pickEverywhere() {
    if (analysisBusy) return;
    analysisBusy = true;
    try {
      for (const doc of shown) {
        const updated = await autoPeaks(
          doc.id,
          doc.noise * noiseFactor,
          doc.analysis.peakMethod ?? 'standard',
        );
        docs = docs.map((d) => (d.id === updated.id ? updated : d));
        say(
          updated.analysis.peakMethod === 'deconvolution'
            ? `${peakFitSummary(updated.analysis.peakFit)} · ${commandKey}Z undoes it`
            : `Picked ${updated.analysis.peaks.length} peaks · ${commandKey}Z undoes it`,
        );
      }
    } catch (e) {
      fail(String(e));
    } finally {
      analysisBusy = false;
    }
  }

  async function fitPickedPeaks() {
    if (!active || analysisBusy || active.analysis.peaks.length === 0) return;
    analysisBusy = true;
    try {
      const updated = await fitPeaks(active.id, active.noise * noiseFactor);
      docs = docs.map((d) => (d.id === updated.id ? updated : d));
      say(`${peakFitSummary(updated.analysis.peakFit)} · ${commandKey}Z undoes it`);
    } catch (e) {
      fail(String(e));
    } finally {
      analysisBusy = false;
    }
  }

  /** Clears the peaks of every spectrum on screen. */
  async function clearEverywhere() {
    if (analysisBusy) return;
    for (const doc of shown) {
      try {
        const updated = await setAnalysis(doc.id, {
          ...doc.analysis,
          peaks: [],
          labels: [],
          peakFit: null,
        });
        docs = docs.map((d) => (d.id === updated.id ? updated : d));
      } catch (e) {
        fail(String(e));
      }
    }
  }

  // The panel follows the tool, so choosing a tool shows what belongs with it.
  $effect(() => {
    if (mode === 'integrate') tab = 'integrals';
    else if (mode === 'multiplet') tab = 'multiplets';
    else if (mode === 'peaks' || mode === 'peakByPeak') tab = 'peaks';
    else if (mode === 'reference') tab = 'processing';
  });

  $effect(() => {
    if (tab === 'stack' && !stackable) tab = 'processing';
  });

  const recentNames = $derived(shortPaths(recent));
  const openItems = $derived<MenuItem[]>([
    { label: 'Open folder…', shortcut: `${commandKey}O`, run: chooseFolder },
    { label: 'Open project…', run: () => openProject() },
    ...recent.map((path, i) => ({
      label: recentNames[i],
      title: path,
      separator: i === 0,
      run: () => load(path),
    })),
  ]);

  const moreItems: MenuItem[] = [
    { label: 'Settings…', shortcut: `${commandKey},`, run: () => openSettings() },
    { label: 'Keys and mouse', shortcut: '?', run: () => helpDialog?.showModal() },
  ];

  const exportItems = $derived<MenuItem[]>([
    { label: 'Export page as PDF…', disabled: !active, run: () => void exportPdf() },
    { label: 'Export page as SVG…', disabled: !active, run: () => void exportPageSvg() },
    {
      label: 'Export many pages…',
      disabled: docs.length === 0,
      run: openExportMany,
    },
    {
      label: 'Export the screen as PNG…',
      separator: true,
      disabled: !active,
      run: () => exportView('png'),
    },
    {
      label: 'Export peaks as CSV…',
      separator: true,
      disabled: !active?.analysis.peaks.length,
      run: () => exportCsv('peaks'),
    },
    {
      label: 'Export integrals as CSV…',
      disabled: !active?.analysis.integrals.length,
      run: () => exportCsv('integrals'),
    },
  ]);

  const commands = $derived.by((): PaletteCommand[] => {
    const noPlot = !active;
    // The view's own keys work on a 2D as well.
    const noView = !active && !shown2d;
    return [
      { label: 'Open folder…', shortcut: `${commandKey}O`, run: chooseFolder },
      { label: 'Open project…', run: () => openProject() },
      {
        label: 'Save project',
        shortcut: `${commandKey}S`,
        disabled: docs.length + docs2d.length === 0,
        run: () => saveProject(),
      },
      {
        label: 'Save project as…',
        shortcut: `${commandKey}⇧S`,
        disabled: docs.length + docs2d.length === 0,
        run: () => saveProject(true),
      },
      { label: 'Export page as PDF…', disabled: noPlot, run: () => void exportPdf() },
      { label: 'Export page as SVG…', disabled: noPlot, run: () => void exportPageSvg() },
      { label: 'Export many pages…', disabled: docs.length === 0, run: openExportMany },
      { label: 'Figure', disabled: noPlot, run: () => (pageMode = !pageMode) },
      { label: 'Page setup…', run: openPageSetup },
      { label: 'Export the screen as PNG…', disabled: noPlot, run: () => exportView('png') },
      {
        label: 'Export peaks as CSV…',
        disabled: !active?.analysis.peaks.length,
        run: () => exportCsv('peaks'),
      },
      {
        label: 'Export integrals as CSV…',
        disabled: !active?.analysis.integrals.length,
        run: () => exportCsv('integrals'),
      },
      {
        label: 'Undo',
        shortcut: `${commandKey}Z`,
        disabled: !(active ?? shown2d)?.canUndo,
        run: undo,
      },
      {
        label: 'Redo',
        shortcut: `${commandKey}Y`,
        disabled: !(active ?? shown2d)?.canRedo,
        run: redo,
      },
      { label: 'Auto phase', disabled: noPlot, run: autoPhaseActive },
      {
        label: 'Manual phase',
        shortcut: 'Shift+P',
        disabled: noPlot,
        run: press('P', { shiftKey: true }),
      },
      {
        label: 'Baseline points',
        shortcut: 'Shift+B',
        disabled: noPlot,
        run: press('B', { shiftKey: true }),
      },
      {
        label: 'Auto baseline',
        shortcut: 'B',
        disabled: noPlot,
        run: autoBaseline,
      },
      { label: 'Integrate region', shortcut: 'I', disabled: noPlot, run: press('i') },
      { label: 'Pick in region', shortcut: 'K', disabled: noPlot, run: press('k') },
      {
        label: 'Pick one by one',
        shortcut: `${commandKey}K`,
        disabled: noView,
        run: press('k', withCommand),
      },
      {
        label: 'Auto peaks',
        disabled: noPlot,
        run: () => void pickEverywhere(),
      },
      { label: 'Zoom', shortcut: 'Z', disabled: noView, run: press('z') },
      {
        label: 'Zoom out',
        shortcut: 'Shift+Z',
        disabled: noView,
        run: press('Z', { shiftKey: true }),
      },
      { label: 'Pan', shortcut: 'P', disabled: noView, run: press('p') },
      { label: 'Set range…', shortcut: 'M', disabled: noPlot, run: press('m') },
      { label: 'Full', shortcut: 'F', disabled: noView, run: press('f') },
      { label: 'Fit height', shortcut: 'H', disabled: noView, run: press('h') },
      {
        label: 'Previous zoom',
        shortcut: 'Shift+←',
        disabled: noView,
        run: press('ArrowLeft', { shiftKey: true }),
      },
      {
        label: 'Next zoom',
        shortcut: 'Shift+→',
        disabled: noView,
        run: press('ArrowRight', { shiftKey: true }),
      },
      { label: 'Increase intensity', shortcut: '+', disabled: noView, run: press('+') },
      { label: 'Decrease intensity', shortcut: '−', disabled: noView, run: press('-') },
      { label: 'Crosshair', shortcut: 'C', disabled: noView, run: press('c') },
      { label: 'Select', shortcut: 'Esc', disabled: noPlot, run: press('Escape') },
      {
        label: 'Reset spectrum colours',
        disabled: Object.keys(colors).length === 0,
        run: () => (colors = {}),
      },
      {
        label: 'Reset spectrum positions',
        disabled: Object.keys(display).length === 0,
        run: () => (display = {}),
      },
      { label: 'Show or hide the spectra list', run: () => (listOpen = !listOpen) },
      { label: 'Show or hide the side panel', run: () => (panelOpen = !panelOpen) },
      { label: 'Auto integrals', disabled: noPlot, run: () => void integrateEverywhere() },
      {
        label: 'Fit picked peaks',
        disabled: noPlot || !active?.analysis.peaks.length,
        run: () => void fitPickedPeaks(),
      },
      { label: 'Auto multiplets', disabled: noPlot, run: () => void multipletsEverywhere() },
      {
        label: 'Integrals to multiplets',
        disabled: noPlot,
        run: () => void multipletsFromIntegrals(),
      },
      { label: 'Fit all multiplets', disabled: noPlot, run: () => void fitAll() },
      { label: 'Copy report', disabled: noPlot, run: () => void copyReport() },
      { label: 'Show everything', disabled: noPlot, run: showEverything },
      { label: 'Settings', shortcut: `${commandKey},`, run: openSettings },
    ];
  });

  onMount(() => {
    lastSession()
      .then((text) => (previousSession = text))
      .catch((e) => fail(String(e)))
      .finally(() => (sessionRead = true));
    const unlisten = getCurrentWebview().onDragDropEvent(async (e) => {
      dropping = e.payload.type === 'enter' || e.payload.type === 'over';
      if (e.payload.type === 'drop') {
        for (const path of e.payload.paths) {
          await load(path);
        }
      }
    });
    // macOS keeps Cmd+Z for its own Edit menu, so the menu sends these instead.
    const unlistenMenu = listen<string>('menu', (e) => {
      if (e.payload === 'undo') void undo();
      else if (e.payload === 'redo') void redo();
      else if (e.payload === 'open') void chooseFolder();
    });
    const media = matchMedia('(prefers-color-scheme: dark)');
    const onscheme = () => (systemDark = media.matches);
    media.addEventListener('change', onscheme);
    const onkey = (e: KeyboardEvent) => {
      if (isEditable(e.target) || windowOpen()) return;
      const command = e.metaKey || e.ctrlKey;
      const key = e.key.toLowerCase();
      if (command && e.shiftKey && key === 'p') paletteOpen = true;
      else if (command && key === 'o') void chooseFolder();
      else if (command && key === 's') void saveProject(e.shiftKey);
      else if (command && key === 'z') void (e.shiftKey ? redo() : undo());
      else if (command && key === 'y') void redo();
      else if (command && key === ',') openSettings();
      else if (!command && e.key === '?') helpDialog?.showModal();
      else if (
        e.key === 'Escape' &&
        editing !== undefined &&
        e.target instanceof Element &&
        e.target.closest('.multiplet.detail')
      )
        editing = undefined;
      else if (!command && !e.altKey && !e.shiftKey && key === 'b') void autoBaseline();
      else return;
      e.preventDefault();
    };
    window.addEventListener('keydown', onkey);
    // On the window rather than on the pane: a structure is pasted without
    // having to put the pane in focus first.
    window.addEventListener('paste', pasteStructure);
    return () => {
      void unlisten.then((stop) => stop());
      void unlistenMenu.then((stop) => stop());
      media.removeEventListener('change', onscheme);
      window.removeEventListener('keydown', onkey);
      window.removeEventListener('paste', pasteStructure);
    };
  });
</script>

{#if paletteOpen}
  <Palette {commands} onclose={() => (paletteOpen = false)} />
{/if}

<div class="app" class:dropping>
  <header class="toolbar">
    <Menu items={openItems} title="Open a spectrum or a project" label="Open">
      <FolderOpen size={16} /> <span class="label">Open</span>
      <ChevronDown size={14} />
    </Menu>
    <button
      class="ghost"
      title={projectPath
        ? `Save to ${fileOf(projectPath)} (${commandKey}S)`
        : `Save as a project (${commandKey}S)`}
      disabled={docs.length + docs2d.length === 0}
      aria-label="Save"
      onclick={() => saveProject()}><Save size={16} /> <span class="label">Save</span></button
    >
    <Menu items={exportItems} title="Export the plot or a table" label="Export" disabled={!active}>
      <Download size={16} /> <span class="label">Export</span>
      <ChevronDown size={14} />
    </Menu>
    <span class="separator"></span>
    <button
      class="icon"
      title="Undo ({commandKey}Z)"
      aria-label="Undo"
      disabled={!(active ?? shown2d)?.canUndo}
      onclick={undo}><Undo2 size={16} /></button
    >
    <button
      class="icon"
      title="Redo ({commandKey}Y)"
      aria-label="Redo"
      disabled={!(active ?? shown2d)?.canRedo}
      onclick={redo}><Redo2 size={16} /></button
    >
    <span class="separator"></span>
    {#if pageMode}
      <button
        class="ghost"
        title="Back to the analysis, where the spectrum has the whole window"
        onclick={() => (pageMode = false)}><ArrowLeft size={16} /> Analysis</button
      >
      <strong class="mode-title">Figure</strong>
    {:else}
      <!-- The stages in working order, then what is not a stage of the work. -->
      <div class="tablist" role="tablist" aria-label="Stages">
        {#each tabList as [id, name], i (id)}
          {#if i > 0 && SECONDARY_TABS.includes(id) && !SECONDARY_TABS.includes(tabList[i - 1][0])}
            <span class="separator" role="none"></span>
          {/if}
          <button
            role="tab"
            id="tab-{id}"
            aria-controls="tabpanel"
            aria-selected={tab === id}
            tabindex={tab === id ? 0 : -1}
            onkeydown={tabKey}
            onclick={() => (tab = id)}>{name}</button
          >
        {/each}
      </div>
      <span class="separator"></span>
      <button
        class="ghost"
        title="Make the figure: the plot on its page, as it will be printed, in a view of its own"
        aria-label="Figure"
        disabled={!active}
        onclick={() => (pageMode = true)}><FileText size={16} /> Figure</button
      >
    {/if}
    <span class="spacer"></span>
    <button
      class="search"
      title="Find any command and its shortcut"
      aria-label="Search commands"
      aria-keyshortcuts="Control+Shift+P Meta+Shift+P"
      onclick={() => (paletteOpen = true)}
      ><Search size={14} /> <span class="label">Search commands</span>
      <kbd>{commandKey}⇧P</kbd></button
    >
    <span class="separator"></span>
    {#if !pageMode}
      <button
        class="icon"
        aria-pressed={listOpen}
        title="Show or hide the list of spectra"
        aria-label="Spectra list"
        onclick={() => (listOpen = !listOpen)}><PanelLeft size={16} /></button
      >
    {/if}
    <button
      class="icon"
      aria-pressed={panelOpen}
      title="Show or hide the side panel"
      aria-label="Side panel"
      onclick={() => (panelOpen = !panelOpen)}><PanelRight size={16} /></button
    >
    <Menu class="icon" items={moreItems} title="Settings and help" label="More">
      <Ellipsis size={16} />
    </Menu>
  </header>

  {#if errors.length > 0}
    <div class="errors" role="alert">
      <ul>
        {#each errors.slice(0, SHOWN_ERRORS) as message (message)}
          <li>{message}</li>
        {/each}
        {#if errors.length > SHOWN_ERRORS}
          <li>and {errors.length - SHOWN_ERRORS} more</li>
        {/if}
      </ul>
      <button
        class="ghost"
        title="Copy every message"
        onclick={() => void navigator.clipboard.writeText(errors.join('\n'))}>Copy details</button
      >
      <button class="icon" title="Dismiss" aria-label="Dismiss" onclick={() => (errors = [])}
        ><X size={16} /></button
      >
    </div>
  {/if}

  <!-- The figure is made in a view of its own, the analysis tools out of it. -->
  {#if !pageMode}
    <div class="ribbon">
      <div
        class="controls"
        id="tabpanel"
        role="tabpanel"
        aria-labelledby="tab-{tab}"
        aria-busy={analysisBusy}
      >
        {#if tab === 'processing'}
          <button
            class="primary"
            title="Correct the phase automatically"
            disabled={!active}
            onclick={autoPhaseActive}><WandSparkles size={16} /> Auto phase</button
          >
          <button
            class="ghost"
            title="Phase by hand: drag for PH0, right-drag for PH1 (Shift+P)"
            disabled={!active}
            onclick={press('P', { shiftKey: true })}
            ><SlidersHorizontal size={16} /> Manual phase</button
          >
          <span class="separator"></span>
          <button
            class="ghost"
            title="Flatten the baseline automatically (B)"
            disabled={!active}
            onclick={autoBaseline}><Activity size={16} /> Auto baseline</button
          >
          <button
            class="ghost"
            title="Draw the baseline through points on the spectrum (Shift+B)"
            disabled={!active}
            onclick={press('B', { shiftKey: true })}><Waypoints size={16} /> Baseline points</button
          >
          <span class="separator"></span>
          <button
            class="ghost tool"
            aria-pressed={mode === 'reference'}
            title="Click the solvent line to set the ppm axis by it (L)"
            disabled={!active}
            onclick={() => (mode = 'reference')}><Target size={16} /> Reference to solvent</button
          >
        {:else if tab === 'peaks' && shown2d}
          <button
            class="ghost tool"
            aria-pressed={mode === 'peakByPeak'}
            title="Click cross peaks one at a time ({commandKey}K); right-click one to remove it"
            onclick={() => (mode = 'peakByPeak')}
            ><MousePointerClick size={16} /> Pick one by one</button
          >
          {#if shown2d.peaks.length}
            <span class="separator"></span>
            <button
              class="ghost"
              title="Remove every cross peak picked"
              onclick={() => void change2d((id) => setPeaks2d(id, []))}
              ><Eraser size={16} /> Clear</button
            >
          {/if}
        {:else if tab === 'peaks'}
          <button
            class="primary"
            title="Pick the peaks of every spectrum shown"
            disabled={!active || analysisBusy}
            onclick={() => void pickEverywhere()}><WandSparkles size={16} /> Auto peaks</button
          >
          <select
            aria-label="Peak method"
            title="Method for automatically picking this spectrum"
            value={active?.analysis.peakMethod ?? 'standard'}
            disabled={!active || analysisBusy}
            onchange={(e) =>
              active &&
              updateAnalysis({
                ...active.analysis,
                peakMethod: e.currentTarget.value as NonNullable<Analysis['peakMethod']>,
              })}
          >
            <option value="standard">Standard</option>
            <option value="deconvolution">Deconvolution</option>
          </select>
          <button
            class="ghost"
            title="Fit the accepted peaks without finding additional ones"
            disabled={!active || analysisBusy || active.analysis.peaks.length === 0}
            onclick={() => void fitPickedPeaks()}><Activity size={16} /> Fit picked peaks</button
          >
          <button
            class="ghost tool"
            aria-pressed={mode === 'peaks'}
            title="Drag across a region to pick its peaks (K)"
            disabled={!active || analysisBusy}
            onclick={() => (mode = 'peaks')}><ScanLine size={16} /> Pick in region</button
          >
          <button
            class="ghost tool"
            aria-pressed={mode === 'peakByPeak'}
            title="Click peaks one at a time ({commandKey}K)"
            disabled={!active || analysisBusy}
            onclick={() => (mode = 'peakByPeak')}
            ><MousePointerClick size={16} /> Pick one by one</button
          >
          <button
            class="ghost"
            title="Remove the peaks of every spectrum shown"
            disabled={!active || analysisBusy}
            onclick={() => void clearEverywhere()}><Eraser size={16} /> Clear</button
          >
          <span class="separator"></span>
          <button
            class="ghost"
            aria-pressed={snap}
            title="A click takes the top of the nearest line rather than the pointer"
            disabled={!active || analysisBusy}
            onclick={() => (snap = !snap)}><Magnet size={16} /> Snap</button
          >
          <button
            class="ghost"
            aria-pressed={showAll}
            title="Pick and label every spectrum shown, not only the selected one"
            disabled={!active || analysisBusy}
            onclick={() => (showAll = !showAll)}><Files size={16} /> All spectra</button
          >
        {:else if tab === 'integrals'}
          <button
            class="primary"
            title="Integrate every signal, leaving out the solvent and known impurities"
            disabled={!active || analysisBusy}
            onclick={() => void integrateEverywhere()}
            ><WandSparkles size={16} /> Auto integrals</button
          >
          <select
            aria-label="Integration method"
            title="How areas are measured for this spectrum"
            value={active?.analysis.integrationMethod ?? 'sum'}
            disabled={!active || analysisBusy}
            onchange={(e) =>
              active &&
              updateAnalysis({
                ...active.analysis,
                integrationMethod: e.currentTarget.value as NonNullable<
                  Analysis['integrationMethod']
                >,
              })}
          >
            <option value="sum">Sum</option>
            <option value="fitted">Fitted areas</option>
          </select>
          <button
            class="ghost tool"
            aria-pressed={mode === 'integrate'}
            title="Drag across a signal to integrate it (I)"
            disabled={!active || analysisBusy}
            onclick={() => (mode = 'integrate')}><Sigma size={16} /> Integrate region</button
          >
          <button
            class="ghost"
            title="Remove every integral of every spectrum shown"
            disabled={!active || analysisBusy}
            onclick={() => void clearIntegralsEverywhere()}><Eraser size={16} /> Clear</button
          >
          <span class="separator"></span>
          <button
            class="ghost"
            title="Turn each integral into a multiplet"
            disabled={!active || active.analysis.integrals.length === 0}
            onclick={() => void multipletsFromIntegrals()}
            ><Replace size={16} /> Integrals to multiplets</button
          >
        {:else if tab === 'multiplets'}
          <button
            class="primary"
            title="Find and fit every multiplet, leaving out the solvent and known impurities"
            disabled={!active}
            onclick={() => void multipletsEverywhere()}
            ><WandSparkles size={16} /> Auto multiplets</button
          >
          <button
            class="ghost tool"
            aria-pressed={mode === 'multiplet'}
            title="Drag across the lines of one signal (J)"
            disabled={!active}
            onclick={() => (mode = 'multiplet')}><Brackets size={16} /> Mark multiplet</button
          >
          <button
            class="ghost"
            title="Remove every multiplet of every spectrum shown"
            disabled={!active}
            onclick={() => void clearMultipletsEverywhere()}><Eraser size={16} /> Clear</button
          >
          <button
            class="ghost"
            title="Fit shift, couplings and line width of every multiplet to the spectrum"
            disabled={!active || active.analysis.multiplets.length === 0}
            onclick={() => void fitAll()}><Sparkles size={16} /> Fit all</button
          >
          <span class="separator"></span>
          <button
            class="ghost"
            title="Copy the multiplets as a line for a paper, e.g. δ 7.26 (d, J = 8.0 Hz, 2H)"
            disabled={!active}
            onclick={() => void copyReport()}><ClipboardCopy size={16} /> Copy report</button
          >
        {:else if tab === 'view'}
          <button
            class="ghost tool"
            aria-pressed={mode === 'pointer'}
            title="Select and move things (Esc)"
            disabled={!active && !shown2d}
            onclick={() => (mode = 'pointer')}><MousePointer2 size={16} /> Select</button
          >
          <button
            class="ghost tool"
            aria-pressed={mode === 'zoom'}
            title="Drag to zoom; press again for horizontal, vertical or box (Z)"
            disabled={!active && !shown2d}
            onclick={() => ([mode, zoomAxis] = pressZoom(mode, zoomAxis))}
            ><ZoomIn size={16} /> Zoom {AXIS_LABEL[zoomAxis]}</button
          >
          <button
            class="ghost tool"
            aria-pressed={mode === 'zoomOut'}
            title="Click to zoom out (Shift+Z)"
            disabled={!active && !shown2d}
            onclick={() => (mode = 'zoomOut')}><ZoomOut size={16} /> Zoom out</button
          >
          <button
            class="ghost tool"
            aria-pressed={mode === 'pan'}
            title="Drag to move the view; press again for both, horizontal or vertical (P)"
            disabled={!active && !shown2d}
            onclick={() => ([mode, panAxis] = pressPan(mode, panAxis))}
            ><Hand size={16} /> Pan {AXIS_LABEL[panAxis]}</button
          >
          <button
            class="ghost"
            aria-pressed={crosshair}
            title="Crosshair lines (C)"
            disabled={!active && !shown2d}
            onclick={() => (crosshair = !crosshair)}><Crosshair size={16} /> Crosshair</button
          >
          <span class="separator"></span>
          <button
            class="ghost"
            title="Whole spectrum (F)"
            disabled={!active && !shown2d}
            onclick={press('f')}><Maximize2 size={16} /> Full</button
          >
          {#if shown2d}
            <button
              class="ghost"
              title="Contour levels back to where they started (H)"
              onclick={press('h')}><MoveVertical size={16} /> Reset levels</button
            >
          {:else}
            <button
              class="ghost"
              title="Fit the highest peak (H)"
              disabled={!active}
              onclick={press('h')}><MoveVertical size={16} /> Fit height</button
            >
            <button
              class="ghost"
              title="Type the two ends of the view in ppm (M)"
              disabled={!active}
              onclick={press('m')}><Ruler size={16} /> Set range…</button
            >
          {/if}
          <span class="separator"></span>
          <button
            class="icon"
            title="Previous zoom (Shift+←)"
            aria-label="Previous zoom"
            disabled={!active && !shown2d}
            onclick={press('ArrowLeft', { shiftKey: true })}><ChevronLeft size={16} /></button
          >
          <button
            class="icon"
            title="Next zoom (Shift+→)"
            aria-label="Next zoom"
            disabled={!active && !shown2d}
            onclick={press('ArrowRight', { shiftKey: true })}><ChevronRight size={16} /></button
          >
          <button
            class="icon"
            title={shown2d ? 'Lower contours (+)' : 'Taller peaks (+)'}
            aria-label="Increase intensity"
            disabled={!active && !shown2d}
            onclick={press('+')}><Plus size={16} /></button
          >
          <button
            class="icon"
            title={shown2d ? 'Higher contours (−)' : 'Shorter peaks (−)'}
            aria-label="Decrease intensity"
            disabled={!active && !shown2d}
            onclick={press('-')}><Minus size={16} /></button
          >
        {:else if tab === 'metadata'}
          <button
            class="ghost"
            aria-pressed={choosingMeta}
            title="Tick the rows worth keeping; the rest are hidden until shown again"
            onclick={() => (choosingMeta = !choosingMeta)}
            ><ListChecks size={16} /> Choose rows</button
          >
          <button
            class="ghost"
            title="Show every row again"
            disabled={metaKeys.length === 0}
            onclick={showAllMeta}><Eye size={16} /> Show all</button
          >
          {#if active && metaKeys.length > 0 && !choosingMeta}
            <span class="hint">{metaKeys.length} of {recorded.length} rows shown</span>
          {/if}
        {:else if tab === 'stack'}
          <label title="90° stacks the spectra straight up, 0° lays them side by side">
            Angle
            <input type="range" min="0" max="90" step="1" bind:value={stackAngle} />
            <output>{stackAngle}°</output>
          </label>
          <button
            class="ghost"
            aria-pressed={autoScale}
            title="Give every spectrum its own scale instead of one for the set"
            onclick={() => (autoScale = !autoScale)}><Scale size={16} /> Own scale</button
          >
          <button
            class="ghost"
            aria-pressed={clipped}
            title="Cut every spectrum off at its own row"
            onclick={() => (clipped = !clipped)}><Crop size={16} /> Clip rows</button
          >
          <span class="separator"></span>
          <button
            class="ghost"
            aria-pressed={adjusting}
            title="Drag a spectrum to move it, Alt with the wheel to scale it"
            onclick={() => (adjusting = !adjusting)}><Move size={16} /> Adjust spectra</button
          >
          <button
            class="ghost"
            title="Put every spectrum back"
            disabled={!adjusted}
            onclick={resetShifts}><RotateCcw size={16} /> Reset positions</button
          >
          <button
            class="ghost"
            title="Undo every scaling"
            disabled={!adjusted}
            onclick={resetIntensities}><RotateCcw size={16} /> Reset heights</button
          >
        {/if}
      </div>
    </div>
  {/if}

  <div class="body">
    {#if listOpen && !pageMode}
      <aside class="list" style:width="{listWidth}px">
        <h2 class="column-title section-label">Spectra</h2>
        {#if selected.length > 0}
          <div class="selection">
            <span class="hint"
              >{selected.length === 1
                ? 'Tick another to combine'
                : `${selected.length} ticked`}</span
            >
            <button
              class="icon"
              title="Stack the ticked spectra"
              aria-label="Stack"
              disabled={selected.length < 2}
              onclick={() => saveView('stacked')}><Rows3 size={16} /></button
            >
            <button
              class="icon"
              title="Superimpose the ticked spectra"
              aria-label="Superimpose"
              disabled={selected.length < 2}
              onclick={() => saveView('overlay')}><Layers size={16} /></button
            >
            <button
              class="icon"
              title="Show the ticked spectra side by side, one plot each"
              aria-label="Split"
              disabled={selected.length < 2}
              onclick={() => saveView('split')}><Columns2 size={16} /></button
            >
            <button
              class="icon"
              title="Untick all"
              aria-label="Untick all"
              onclick={() => (selected = [])}><X size={16} /></button
            >
          </div>
        {/if}
        <div class="rows">
          {#each views as view (view.id)}
            {@const open = view.ids.filter((id) => docs.some((d) => d.id === id)).length}
            <div class="doc-row">
              <div
                class="doc view"
                class:current={view.id === viewId}
                role="button"
                tabindex="0"
                aria-current={view.id === viewId ? 'true' : undefined}
                title={open === 0
                  ? 'None of these spectra are open any more'
                  : 'Show these spectra again, arranged as they were\nDouble click the name to rename it'}
                onclick={() => showView(view.id)}
                onkeydown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') showView(view.id);
                }}
                ondblclick={() => (renaming = { kind: 'view', id: view.id })}
              >
                {#if isRenaming('view', view.id)}
                  <input
                    class="name quiet-field"
                    aria-label="Name of {view.name}"
                    value={view.name}
                    use:takeCursor
                    onblur={(e) => finishRename('view', view.id, e.currentTarget.value)}
                    onkeydown={(e) => nameKeys(e, view.name)}
                  />
                {:else}
                  <span class="name">{view.name}</span>
                {/if}
                <small>{ARRANGEMENT[view.stack]} · {open} of {view.ids.length} open</small>
              </div>
              <button
                class="icon"
                title="Forget this view"
                aria-label="Forget {view.name}"
                onclick={() => {
                  views = views.filter((v) => v.id !== view.id);
                  if (viewId === view.id) page = undefined;
                }}><X size={14} /></button
              >
            </div>
          {/each}
          {#each docs as doc (doc.id)}
            <div class="doc-row">
              <input
                type="checkbox"
                title="Choose this spectrum for a new view; the plot does not change"
                aria-label="Choose {doc.name}"
                checked={selected.includes(doc.id)}
                onchange={() => toggleSelected(doc.id)}
              />
              <div
                class="doc"
                class:current={page?.kind === 'doc' && page.id === doc.id}
                class:active={doc.id === activeId}
                role="button"
                tabindex="0"
                aria-current={page?.kind === 'doc' && page.id === doc.id ? 'true' : undefined}
                title={`${doc.path}\n${doc.points.toLocaleString()} points\nDouble-click to rename`}
                onclick={(e) => pickRow(e, doc.id)}
                onkeydown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') showDoc(doc.id);
                }}
                ondblclick={() => (renaming = { kind: 'doc', id: doc.id })}
              >
                {#if isRenaming('doc', doc.id)}
                  <input
                    class="name quiet-field"
                    aria-label="Name of {named(doc)}"
                    value={named(doc)}
                    use:takeCursor
                    onblur={(e) => finishRename('doc', doc.id, e.currentTarget.value)}
                    onkeydown={(e) => nameKeys(e, named(doc))}
                  />
                {:else}
                  <span class="name">{named(doc)}</span>
                {/if}
                <small>
                  {nucleusLabel(doc.nucleus)} · {Math.round(doc.frequencyMhz)} MHz{doc.solvent
                    ? ` · ${doc.solvent}`
                    : ''}
                </small>
              </div>
              <input
                type="color"
                class="swatch"
                title="Colour of this spectrum"
                aria-label="Colour of {doc.name}"
                value={colors[doc.id] ?? autoColors[doc.id] ?? traceColors[0]}
                onchange={(e) => (colors = { ...colors, [doc.id]: e.currentTarget.value })}
              />
              <button
                class="icon close"
                title="Close {doc.name}"
                aria-label="Close {doc.name}"
                onclick={() => void closeDoc(doc.id)}><X size={14} /></button
              >
            </div>
          {/each}
          {#each docs2d as doc (doc.id)}
            <div class="doc-row two-d">
              <div
                class="doc"
                class:current={shown2d?.id === doc.id}
                role="button"
                tabindex="0"
                aria-current={shown2d?.id === doc.id ? 'true' : undefined}
                title={`${doc.path}\n${doc.points[0].toLocaleString()} × ${doc.points[1].toLocaleString()} points`}
                onclick={() => (page = { kind: '2d', id: doc.id })}
                onkeydown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') page = { kind: '2d', id: doc.id };
                }}
              >
                <span class="name">{doc.name}</span>
                <small>
                  {nucleusLabel(doc.nuclei[0])}–{nucleusLabel(doc.nuclei[1])} · {Math.round(
                    doc.frequenciesMhz[0],
                  )} MHz{doc.solvent ? ` · ${doc.solvent}` : ''}
                </small>
              </div>
              <button
                class="icon close"
                title="Close {doc.name}"
                aria-label="Close {doc.name}"
                onclick={() => void closeDoc2d(doc.id)}><X size={14} /></button
              >
            </div>
          {/each}
          {#if docs.length + docs2d.length === 0}
            <p class="empty">Nothing open yet.</p>
          {/if}
        </div>
        {#if holder}
          <section
            class="structure"
            aria-label="Structure"
            title="Paste a structure here, or choose one"
            onmouseenter={() => paster?.focus()}
          >
            {#if structure}
              {#if molecule}
                <MoleculeView
                  {molecule}
                  style={structureStyle}
                  mine={editing === undefined || !active
                    ? []
                    : (active.analysis.multiplets[editing]?.atoms ?? [])}
                  assigned={assignedTo}
                  {armed}
                  linked={linkedAtoms}
                  onpick={pickAtom}
                  onhover={(atom) => (hoverAtom = atom)}
                />
                {#if armed.length}
                  <span class="arm">
                    {armed.reduce((n, i) => n + (molecule.atoms[i]?.h ?? 0), 0)} H · click its
                    {shown2d ? 'cross peak' : 'signal on the spectrum'} · Esc
                  </span>
                {/if}
              {:else if structure.startsWith('data:application/pdf')}
                <embed
                  class:invert={dark}
                  src={structure}
                  type="application/pdf"
                  title="Structure"
                />
              {:else}
                <img class:invert={dark} src={structure} alt="Structure of {named(holder)}" />
              {/if}
              <button
                class="icon remove"
                title="Take this structure away"
                aria-label="Take this structure away"
                onclick={dropStructure}><X size={14} /></button
              >
            {:else}
              <span class="hint">Structure</span>
              <button
                class="icon choose"
                title="Paste a structure copied from a drawing program"
                aria-label="Paste structure"
                onclick={() => void pasteFromPasteboard()}><ClipboardPaste size={16} /></button
              >
              <button
                class="icon choose"
                title="Choose a molfile, picture or PDF of the structure"
                aria-label="Choose structure file"
                onclick={() => void chooseStructure()}><ImagePlus size={16} /></button
              >
            {/if}
            <!-- A paste is handed to whatever can be typed into, so this layer can
                 be, and takes it. Nothing typed is kept. -->
            <div
              class="paster"
              bind:this={paster}
              contenteditable="true"
              role="textbox"
              tabindex="0"
              aria-label="Paste a structure"
              onpaste={pasteStructure}
              onbeforeinput={(e) => {
                if (!e.inputType.startsWith('insertFromPaste')) e.preventDefault();
              }}
            ></div>
          </section>
        {/if}
      </aside>
      <button
        class="splitter"
        aria-label="Width of the spectra column"
        title="Drag to give the list more room, or less"
        onpointerdown={(e) => dragWidth(e, listWidth, setListWidth, 1, saveColumns)}
        onkeydown={(e) => {
          if (e.key === 'ArrowLeft') nudgeColumn(setListWidth, listWidth, -16);
          else if (e.key === 'ArrowRight') nudgeColumn(setListWidth, listWidth, 16);
        }}
      ></button>
    {/if}
    <div class="plots">
      {#if active || shown2d}
        <!-- What the plot draws, in one place whatever tab is open, on the plot's
             edge rather than over its data; the pointer's reading at the end. -->
        <div class="draw-strip">
          {#if active}
            <div class="draws" role="group" aria-label="Draw on the plot">
              <button
                class="ghost"
                aria-pressed={showLabels}
                title="Draw the peak labels"
                onclick={() => (showLabels = !showLabels)}><Tag size={14} /> Labels</button
              >
              <button
                class="ghost"
                aria-pressed={showIntegrals}
                title="Draw the integrals: brackets and values"
                onclick={() => (showIntegrals = !showIntegrals)}
                ><Sigma size={14} /> Integrals</button
              >
              <button
                class="ghost"
                aria-pressed={showCurves}
                title="Draw running integral curves for Sum areas"
                disabled={!showIntegrals}
                onclick={() => (showCurves = !showCurves)}><ChartSpline size={14} /> Curves</button
              >
              <button
                class="ghost"
                aria-pressed={showMultiplets}
                title="Draw each multiplet's range and name"
                onclick={() => (showMultiplets = !showMultiplets)}
                ><Brackets size={14} /> Multiplets</button
              >
              <button
                class="ghost"
                aria-pressed={showSimulation}
                title="Draw the lines each fitted multiplet should have"
                onclick={() => (showSimulation = !showSimulation)}
                ><ChartNoAxesColumn size={14} /> Fitted lines</button
              >
            </div>
          {:else if shown2d}
            <div class="draws" role="group" aria-label="Draw on the plot">
              <button
                class="ghost"
                aria-pressed={showProjections}
                title="Draw a spectrum along each side of the 2D"
                onclick={() => (showProjections = !showProjections)}
                ><PanelsLeftBottom size={14} /> Sides</button
              >
              {#if showProjections}
                {#each [0, 1] as const as dim (dim)}
                  {@const side = sideOf(shown2d, dim)}
                  <Menu
                    items={sideItems(dim)}
                    title="What is drawn along {dim === 0
                      ? 'the top'
                      : 'the left'}: the 2D's own projection or an open spectrum"
                  >
                    {nucleusLabel(shown2d.nuclei[dim])}: {side === 'own'
                      ? 'projection'
                      : named(side)}
                    <ChevronDown size={14} />
                  </Menu>
                {/each}
              {/if}
            </div>
          {/if}
          <span class="spacer"></span>
          <span class="readout">{readout}</span>
        </div>
      {/if}
      <div class="tool-lane">
        {#if notice || ((active || shown2d) && mode !== 'pointer' && toolHint(mode, snap))}
          {@const text = notice ?? toolHint(mode, snap)}
          <div class="tool-hint" class:notice role="status">
            <span>{text}</span>
            {#if !notice}
              <button
                class="ghost"
                title="Back to selecting (Esc)"
                onclick={() => (mode = 'pointer')}>Done</button
              >
            {/if}
          </div>
        {/if}
        {#if picking}
          <div class="picking">
            <span>
              {picking.want === 'split'
                ? 'Click where to cut this multiplet in two.'
                : picking.want === 'shift'
                  ? 'Click the line this multiplet is quoted at.'
                  : picking.want === 'from'
                    ? 'Click the line the range starts at.'
                    : picking.want === 'to'
                      ? 'Click the line the range ends at.'
                      : 'Click the lines the coupling is between, two or more.'}
            </span>
            <output>
              {picking.peaks.length === 0
                ? 'none yet'
                : picking.peaks.map((ppm) => fixed(ppm, decimals)).join(', ')}
            </output>
            <button onclick={() => stopPicking(false)}>Cancel</button>
            <button
              class="primary"
              disabled={picking.peaks.length < (picking.want === 'js' ? 2 : 1)}
              onclick={() => stopPicking(true)}>OK</button
            >
          </div>
        {/if}
      </div>
      <main
        class:split={split && !pageMode}
        class:desk={pageMode}
        bind:clientWidth={deskWidth}
        bind:clientHeight={deskHeight}
      >
        {#if shown2d}
          <Plot2D
            doc={shown2d}
            {projections}
            onpickpeak={(at, radius) => void pickCrossPeak(at, radius)}
            onremovepeak={(index) => void removeCrossPeak(index)}
            armed={armed.length > 0}
            marked={markedPeak}
            onassignpeak={assignArmedPeak}
            onhoverpeak={(index) => (hoverPeak = index)}
            palette={traceColors}
            {decimals}
            onerror={fail}
            onreadout={(text) => (readout = text)}
            bind:mode
            bind:zoomAxis
            bind:panAxis
            bind:crosshair
          />
        {:else if active}
          <!-- In page mode the plot is laid out at the size of its page and shown
               enlarged to fit; otherwise these two boxes are not there at all. -->
          {@const perMm = pixelsPerMm(pageSetup)}
          {@const plot = plotSize(pageSetup)}
          <div
            class="paper"
            class:on={pageMode}
            style:width={pageMode ? `${pageSetup.width * perMm * pageScale}px` : null}
            style:height={pageMode ? `${pageSetup.height * perMm * pageScale}px` : null}
            style:background={pageMode ? pageSetup.background : null}
          >
            <div
              class="page-plot"
              class:on={pageMode}
              style:left={pageMode ? `${pageSetup.margin * perMm * pageScale}px` : null}
              style:top={pageMode ? `${pageSetup.margin * perMm * pageScale}px` : null}
              style:width={pageMode ? `${plot.width * pageScale}px` : null}
              style:height={pageMode ? `${plot.height * pageScale}px` : null}
            >
              {#each paneList as pane (pane.key)}
                <Plot
                  bind:this={panes[pane.key]}
                  zoom={pageMode ? pageScale : 1}
                  textSizes={pageMode ? textSizesOf(pageSetup) : screenSizes}
                  baselineAt={(pageMode ? pageSetup.baselineGap : baselineGap) / 100}
                  axisTitle={pageMode ? pageSetup.axisTitle || undefined : 'ppm'}
                  ink={pageMode ? pageInk : screenInk}
                  palette={!pageMode || pageSetup.traces === 'spectra'
                    ? pane.palette
                    : [pageSetup.traces === 'one' ? pageSetup.traceColour : '#000000']}
                  docs={pane.docs}
                  active={pane.active}
                  stack={pane.stack}
                  threshold={pane.threshold}
                  focused={pane.focused}
                  onfocus={() => activate(pane.active.id)}
                  onerror={fail}
                  onphase={applyPhase}
                  onbaselinepoints={applyBaselinePoints}
                  onrange={(tool, from, to) =>
                    ({ integrate, multiplet: addMultipletRange, peaks: pickRange })[tool](from, to)}
                  onpickpoint={pickPoint}
                  onpickvalue={(ppm) =>
                    picking && (picking = { ...picking, peaks: [...picking.peaks, ppm] })}
                  {shifts}
                  onopenmultiplet={(index) => (editing = index)}
                  onhovermultiplet={(index) => (hoverSignal = index)}
                  onassign={assignArmed}
                  marked={markedSignal}
                  armed={armed.length > 0}
                  onshift={shiftAnnotation}
                  onmove={(kind, index, from, to) =>
                    (kind === 'integral' ? moveIntegral : moveMultiplet)(index, from, to)}
                  {display}
                  onmovetrace={moveTrace}
                  onscaletrace={scaleTrace}
                  onthreshold={(factor) => (noiseFactor = factor)}
                  angle={stackAngle}
                  {autoScale}
                  {adjusting}
                  {clipped}
                  {showAll}
                  {showLabels}
                  snap={snap && picking?.want !== 'split'}
                  {showCurves}
                  {showMultiplets}
                  {showIntegrals}
                  {multipletLabel}
                  {showSimulation}
                  {decimals}
                  colorOverrides={!pageMode || pageSetup.traces === 'spectra' ? traceColorOf : {}}
                  {names}
                  onactivate={activate}
                  onreference={(from, to) => void reference(from, to)}
                  onremove={drop}
                  onlabelpeak={labelPeak}
                  onreadout={(text) => (readout = text)}
                  onoverlap={(count) => (crowdedNames = count)}
                  bind:mode
                  bind:zoomAxis
                  bind:panAxis
                  bind:crosshair
                />
              {/each}
            </div>
          </div>
        {:else}
          <div class="placeholder">
            <div class="start">
              {#if previousSession}
                <h2>openNMR stopped before it was closed</h2>
                <p class="hint">
                  What was open then, with its processing and analysis, can be opened again.
                </p>
                <div class="start-actions">
                  <button class="primary" onclick={restoreSession}>Restore session</button>
                </div>
              {/if}
              <h2>Open a spectrum to begin</h2>
              <p class="hint">
                Choose a Bruker experiment folder (the one holding <code>fid</code>), or a folder of
                experiments, or drop one anywhere in this window.
              </p>
              <div class="start-actions">
                <button class={previousSession ? undefined : 'primary'} onclick={chooseFolder}
                  >Open folder…</button
                >
                <button onclick={() => openProject()}>Open project…</button>
              </div>
              {#if recent.length > 0}
                <h3 class="section-label">Recent</h3>
                <ul class="recent">
                  {#each recent as path, i (path)}
                    <li>
                      <button title={path} onclick={() => void load(path)}>{recentNames[i]}</button>
                    </li>
                  {/each}
                </ul>
              {/if}
              <p class="hint">
                Then work left to right along the tabs: Processing, Peaks, Integrals, Multiplets.
              </p>
            </div>
          </div>
        {/if}
      </main>
    </div>
    {#if (active || shown2d) && panelOpen}
      <button
        class="splitter"
        aria-label="Width of the panel"
        title="Drag to give the panel more room, or less"
        onpointerdown={(e) => dragWidth(e, panelWidth, setPanelWidth, -1, saveColumns)}
        onkeydown={(e) => {
          if (e.key === 'ArrowLeft') nudgeColumn(setPanelWidth, panelWidth, 16);
          else if (e.key === 'ArrowRight') nudgeColumn(setPanelWidth, panelWidth, -16);
        }}
      ></button>
      <aside class="side" style:width="{panelWidth}px">
        <!-- Named for what it is about, the spectrum: which part of it is shown
             is the stage's underline above, not said twice. -->
        <header class="column-title inspector-title">
          <h2>{active ? named(active) : (shown2d?.name ?? '')}</h2>
        </header>
        {#if pageMode && crowdedNames > 0}
          <p class="hint crowded" role="status">
            {crowdedNames === 1
              ? 'One multiplet name overlaps'
              : `${crowdedNames} multiplet names overlap`}
            another: widen the page, shrink the names under Multiplets, or hide some.
          </p>
        {/if}
        {#if pageMode}
          <PageSetupEditor
            setup={pageSetup}
            onchange={(setup) => (pageSetup = setup)}
            attach={attachProject}
            onattach={(on) => (attachProject = on)}
          />
        {:else if panelTab === 'processing' && active}
          <Processing
            doc={active}
            onchange={apply}
            onmanualphase={press('P', { shiftKey: true })}
            onbaselinepoints={press('B', { shiftKey: true })}
          />
        {:else if panelTab === 'metadata'}
          <section class="recorded">
            <table>
              <colgroup>
                {#if choosingMeta}<col style="width: 22px" />{/if}
                <col style="width: {metaLabelWidth}px" />
                <col />
              </colgroup>
              <thead>
                <tr>
                  {#if choosingMeta}<th></th>{/if}
                  <th>
                    Parameter
                    <button
                      class="grip"
                      aria-label="Width of the parameter column"
                      title="Drag to give the names more room, or less"
                      onpointerdown={(e) =>
                        dragWidth(e, metaLabelWidth, setMetaWidth, 1, saveMetaWidth)}
                      onkeydown={(e) => {
                        if (e.key === 'ArrowLeft') nudgeMetaWidth(-8);
                        else if (e.key === 'ArrowRight') nudgeMetaWidth(8);
                      }}
                    ></button>
                  </th>
                  <th class="value">Value</th>
                </tr>
              </thead>
              <tbody>
                {#each shownMeta as [label, value], i (i)}
                  <tr>
                    {#if choosingMeta}
                      <td>
                        <input
                          type="checkbox"
                          aria-label="Keep {label}"
                          disabled={label === ''}
                          checked={metaKeys.includes(label)}
                          onchange={() => toggleMeta(label)}
                        />
                      </td>
                    {/if}
                    <td class="key" title={label}>{label}</td><td class="value">{value}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
            {#if recorded.length === 0}
              <p class="hint">Nothing was read from this dataset's parameter files.</p>
            {/if}
          </section>
        {:else if shown2d}
          <section class="recorded">
            <table>
              <thead>
                <tr>
                  <th style="width: 28px">#</th>
                  <th class="value">{nucleusLabel(shown2d.nuclei[0])} ppm</th>
                  <th class="value">{nucleusLabel(shown2d.nuclei[1])} ppm</th>
                  <th style="width: 28px"></th>
                </tr>
              </thead>
              <tbody>
                {#each shown2d.peaks as { at: [f2, f1], atoms }, k (k)}
                  {@const x = f2.toFixed(placesFor(shown2d.nuclei[0], decimals))}
                  {@const y = f1.toFixed(placesFor(shown2d.nuclei[1], decimals))}
                  <tr
                    class:lit={k === hoverPeak || k === markedPeak}
                    title={atoms.length ? 'Assigned on the structure' : undefined}
                  >
                    <td class="key">{k + 1}{atoms.length ? ' ✓' : ''}</td>
                    <td class="value">{x}</td>
                    <td class="value">{y}</td>
                    <td>
                      <button
                        class="icon"
                        title="Remove this cross peak"
                        aria-label="Remove cross peak at {x}, {y}"
                        onclick={() => void removeCrossPeak(k)}><X size={14} /></button
                      >
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
            {#if shown2d.peaks.length === 0}
              <p class="hint">Pick cross peaks one by one ({commandKey}K) to list them here.</p>
            {/if}
          </section>
        {:else if active && editing !== undefined && active.analysis.multiplets[editing]}
          {@const doc = active}
          {@const at = editing}
          {@const m = doc.analysis.multiplets[at]}
          {@const info = doc.multipletInfo[at]}
          <section class="multiplet detail" aria-label="Multiplet {m.name ?? letterOf(at)}">
            <header>
              <button
                class="ghost back"
                title="Back to the table (Esc)"
                onclick={() => (editing = undefined)}><ChevronLeft size={16} /> Multiplets</button
              >
              <h3>{m.name ?? letterOf(at)}</h3>
              <span class="spacer"></span>
              <button
                class="icon"
                tabindex="-1"
                title="Previous multiplet"
                aria-label="Previous multiplet"
                disabled={at === 0}
                onclick={() => (editing = at - 1)}><ChevronLeft size={16} /></button
              >
              <span class="hint">{at + 1} of {doc.analysis.multiplets.length}</span>
              <button
                class="icon"
                tabindex="-1"
                title="Next multiplet"
                aria-label="Next multiplet"
                disabled={at === doc.analysis.multiplets.length - 1}
                onclick={() => (editing = at + 1)}><ChevronRight size={16} /></button
              >
              <button
                class="icon"
                tabindex="-1"
                title="Show this multiplet on the plot"
                aria-label="Show this multiplet on the plot"
                onclick={() =>
                  panes[split ? (activeId ?? SINGLE) : SINGLE]?.showRange(m.from, m.to)}
                ><ZoomIn size={16} /></button
              >
            </header>
            <div class="detail-body">
              <!-- What the window exists to produce: the line a paper quotes. Set to be
                 read and taken, not to be typed into. -->
              <div class="output">
                <p>{info ? describeMultiplet(info, nucleiOf(m, info), decimals, m) : ''}</p>
                <button
                  class="icon"
                  title="Copy this line"
                  aria-label="Copy this line"
                  onclick={() =>
                    info &&
                    void navigator.clipboard.writeText(
                      describeMultiplet(info, nucleiOf(m, info), decimals, m),
                    )}><Copy size={14} /></button
                >
              </div>
              <section>
                <h3 class="section-label">Signal</h3>
                <label title="What this multiplet is called in the report">
                  <span class="name">Name</span>
                  <span class="control">
                    <input
                      class="text"
                      value={m.name ?? letterOf(at)}
                      onchange={(e) =>
                        updateAnalysis(
                          setMultipletName(doc.analysis, at, e.currentTarget.value.trim()),
                        )}
                    />
                  </span>
                  <span class="actions"></span>
                </label>
                <label title="The class it is quoted as; empty reads the lines again">
                  <span class="name">Class</span>
                  <span class="control">
                    <select
                      value={m.kind ?? info?.kind ?? 'm'}
                      onchange={(e) =>
                        updateAnalysis(setMultipletKind(doc.analysis, at, e.currentTarget.value))}
                    >
                      <option value="">read from the lines</option>
                      {#each CLASSES as name (name)}
                        <option value={name}>{name}</option>
                      {/each}
                    </select>
                  </span>
                  <span class="actions">
                    {#if m.kind}
                      <button
                        class="icon"
                        title="Auto: read the class from the lines again"
                        aria-label="Class back to auto"
                        onclick={() => updateAnalysis(setMultipletKind(doc.analysis, at, ''))}
                        ><Reset size={14} /></button
                      >
                    {/if}
                  </span>
                </label>
                <label title="The couplings it is quoted with; empty measures them again">
                  <span class="name">J</span>
                  <span class="control">
                    <input
                      inputmode="decimal"
                      value={(m.jsHz ?? info?.jsHz ?? []).map((j) => j.toFixed(1)).join(', ')}
                      onchange={(e) =>
                        updateAnalysis(
                          setMultipletJs(doc.analysis, at, parseJs(e.currentTarget.value)),
                        )}
                    />
                    <span class="unit">Hz</span>
                  </span>
                  <span class="actions">
                    <button
                      class="icon"
                      title="Pick the couplings off the spectrum"
                      aria-label="Pick the couplings off the spectrum"
                      onclick={() => startPicking(at, 'js')}><Pipette size={14} /></button
                    >
                    {#if m.jsHz}
                      <button
                        class="icon"
                        title="Auto: measure the couplings from the lines again"
                        aria-label="Couplings back to auto"
                        onclick={() => updateAnalysis(setMultipletJs(doc.analysis, at, []))}
                        ><Reset size={14} /></button
                      >
                    {/if}
                  </span>
                </label>
                {#each sharedCouplings(doc.multipletInfo, at) as shared (`${shared.j}:${shared.other}:${shared.k}`)}
                  <p class="shared">
                    <Link size={14} />
                    <span
                      >{info.jsHz[shared.j].toFixed(2)} Hz is also in {doc.analysis.multiplets[
                        shared.other
                      ]?.name ?? letterOf(shared.other)} ({shared.hz.toFixed(2)} Hz)</span
                    >
                    {#if Math.abs(info.jsHz[shared.j] - shared.hz) > 0.005}
                      <button
                        class="ghost"
                        title="Quote both as their average"
                        onclick={() => matchCoupling(at, shared)}>Match</button
                      >
                    {/if}
                  </p>
                {/each}
                <label title="How wide its lines are drawn; 0 uses the width the spectrum shows">
                  <span class="name">Line width</span>
                  <span class="control">
                    <input
                      inputmode="decimal"
                      value={(m.widthHz ?? info?.widthHz ?? 0).toFixed(1)}
                      onchange={(e) =>
                        updateAnalysis(
                          setMultipletWidth(doc.analysis, at, decimal(e.currentTarget.value)),
                        )}
                    />
                    <span class="unit">Hz</span>
                  </span>
                  <span class="actions">
                    {#if m.widthHz}
                      <button
                        class="icon"
                        title="Auto: the width the lines fit best with"
                        aria-label="Line width back to auto"
                        onclick={() => updateAnalysis(setMultipletWidth(doc.analysis, at, 0))}
                        ><Reset size={14} /></button
                      >
                    {/if}
                  </span>
                </label>
                <label
                  title="Nuclei it stands for; 0 follows its integral again. Assigning hydrogens of the structure sets it to their count"
                >
                  <span class="name">Nuclei</span>
                  <span class="control">
                    <input
                      inputmode="numeric"
                      value={nucleiOf(m, info) ?? ''}
                      placeholder="Unavailable"
                      onchange={(e) =>
                        updateAnalysis(
                          setMultipletNuclei(doc.analysis, at, decimal(e.currentTarget.value)),
                        )}
                    />
                    <span class="unit">{doc.nucleus.replace(/^\d+/, '')}</span>
                  </span>
                  <span class="actions"></span>
                </label>
              </section>
              <section>
                <h3 class="section-label">Range</h3>
                <label title="Where it is quoted, which need not be where its lines sit">
                  <span class="name">Shift</span>
                  <span class="control">
                    <input
                      inputmode="decimal"
                      value={fixed(info?.ppm ?? 0, decimals)}
                      onchange={(e) =>
                        updateAnalysis(
                          setMultipletShift(doc.analysis, at, decimal(e.currentTarget.value)),
                        )}
                    />
                    <span class="unit">ppm</span>
                  </span>
                  <span class="actions">
                    <button
                      class="icon"
                      title="Pick the shift off the spectrum"
                      aria-label="Pick the shift off the spectrum"
                      onclick={() => startPicking(at, 'shift')}><Pipette size={14} /></button
                    >
                    {#if m.ppm !== null && m.ppm !== undefined}
                      <button
                        class="icon"
                        title="Auto: the shift its lines sit at"
                        aria-label="Shift back to auto"
                        onclick={() => updateAnalysis(setMultipletShift(doc.analysis, at, null))}
                        ><Reset size={14} /></button
                      >
                    {/if}
                  </span>
                </label>
                <label title="The higher end of the range its lines are read from">
                  <span class="name">From</span>
                  <span class="control">
                    <input
                      inputmode="decimal"
                      value={fixed(m.from, decimals)}
                      onchange={(e) =>
                        updateAnalysis(
                          setMultipletRange(doc.analysis, at, decimal(e.currentTarget.value), m.to),
                        )}
                    />
                    <span class="unit">ppm</span>
                  </span>
                  <span class="actions">
                    <button
                      class="icon"
                      title="Pick this end off the spectrum"
                      aria-label="Pick the start of the range off the spectrum"
                      onclick={() => startPicking(at, 'from')}><Pipette size={14} /></button
                    >
                  </span>
                </label>
                <label title="The lower end of the range its lines are read from">
                  <span class="name">To</span>
                  <span class="control">
                    <input
                      inputmode="decimal"
                      value={fixed(m.to, decimals)}
                      onchange={(e) =>
                        updateAnalysis(
                          setMultipletRange(
                            doc.analysis,
                            at,
                            m.from,
                            decimal(e.currentTarget.value),
                          ),
                        )}
                    />
                    <span class="unit">ppm</span>
                  </span>
                  <span class="actions">
                    <button
                      class="icon"
                      title="Pick this end off the spectrum"
                      aria-label="Pick the end of the range off the spectrum"
                      onclick={() => startPicking(at, 'to')}><Pipette size={14} /></button
                    >
                  </span>
                </label>
              </section>
              <p class="fact">
                Read from {info?.lines ?? 0}
                {(info?.lines ?? 0) === 1 ? 'line' : 'lines'}.
                {#if info?.explained != null}
                  <span class:poor={info.explained < 0.9}
                    >Its lines fit {Math.round(info.explained * 100)} % of the signal{info.explained <
                    0.9
                      ? ': try another class, or Fit to spectrum'
                      : ''}.</span
                  >
                {/if}
              </p>
              <label
                class="switch"
                title="Drawn over the signal, to read what was typed against it"
              >
                <input
                  type="checkbox"
                  checked={m.shape ?? true}
                  onchange={(e) =>
                    updateAnalysis(setMultipletShape(doc.analysis, at, e.currentTarget.checked))}
                />
                <span>Draw its fitted lines (when Fitted lines is on)</span>
              </label>
            </div>
            <footer>
              <button
                class="ghost"
                title="Fit shift, couplings and line width to the spectrum together"
                onclick={() => void fitEditing(at)}><Sparkles size={16} /> Fit to spectrum</button
              >
              <button
                class="ghost"
                title="Cut this multiplet in two at a point picked on the spectrum"
                onclick={() => startPicking(at, 'split')}><Scissors size={16} /> Split</button
              >
              <button
                class="ghost"
                title="Join this multiplet with the next one down the spectrum"
                disabled={at === doc.analysis.multiplets.length - 1}
                onclick={() => void updateAnalysis(mergeMultiplets(doc.analysis, at))}
                ><Merge size={16} /> Merge</button
              >
            </footer>
          </section>
        {:else}
          <AnalysisPanel
            doc={active}
            part={panelTab === 'integrals'
              ? 'integrals'
              : panelTab === 'multiplets'
                ? 'multiplets'
                : 'peaks'}
            {decimals}
            onchange={updateAnalysis}
            onedit={(index) => (editing = index)}
            oncopy={() => void copyReport()}
          />
        {/if}
      </aside>
    {/if}
  </div>
</div>

<dialog class="window export-many" bind:this={exportDialog}>
  <header>
    <h2>Export many</h2>
    <button
      class="icon"
      tabindex="-1"
      title="Close (Esc)"
      aria-label="Close"
      onclick={(e) => e.currentTarget.closest('dialog')?.close()}><X size={16} /></button
    >
  </header>
  <div class="window-body">
    <p class="hint">
      Each spectrum is shown in turn on the page and printed as it is drawn: the range and height
      the plot has, and the page setup.
    </p>
    <section class="export-list">
      {#each docs as doc (doc.id)}
        <label>
          <input
            type="checkbox"
            checked={exportIds.includes(doc.id)}
            onchange={(e) =>
              (exportIds = e.currentTarget.checked
                ? [...exportIds, doc.id]
                : exportIds.filter((id) => id !== doc.id))}
          />
          <span class="swatch" style:background={traceColorOf[doc.id] ?? traceColors[0]}></span>
          {named(doc)}
        </label>
      {/each}
    </section>
    <fieldset class="export-how">
      <label><input type="radio" bind:group={exportOne} value={true} /> One PDF, a page each</label>
      <label
        ><input type="radio" bind:group={exportOne} value={false} /> A PDF for each, in a folder</label
      >
    </fieldset>
  </div>
  <footer>
    <span class="hint">{exportIds.length} of {docs.length} chosen</span>
    <button onclick={() => exportDialog?.close()}>Cancel</button>
    <button
      class="primary"
      disabled={exporting || exportIds.length === 0}
      onclick={() => void exportMany()}>{exporting ? 'Exporting…' : 'Export'}</button
    >
  </footer>
</dialog>

<dialog class="window help" bind:this={helpDialog}>
  <header>
    <h2>Keys and mouse</h2>
    <button
      class="icon"
      tabindex="-1"
      title="Close (Esc)"
      aria-label="Close"
      onclick={(e) => e.currentTarget.closest('dialog')?.close()}><X size={16} /></button
    >
  </header>
  <div class="window-body">
    {#each SHORTCUTS as { group, items } (group)}
      <section>
        <h3 class="section-label">{group}</h3>
        <dl>
          {#each items as [keys, what] (keys)}
            <dt><kbd>{keys}</kbd></dt>
            <dd>{what}</dd>
          {/each}
        </dl>
      </section>
    {/each}
  </div>
  <footer>
    <form method="dialog"><button class="primary">Done</button></form>
  </footer>
</dialog>

<dialog class="window settings" bind:this={settingsDialog}>
  <header>
    <h2>Settings</h2>
    <button
      class="icon"
      tabindex="-1"
      title="Close (Esc)"
      aria-label="Close"
      onclick={(e) => e.currentTarget.closest('dialog')?.close()}><X size={16} /></button
    >
  </header>
  <div class="tabs settings-tabs" role="tablist">
    {#each [['general', 'General'], ['processing', 'Processing'], ['analysis', 'Analysis']] as const as [key, label] (key)}
      <button role="tab" aria-selected={settingsTab === key} onclick={() => (settingsTab = key)}
        >{label}</button
      >
    {/each}
  </div>
  <div class="window-body">
    {#if settingsTab === 'general'}
      <section>
        <h3 class="section-label">Appearance</h3>
        <label title="Colours of the interface and of the spectra">
          Theme
          <select value={theme} onchange={(e) => (theme = e.currentTarget.value)}>
            <optgroup label="Built in">
              {#each BUILT_IN as name (name)}
                <option value={name}>{name[0].toUpperCase() + name.slice(1)}</option>
              {/each}
            </optgroup>
            <optgroup label="Your own">
              <option value={CUSTOM}>Custom</option>
            </optgroup>
            <optgroup label="Palettes">
              {#each Object.entries(PALETTES) as [key, entry] (key)}
                <option value={key}>{entry.name}</option>
              {/each}
            </optgroup>
          </select>
        </label>
        <label
          title="Typeface of the interface: the fonts found on this computer, or any other by name"
        >
          Font
          <span class="font-slot">
            <FontChoice value={font} first={[SYSTEM_FONT]} onchange={(name) => (font = name)} />
          </span>
        </label>
        <label
          title="Bond length, line widths and lettering of the structure, in points as a journal states them"
        >
          Structure
          <select
            value={acsStyle ? 'acs' : 'custom'}
            onchange={(e) => {
              if (e.currentTarget.value === 'acs') structureStyle = { ...ACS_1996 };
            }}
          >
            <option value="acs">ACS 1996</option>
            <option value="custom">Custom</option>
          </select>
        </label>
        {#each [['bondLength', 'Bond length', 'pt'], ['lineWidth', 'Line width', 'pt'], ['boldWidth', 'Bold width', 'pt'], ['hashSpacing', 'Hash spacing', 'pt'], ['margin', 'Margin', 'pt'], ['fontSize', 'Label size', 'pt']] as const as [key, name, unit] (key)}
          <label
            ><span>{name} ({unit})</span><input
              type="number"
              min="0.1"
              max="100"
              step="0.1"
              bind:value={structureStyle[key]}
            /></label
          >
        {/each}
        <label
          class="indent"
          title="Gap between the two lines of a double bond, as a share of the bond length"
          ><span>Double bond spacing (%)</span><input
            type="number"
            min="5"
            max="50"
            step="1"
            value={Math.round(structureStyle.spacing * 100)}
            onchange={(e) => (structureStyle.spacing = Number(e.currentTarget.value) / 100)}
          /></label
        >
        <label
          ><span>Label font</span><span class="font-slot"
            ><FontChoice
              value={structureStyle.font}
              onchange={(name) => (structureStyle.font = name)}
            /></span
          ></label
        >
        <label
          ><span>Colour heteroatoms</span><input
            type="checkbox"
            bind:checked={structureStyle.elementColours}
          /></label
        >
        <label
          class="indent"
          title="Write CH3 and the like at the end of a chain, where the hydrogens are what is being assigned"
          ><span>Label terminal carbons</span><input
            type="checkbox"
            bind:checked={structureStyle.terminalCarbons}
          /></label
        >
        <label title="Decimal places for every shift in ppm, in the tables and on the plot">
          Decimals
          <input type="number" min="0" max="4" step="1" bind:value={decimals} />
        </label>
      </section>
      <section>
        <h3 class="section-label">Text on the plot</h3>
        {#each [['Peak labels', 'peakSize', 'peakColour'], ['Integrals', 'integralSize', 'integralColour'], ['Multiplets', 'multipletSize', 'multipletColour']] as const as [name, size, colour] (name)}
          <label title="Size in pixels, and a colour of its own or the spectrum's">
            {name}
            <span class="font-slot text-style">
              <input
                type="number"
                min="6"
                max="24"
                step="1"
                aria-label="{name} size"
                bind:value={plotText[size]}
              />
              <select
                aria-label="{name} colour"
                value={plotText[colour] === '' ? 'spectrum' : 'own'}
                onchange={(e) =>
                  (plotText[colour] =
                    e.currentTarget.value === 'spectrum' ? '' : plotText[colour] || '#808080')}
              >
                <option value="spectrum">Spectrum</option>
                <option value="own">Custom</option>
              </select>
              {#if plotText[colour] !== ''}
                <input type="color" aria-label="{name} colour" bind:value={plotText[colour]} />
              {/if}
            </span>
          </label>
        {/each}
      </section>
      {#if theme === CUSTOM}
        <section>
          <h3 class="section-label">Colours</h3>
          {#each CUSTOM_COLOURS as [name, label] (name)}
            <label title="Choosing a colour makes the theme your own">
              {label}
              <input
                type="color"
                value={custom[name]}
                onchange={(e) => setColour(name, e.currentTarget.value)}
              />
            </label>
          {/each}
          {#each contrastWarnings(custom) as warning (warning)}
            <p class="hint" role="status">{warning}</p>
          {/each}
          <label title="Whether menus and form controls are drawn dark">
            Dark controls
            <input
              type="checkbox"
              checked={customDark}
              onchange={(e) => {
                customDark = e.currentTarget.checked;
                theme = CUSTOM;
                saveColours();
              }}
            />
          </label>
        </section>
      {/if}
    {:else if settingsTab === 'processing'}
      <section>
        <h3 class="section-label">A spectrum as it is opened</h3>
        <label
          title="Work a phase out when the dataset carries none; the one the spectrometer recorded is always applied"
        >
          <span>Phase it</span>
          <input type="checkbox" bind:checked={phaseOnOpen} />
        </label>
        <label title="Take a baseline off, with the smoother below">
          <span>Correct its baseline</span>
          <input type="checkbox" bind:checked={baselineOnOpen} />
        </label>
      </section>
      <section>
        <h3 class="section-label">Baseline (Whittaker smoother)</h3>
        <label title="How stiff the baseline is: larger follows the spectrum less">
          Smooth Factor
          <input
            type="number"
            min="1"
            step="any"
            value={baselineDefault.smoothness}
            onchange={(e) => {
              const v = decimal(e.currentTarget.value);
              if (v > 0) baselineDefault = { ...baselineDefault, smoothness: v };
            }}
          />
        </label>
        <label title="Widen the filter to preserve detected broad signals">
          Automatic filter
          <input type="checkbox" bind:checked={baselineDefault.autoFilter} />
        </label>
        <label title="Width of the filter that tells signal from baseline, in Hz">
          {baselineDefault.autoFilter ? 'Minimum filter (Hz)' : 'Filter (Hz)'}
          <input
            type="number"
            min="0.1"
            step="0.1"
            value={baselineDefault.filterHz}
            onchange={(e) => {
              const v = decimal(e.currentTarget.value);
              if (v > 0) baselineDefault = { ...baselineDefault, filterHz: v };
            }}
          />
        </label>
        <p class="hint">
          Used on opening and by Auto baseline; a spectrum's own step can still be changed in its
          panel.
        </p>
      </section>
    {:else}
      <section>
        <h3 class="section-label">Picking</h3>
        <label title="Peaks are picked above this many times the noise">
          Noise ×
          <input type="number" min="1" step="0.5" bind:value={noiseFactor} />
        </label>
        <label
          title="How far above the axis the lowest thing sits when the view is fitted (F, H) - the baseline, or the integrals under it; Shift and drag moves one spectrum"
        >
          Above the axis
          <span class="range-slot">
            <input type="range" min="0" max="60" step="1" bind:value={baselineGap} />
            <output>{baselineGap} %</output>
          </span>
        </label>
        <label title="A pick takes the top of the nearest line; off, the pointer where it is">
          <span>Snap to peaks</span>
          <input type="checkbox" bind:checked={snap} />
        </label>
        <label title="What a multiplet is called on the plot">
          Multiplet labels
          <select bind:value={multipletLabel}>
            <option value="shift">Shift, e.g. 7.92 (d)</option>
            <option value="name">Name, e.g. A (d)</option>
            <option value="both">Both, e.g. A 7.92 (d)</option>
          </select>
        </label>
      </section>
      <section>
        <h3 class="section-label">On the plot</h3>
        <label><span>Peak labels</span><input type="checkbox" bind:checked={showLabels} /></label>
        <label><span>Integrals</span><input type="checkbox" bind:checked={showIntegrals} /></label>
        <label
          ><span>Integral curves</span><input type="checkbox" bind:checked={showCurves} /></label
        >
        <label><span>Multiplets</span><input type="checkbox" bind:checked={showMultiplets} /></label
        >
        <label
          ><span>Fitted lines</span><input type="checkbox" bind:checked={showSimulation} /></label
        >
        <label title="Label every spectrum of a stack, not only the active one"
          ><span>Labels on every spectrum</span><input
            type="checkbox"
            bind:checked={showAll}
          /></label
        >
      </section>
    {/if}
  </div>
  <footer>
    <span class="hint">Kept between sessions</span>
    <form method="dialog"><button class="primary">Done</button></form>
  </footer>
</dialog>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .app.dropping main {
    position: relative;
    outline: 2px dashed var(--accent);
    outline-offset: -8px;
  }

  .app.dropping main::after {
    content: 'Drop to open';
    position: absolute;
    inset: auto 0 var(--space-6);
    text-align: center;
    color: var(--accent);
    font-size: var(--text-body);
    pointer-events: none;
  }

  /* One bar: the file, the stages of the work and what is not a stage, then
     search and the panels. Two bars before it took 125 px from the spectrum. */
  .toolbar {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    min-height: 44px;
    padding: 0 var(--space-4);
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }

  .toolbar .search {
    min-width: 16rem;
    justify-content: flex-start;
    border-color: var(--border);
    background: var(--bg);
    color: var(--muted);
  }

  .toolbar .search kbd {
    margin-left: auto;
    font-family: inherit;
    font-size: var(--text-label);
  }

  /* Narrow windows keep the icons and drop the words, which stay in titles. */
  @media (max-width: 1279px) {
    .toolbar .label {
      display: none;
    }

    .toolbar .search {
      min-width: 0;
    }
  }

  .separator {
    width: 1px;
    height: 18px;
    margin: 0 4px;
    background: var(--border);
  }

  .spacer {
    flex: 1;
  }

  .errors {
    display: flex;
    align-items: flex-start;
    gap: var(--space-4);
    padding: var(--space-3) var(--space-5);
    background: var(--error-bg);
    color: var(--error-fg);
  }

  .errors ul {
    flex: 1;
    margin: 0;
    padding-left: 1rem;
  }

  .errors button {
    border: none;
    background: none;
  }

  .body {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .export-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    max-height: 40vh;
    overflow-y: auto;
  }

  .export-list label,
  .export-how label {
    display: flex;
    gap: var(--space-4);
    align-items: center;
  }

  .export-list .swatch {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }

  .export-how {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    margin: 0;
    padding: 0;
    border: none;
  }

  /* A page on a desk: the plot at the size it is printed, shown enlarged. */
  main.desk {
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--fg) 7%, var(--bg));
  }

  .paper,
  .page-plot {
    display: contents;
  }

  .paper.on {
    position: relative;
    display: block;
    flex: none;
    box-shadow: var(--shadow-overlay);
  }

  .page-plot.on {
    position: absolute;
    display: flex;
    flex-direction: column;
  }

  .list,
  .side {
    display: flex;
    flex: none;
    flex-direction: column;
    overflow-y: auto;
    /* Told apart from the ribbon above them and the plot between them. */
    background: color-mix(in srgb, var(--panel) 55%, var(--bg));
  }

  /* Pinned to the foot of the column, so it stays in view while the list
     scrolls: the structure is there to be glanced at, not scrolled to. */
  /* It takes the room the drawing needs rather than a fixed box: a structure
     pasted at its own size was cut off at the top, because a box that cannot
     shrink is pushed past the foot of a short column and the column clips it.
     It gives way to the list when there is no room, and never grows past half
     the window. */
  .structure {
    position: relative;
    flex: 0 1 auto;
    display: flex;
    gap: var(--space-1);
    align-items: center;
    justify-content: center;
    min-height: 56px;
    max-height: 45vh;
    margin-top: 6px;
    padding: var(--space-2);
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background: var(--bg);
  }

  .structure:focus-visible {
    border-color: var(--accent);
  }

  /* The drawing at its own size while it fits, and no larger than the pane. An
     embed has no intrinsic size to fit, so it is given the whole pane. */
  .structure img {
    width: auto;
    max-width: 100%;
    height: auto;
    max-height: 100%;
    object-fit: contain;
  }

  .structure embed {
    width: 100%;
    height: 100%;
    min-height: 140px;
  }

  /* A structure is drawn in black on nothing, which is invisible on a dark page. */
  .structure .invert {
    filter: invert(1) hue-rotate(180deg);
  }

  .paster {
    position: absolute;
    inset: 0;
    caret-color: transparent;
    outline: none;
  }

  .structure .arm {
    position: absolute;
    top: var(--space-1);
    left: var(--space-2);
    z-index: 2;
    padding: 0 var(--space-2);
    border-radius: var(--radius-control);
    background: var(--accent);
    color: var(--bg);
    font-size: var(--text-caption);
    pointer-events: none;
  }

  .structure .hint {
    pointer-events: none;
  }

  .structure .choose {
    position: relative;
    z-index: 1;
  }

  .structure .remove {
    position: absolute;
    z-index: 1;
    top: var(--space-1);
    right: var(--space-1);
  }

  /* A hairline between the columns, its grip the width of a finger: the line
     is drawn in the middle of a transparent bar, and only widens to say it can
     be dragged when pointed at. A 5 px bar of the border colour read as a frame. */
  .splitter {
    flex: none;
    width: 5px;
    padding: 0;
    border: none;
    border-radius: 0;
    background: linear-gradient(var(--border), var(--border)) center / 1px 100% no-repeat;
    cursor: col-resize;
  }

  .splitter:hover,
  .splitter:focus-visible {
    background: linear-gradient(var(--accent), var(--accent)) center / 3px 100% no-repeat;
  }

  .help {
    width: min(560px, 92vw);
  }

  /* One key column for every group, so the descriptions line up down the whole
     sheet; a long key wraps in it rather than pushing its group's column out. */
  .help dl {
    display: grid;
    grid-template-columns: 10rem 1fr;
    gap: var(--space-2) var(--space-5);
    margin: 0;
  }

  .help dt,
  .help dd {
    margin: 0;
  }

  .help kbd {
    font-family: inherit;
    font-weight: 600;
  }

  /* What the chosen tool does, floating at the top of the plot for as long as
     the tool is chosen, and for a moment after an automatic step what it did. */
  .tool-lane {
    position: absolute;
    top: calc(var(--draw-strip) + var(--space-4));
    left: 50%;
    z-index: 3;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    align-items: center;
    max-width: 90%;
    transform: translateX(-50%);
    pointer-events: none;
  }

  .tool-lane > * {
    pointer-events: auto;
  }

  .tool-hint {
    display: flex;
    gap: var(--space-4);
    align-items: center;
    /* Its own width, not what is left right of the middle: centred by left 50%,
       it was given half the plot and wrapped. */
    width: max-content;
    max-width: min(46rem, 90%);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-5);
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--bg);
    box-shadow: var(--shadow-overlay);
  }

  /* Low on the right, over the axis end of the plot: at the top it covered the
     peak labels, which run right up to the edge on a real spectrum. */
  /* The spectrum's name, and which part of it the panel shows. */
  .mode-title {
    font-size: var(--text-title);
  }

  .crowded {
    padding: var(--space-4) var(--space-4) 0;
    color: var(--fg);
  }

  .inspector-title {
    display: flex;
    gap: var(--space-3);
    align-items: baseline;
    justify-content: space-between;
  }

  .inspector-title h2 {
    overflow: hidden;
    margin: 0;
    font-size: var(--text-body);
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The plot's own edge: what it draws, and where the pointer is. */
  .draw-strip {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    min-height: var(--draw-strip);
    padding: 0 var(--space-3);
    border-bottom: 1px solid var(--border);
    background: var(--bg);
  }

  .draws {
    display: flex;
    gap: var(--space-1);
  }

  /* :global for the button that opens a menu in it, its own component; the
     menu's items keep the size every menu has. */
  .draws > button,
  .draws > :global(.menu) > :global(button) {
    min-height: 26px;
    padding: 0 var(--space-3);
    font-size: var(--text-label);
  }

  .draw-strip .readout {
    font-size: var(--text-label);
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    white-space: pre;
  }

  .tool-hint.notice {
    padding-right: var(--space-5);
  }

  .tool-hint button {
    min-height: 22px;
    border-radius: 999px;
    color: var(--accent);
    font-weight: 600;
  }

  .picking {
    display: flex;
    gap: var(--space-4);
    align-items: center;
    padding: var(--space-4) var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-overlay);
    background: var(--panel);
    box-shadow: var(--shadow-overlay);
  }

  .picking output {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .settings section + section {
    margin-top: var(--space-2);
  }

  .settings h3 {
    margin-bottom: var(--space-4);
  }

  /* Its tabs sit under the title, as the ribbon's sit under the tool bar. */
  .settings-tabs {
    border-bottom: 1px solid var(--border);
  }

  .settings .hint {
    margin: 0 0 var(--space-4);
  }

  .range-slot {
    display: flex;
    flex: none;
    gap: var(--space-3);
    align-items: center;
    width: 11rem;
  }

  .range-slot input {
    flex: 1;
    min-width: 0;
  }

  .range-slot output {
    width: 3rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .font-slot {
    flex: none;
    width: 11rem;
  }

  /* A size, and the spectrum's colour or one of its own, in the one column. */
  .text-style {
    display: flex;
    gap: var(--space-2);
    width: 15rem;
  }

  .text-style input[type='number'] {
    width: 3.5rem;
    text-align: right;
  }

  .text-style select {
    flex: 1;
    min-width: 0;
  }

  .text-style input[type='color'] {
    flex: none;
    width: var(--control-height);
  }

  .settings label {
    display: flex;
    gap: var(--space-4);
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--space-4);
  }

  /* One column for everything that can be changed, so the right edge is a line
     rather than a ragged fringe: the fields were as wide as they each happened
     to need. */
  .settings label > input:not([type='checkbox']),
  .settings label > select {
    box-sizing: border-box;
    flex: none;
    width: 11rem;
  }

  /* A multiplet gone into, in place of the table: a way back and its name, what
     it reads as, its fields, and what can be done to it at the foot. */
  .multiplet.detail {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }

  .multiplet.detail > header {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border);
  }

  .multiplet.detail > header h3 {
    margin: 0;
    font-size: var(--text-title);
    font-weight: 600;
  }

  .multiplet .back {
    padding-left: var(--space-2);
  }

  .multiplet .detail-body {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-5) var(--space-6);
  }

  .multiplet.detail > footer {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-6);
    border-top: 1px solid var(--border);
  }

  /* The line the window exists to produce: read and taken, never typed into, so
     it wears none of a field's clothes and carries the accent down its edge. */
  /* What the window is for, said plainly: it is the window's own answer, so it
     sits on the window rather than in a panel of its own. A tinted box with a bar
     down its edge is a notice pasted over a page, which this is not. */
  .multiplet .output {
    display: flex;
    gap: var(--space-4);
    align-items: baseline;
    margin-bottom: var(--space-6);
  }

  /* In the interface's own face: set in a typewriter face it was the one line of
     the window written in another hand. */
  .multiplet .output p {
    flex: 1;
    margin: 0;
    font-size: var(--text-title);
    font-weight: 500;
    font-variant-numeric: tabular-nums;
    overflow-wrap: anywhere;
    user-select: text;
  }

  /* Three columns: what it is called, what it is, and what can be done to it.
     The third stays its own width whether or not it holds anything, so no row
     shifts sideways against its neighbours. */
  .multiplet section {
    display: grid;
    grid-template-columns: 6rem 1fr var(--control-height);
    column-gap: var(--space-4);
    row-gap: var(--space-4);
    align-items: center;
    margin-bottom: var(--space-6);
  }

  .multiplet section > h3 {
    grid-column: 1 / -1;
    margin: 0;
  }

  .multiplet section label {
    display: contents;
  }

  .multiplet .name {
    color: var(--muted);
    font-size: var(--text-label);
  }

  /* A field and the unit it is in, which belongs to the number and not to the
     name: inside the box, quiet, and not in the way of the pointer. */
  .multiplet .control {
    position: relative;
    display: flex;
    gap: var(--space-2);
    align-items: center;
    min-width: 0;
  }

  .multiplet .control > input,
  .multiplet .control > select {
    flex: 1;
    width: auto;
    min-width: 0;
    padding-right: 3.25rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  /* A name is not a number, and a list of classes is not one either. */
  .multiplet .control > input.text,
  .multiplet .control > select {
    padding-right: var(--space-6);
    text-align: left;
    font-variant-numeric: normal;
  }

  /* In the number's own size, so the two share a baseline: smaller, it was
     centred and stood above the figures. */
  .multiplet .unit {
    position: absolute;
    right: var(--space-3);
    color: var(--muted);
    pointer-events: none;
  }

  .multiplet .actions {
    display: flex;
    gap: var(--space-2);
  }

  /* A switch is not a field: its words are what it does, so they lead. */
  .multiplet .switch {
    justify-content: flex-start;
    gap: var(--space-4);
    margin-bottom: var(--space-6);
  }

  .multiplet .fact {
    margin: 0 0 var(--space-4);
  }

  .multiplet .fact .poor {
    color: var(--fg);
  }

  /* A coupling found again in another multiplet: under the J it belongs to. */
  .multiplet .shared {
    display: flex;
    grid-column: 2 / -1;
    gap: var(--space-2);
    align-items: center;
    margin: calc(-1 * var(--space-2)) 0 0;
    color: var(--muted);
    font-size: var(--text-label);
  }

  .multiplet .shared button {
    margin-left: auto;
  }

  /* What the window says rather than what it asks: no field, no box, and set
     apart from the rows above it. */
  .fact {
    margin: 0 0 var(--space-4);
    color: var(--muted);
    font-size: var(--text-label);
  }

  .list {
    gap: var(--space-1);
    padding: 0 var(--space-3) var(--space-3);
    overflow: hidden;
  }

  /* The title band spans the column, as the panel's does on the other side. */
  .list > .column-title {
    margin: 0 calc(-1 * var(--space-3)) var(--space-2);
  }

  /* The list scrolls inside the column so that the structure below it does not. */
  .rows {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-1);
    min-height: 0;
    overflow-y: auto;
  }

  /* Its title and hairlines run to the window's edge: a gutter kept for the
     scrollbar left a dead strip down the right of every part. Each part's own
     padding keeps an overlay scrollbar off its values. */
  .side {
    box-sizing: border-box;
  }

  .swatch {
    appearance: none;
    width: 12px;
    height: 12px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    cursor: pointer;
  }

  .swatch::-webkit-color-swatch-wrapper {
    padding: 0;
  }

  .swatch::-webkit-color-swatch {
    border: 1px solid var(--border);
    border-radius: 50%;
  }

  /* What can be done with the ticked spectra, shown only once some are ticked:
     buttons that stand disabled until then read as broken. */
  .selection {
    display: flex;
    gap: var(--space-1);
    align-items: center;
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
    border-radius: var(--radius-control);
    background: var(--live-soft);
  }

  .selection .hint {
    flex: 1;
  }

  /* Closing is there when the row is pointed at or has the keyboard, and out of
     the way otherwise. */
  .doc-row .close {
    opacity: 0;
  }

  .doc-row:hover .close,
  .doc-row:focus-within .close,
  .doc-row:has(.active) .close {
    opacity: 1;
  }

  .doc.view .name,
  .doc.view {
    color: var(--live);
  }

  /* The row is the box, tick to ×: a tint on the name alone left the tick, the
     colour and the × hanging outside it. */
  .doc-row {
    display: grid;
    grid-template-columns: 13px 1fr auto auto;
    gap: var(--space-2);
    align-items: center;
    padding: 0 var(--space-1) 0 var(--space-3);
    border-radius: var(--radius-inner);
  }

  .doc-row:hover {
    background: var(--hover);
  }

  .doc-row:has(.doc.current) {
    background: var(--live-soft);
  }

  .doc-row input[type='checkbox'] {
    justify-self: center;
  }

  /* A view has no checkbox or colour of its own, so it starts where the names do
     and its × keeps the last column. */
  .doc.view {
    grid-column: 2;
  }

  .doc.view + .icon {
    grid-column: 4;
  }

  /* A 2D has neither either, and lines its name up with the spectra's. */
  .two-d .doc {
    grid-column: 2;
  }

  .two-d .doc + .icon {
    grid-column: 4;
  }

  .doc {
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    padding: var(--space-2) var(--space-2);
    background: none;
    text-align: left;
    cursor: pointer;
  }

  .doc small,
  .empty,
  .placeholder {
    color: var(--muted);
  }

  /* The caption role, not the browser's smaller, which came to 10.8 px. */
  .doc small {
    font-size: var(--text-caption);
  }

  /* What a name adds to a quiet field: it is the title of its row, and it is cut
     rather than allowed to push the row about. */
  .name {
    width: 100%;
    overflow: hidden;
    color: var(--fg);
    font: inherit;
    font-size: var(--text-title);
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  input.name {
    cursor: text;
  }

  /* Bold says which spectrum is worked on; the tint says which page is open. */
  .doc.current .name,
  .doc.active .name {
    font-weight: 600;
  }

  .ribbon {
    display: flex;
    flex-direction: column;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }

  /* A tab is its name with a line under it when it is the one open: the frame
     around the open tab of a notebook was one more box among many. */
  .tablist button {
    min-height: 34px;
    padding: 0 var(--space-5);
    border: none;
    border-bottom: 2px solid transparent;
    border-radius: 0;
    background: none;
    color: var(--muted);
    font-weight: 500;
  }

  .tablist button:hover {
    background: none;
    color: var(--fg);
  }

  .tablist button[aria-selected='true'] {
    border-bottom-color: var(--live);
    color: var(--fg);
  }

  .tablist {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    align-self: stretch;
  }

  .tablist button {
    align-self: stretch;
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: center;
    min-height: 40px;
    padding: var(--space-2) var(--space-4);
    background: var(--bg);
  }

  .controls label {
    display: flex;
    gap: var(--space-3);
    align-items: center;
  }

  .controls input[type='range'] {
    width: 8rem;
  }

  .controls output {
    min-width: 2.5rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .recorded {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-5);
  }

  .recorded table {
    border-collapse: collapse;
    width: 100%;
    table-layout: fixed;
  }

  .recorded th {
    position: relative;
    padding: var(--space-1) 0;
    text-align: left;
    font-size: var(--text-label);
    font-weight: 600;
    color: var(--muted);
  }

  .recorded th.value {
    text-align: right;
  }

  /* The line between the columns is what is taken hold of. */
  .grip {
    position: absolute;
    top: 0;
    right: -6px;
    width: 11px;
    height: 100%;
    padding: 0;
    border: none;
    border-radius: 0;
    background: none;
    cursor: col-resize;
  }

  .grip::after {
    content: '';
    position: absolute;
    top: 1px;
    bottom: 1px;
    left: 5px;
    border-left: 1px solid var(--border);
  }

  .grip:hover::after {
    border-color: var(--accent);
  }

  .recorded td {
    padding: 2px 0;
    vertical-align: top;
  }

  /* Named rather than by position: the checkbox comes first while rows are
     being chosen, and it is the name that reads as one. */
  .recorded td.key {
    overflow: hidden;
    padding-right: 12px;
    color: var(--muted);
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  /* The cross peak the pointer is over, on the plot or on its atoms. */
  .recorded tr.lit td {
    color: var(--live);
  }

  .recorded td.value {
    text-align: right;
    font-variant-numeric: tabular-nums;
    overflow-wrap: anywhere;
  }

  .plots {
    --draw-strip: 34px;
    position: relative;
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }

  main {
    flex: 1;
    min-height: 0;
  }

  /* Each spectrum gets an equal share of the height and its own view. */
  main.split {
    display: flex;
    flex-direction: column;
  }

  main.split > :global(.plot) {
    flex: 1;
    min-height: 0;
  }

  main.split > :global(.plot + .plot) {
    border-top: 2px solid var(--border);
  }

  .placeholder {
    display: grid;
    place-items: center;
    height: 100%;
  }

  .start {
    display: grid;
    gap: var(--space-4);
    max-width: 30rem;
    padding: var(--space-6);
  }

  .start h2 {
    margin: 0;
    font-size: var(--text-title);
  }

  .start-actions {
    display: flex;
    gap: var(--space-3);
  }

  .recent {
    display: grid;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .recent button {
    display: block;
    width: 100%;
    border-color: transparent;
    background: none;
    overflow: hidden;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
