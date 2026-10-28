#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod pdf;
mod project;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use nmr_core::process::{self, Operation};
use nmr_core::{
    PpmAxis, Spectrum, analysis, bruker, contour, decimate, deconvolution, fit, phase, process2d,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::ipc::Response;

/// Minimum distance in points between picked peaks, as nmrglue's `msep`.
const PEAK_SEPARATION: usize = 3;

/// Noise multiple the lines of a multiplet are read above. The picking threshold
/// is the front end's to choose per spectrum; a reading of a range that is
/// already a multiplet needs a level of its own, and this is the default one.
const MULTIPLET_NOISE: f64 = 5.0;

/// What a coupling can be, in Hz, for the nucleus at hand: two lines further
/// apart than this are two signals rather than one split one. Proton couplings
/// run to about 20 Hz; between other nuclei they run far larger, so only 1H is
/// held to the tight bound.
fn max_coupling(nucleus: &str) -> f64 {
    if nucleus == "1H" { 20.0 } else { 300.0 }
}

/// Integrals, their normalisation and picked peaks. Positions are in ppm so they
/// survive reprocessing.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Analysis {
    integrals: Vec<[f64; 2]>,
    reference: Option<IntegralReference>,
    peaks: Vec<f64>,
    /// One per peak: `None` follows the impurity table, `Some("")` shows nothing,
    /// anything else is what the user called it. Older projects have none.
    #[serde(default)]
    labels: Vec<Option<String>>,
    /// Ranges read as multiplets. Older projects have none.
    #[serde(default)]
    multiplets: Vec<MultipletRange>,
    #[serde(default)]
    peak_method: PeakMethod,
    #[serde(default)]
    integration_method: IntegrationMethod,
    #[serde(default)]
    peak_fit: Option<deconvolution::PeakFit>,
}

#[derive(Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum PeakMethod {
    #[default]
    Standard,
    Deconvolution,
}

#[derive(Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum IntegrationMethod {
    #[default]
    Sum,
    Fitted,
}

/// A range read as one multiplet, with what it has been called and how many
/// nuclei it stands for; without either, both follow from the spectrum.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MultipletRange {
    from: f64,
    to: f64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    nuclei: Option<f64>,
    /// Where it is quoted, when that is not where its lines sit.
    #[serde(default)]
    ppm: Option<f64>,
    /// The class read over by hand, when the lines are not what they look like.
    #[serde(default)]
    kind: Option<String>,
    /// The couplings read over by hand, in Hz.
    #[serde(default)]
    js_hz: Option<Vec<f64>>,
    /// The width a simulated line is drawn with, in Hz, when the width the
    /// spectrum shows is not the one wanted.
    #[serde(default)]
    width_hz: Option<f64>,
    /// Whether the shape it would have is drawn over the signal. One multiplet at
    /// a time: the switch in the tab says whether any are drawn at all.
    #[serde(default)]
    shape: Option<bool>,
    /// The atoms of the structure whose hydrogens give this signal, by position
    /// in the molecule read from its molfile.
    #[serde(default)]
    atoms: Vec<usize>,
}

/// What a multiplet reads as, worked out from the spectrum each time.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MultipletInfo {
    ppm: f64,
    kind: String,
    js_hz: Vec<f64>,
    lines: usize,
    /// The range's integral, on the same scale as the integrals table.
    value: Option<f64>,
    /// How wide its lines are, in Hz, to draw a simulation with.
    width_hz: f64,
    /// Share of the signal its lines account for, 0 to 1; none for an m.
    explained: Option<f64>,
}

/// Shows integral `index` as `value`; without a reference the first integral is 1.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
struct IntegralReference {
    index: usize,
    value: f64,
}

/// Everything that undo and redo restore.
#[derive(Clone)]
struct DocumentState {
    operations: Vec<Operation>,
    analysis: Analysis,
}

/// An open experiment: raw data, its state with history, and the processed result.
struct Document {
    path: PathBuf,
    raw: Spectrum,
    state: DocumentState,
    undo: Vec<DocumentState>,
    redo: Vec<DocumentState>,
    revision: u64,
    /// The spectrum right after the Fourier transform, used for phase previews.
    phase_base: Spectrum,
    spectrum: Spectrum,
    real: Vec<f64>,
    noise: f64,
}

impl Document {
    /// `baseline` is the correction to open with, as the settings have it, or none.
    fn open(path: PathBuf, phase: bool, baseline: Option<Operation>) -> nmr_core::Result<Self> {
        // A pdata folder holds a finished processed spectrum; everything else is a FID.
        let raw = if path.join("1r").is_file() {
            bruker::read_processed(&path)?
        } else {
            bruker::read_fid(&path)?
        };
        let mut operations = process::auto_1d(&raw, phase, baseline.is_some())?;
        // The correction the settings ask for, in the place the automatic one takes.
        if let Some(chosen) = baseline {
            for op in &mut operations {
                if is_baseline(op) {
                    *op = chosen.clone();
                }
            }
        }
        let (phase_base, spectrum) = process_split(&raw, &operations)?;
        let real = real_part(&spectrum);
        Ok(Self {
            noise: analysis::noise_level(&real),
            real,
            path,
            raw,
            state: DocumentState {
                operations,
                analysis: Analysis::default(),
            },
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            phase_base,
            spectrum,
        })
    }

    /// Switches to `state`, reprocessing if its operations differ, and returns the
    /// previous state. On error the document is left unchanged.
    fn replace(&mut self, mut state: DocumentState) -> nmr_core::Result<DocumentState> {
        if state.operations != self.state.operations {
            let (phase_base, spectrum) = process_split(&self.raw, &state.operations)?;
            self.real = real_part(&spectrum);
            self.noise = analysis::noise_level(&self.real);
            self.phase_base = phase_base;
            self.spectrum = spectrum;
            self.revision += 1;
        }
        if state.analysis.peak_fit.as_ref().is_some_and(|fit| {
            !fit.valid_for(
                &self.real,
                &self.spectrum.ppm_axis(),
                self.spectrum.meta.obs_mhz,
            )
        }) {
            state.analysis.peak_fit = None;
        }
        Ok(std::mem::replace(&mut self.state, state))
    }

    /// Records a user change for undo.
    fn change(&mut self, state: DocumentState) -> nmr_core::Result<()> {
        let previous = self.replace(state)?;
        self.undo.push(previous);
        self.redo.clear();
        Ok(())
    }
}

/// Processes up to and including the Fourier transform, then the remaining steps.
fn process_split(
    raw: &Spectrum,
    operations: &[Operation],
) -> nmr_core::Result<(Spectrum, Spectrum)> {
    let end = operations
        .iter()
        .position(|op| *op == Operation::Ft)
        .map_or(0, |i| i + 1);
    let base = process::process(raw, &operations[..end])?;
    let spectrum = process::process(&base, &operations[end..])?;
    Ok((base, spectrum))
}

/// An open 2D experiment: its spectrum as processed with the recorded
/// parameters, to draw contours of. Processing it is not editable yet, so
/// neither the raw data nor the operations are kept.
struct Document2D {
    path: PathBuf,
    /// Real part, row by row: one row per F1 point, each along F2.
    real: Vec<f64>,
    cols: usize,
    /// F2 and F1.
    axes: [PpmAxis; 2],
    nuclei: [String; 2],
    /// Spectrometer frequency of F2's nucleus and of F1's, in MHz.
    frequencies_mhz: [f64; 2],
    solvent: Option<String>,
    noise: f64,
    max: f64,
    /// Peaks can point down (phase-sensitive); in magnitude mode they cannot.
    signed: bool,
    /// The 2D seen from each side, along F2 and along F1: the largest absolute
    /// value of each column and of each row, as a 2D is usually drawn beside a
    /// projection when no spectrum of its own is given. Absolute, so the
    /// negative CH₂ peaks of an edited HSQC show as well.
    projections: [Vec<f64>; 2],
    /// Cross peaks picked, with what undo and redo restore.
    peaks: Vec<CrossPeak>,
    undo: Vec<Vec<CrossPeak>>,
    redo: Vec<Vec<CrossPeak>>,
}

/// A cross peak picked on a 2D, and the atoms of the structure it is assigned
/// to: for an HSQC the carbon and so its hydrogens, by position in the
/// molecule read from the molfile of the ¹H spectrum beside it.
#[derive(Clone, Serialize, Deserialize)]
struct CrossPeak {
    /// F2 and F1, in ppm.
    at: [f64; 2],
    #[serde(default)]
    atoms: Vec<usize>,
}

impl Document2D {
    fn open(path: PathBuf) -> nmr_core::Result<Self> {
        let raw = bruker::read_ser(&path)?;
        let s = process2d::process(&raw, &process2d::default_2d(&raw))?;
        let real: Vec<f64> = s.rows.iter().flatten().map(|z| z.re).collect();
        let cols = s.rows.first().map_or(0, Vec::len);
        let mut along_f2 = vec![0.0f64; cols];
        let mut along_f1 = vec![0.0f64; s.rows.len()];
        for (r, row) in real.chunks_exact(cols.max(1)).enumerate() {
            for (c, v) in row.iter().enumerate() {
                along_f2[c] = along_f2[c].max(v.abs());
                along_f1[r] = along_f1[r].max(v.abs());
            }
        }
        Ok(Self {
            projections: [along_f2, along_f1],
            noise: analysis::noise_level(&real),
            max: real.iter().fold(0.0, |m, v| m.max(v.abs())),
            cols,
            axes: s.ppm_axes(),
            nuclei: [s.f2.nucleus.clone(), s.f1.nucleus.clone()],
            frequencies_mhz: [s.f2.obs_mhz, s.f1.obs_mhz],
            solvent: s.f2.solvent.clone(),
            signed: !raw.magnitude,
            real,
            path,
            peaks: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }
}

/// What a slot holds: a spectrum, or a 2D. The two share one list so that an
/// id names one document whichever kind it is.
// A handful are open at a time; boxing to even out the sizes saves nothing.
#[allow(clippy::large_enum_variant)]
enum Slot {
    One(Document),
    Two(Document2D),
}

impl Slot {
    fn path(&self) -> &Path {
        match self {
            Self::One(doc) => &doc.path,
            Self::Two(doc) => &doc.path,
        }
    }
}

#[derive(Default)]
struct Documents(Mutex<Vec<Option<Slot>>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentInfo {
    id: usize,
    name: String,
    path: PathBuf,
    nucleus: String,
    /// Solvent and spectrometer frequency, for the experiment list.
    solvent: Option<String>,
    frequency_mhz: f64,
    points: usize,
    first_ppm: f64,
    delta_ppm: f64,
    min: f64,
    max: f64,
    noise: f64,
    revision: u64,
    operations: Vec<Operation>,
    analysis: Analysis,
    /// Normalised integral values, in the order of `analysis.integrals`.
    integral_values: Vec<Option<f64>>,
    /// Intensities at `analysis.peaks`.
    peak_intensities: Vec<f64>,
    /// How wide each picked line is, in Hz, in the order of `analysis.peaks`.
    peak_widths: Vec<f64>,
    /// What each multiplet range reads as.
    multiplet_info: Vec<MultipletInfo>,
    can_undo: bool,
    can_redo: bool,
}

/// A 2D experiment as the front end sees it: both axes, F2 first.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document2DInfo {
    id: usize,
    name: String,
    path: PathBuf,
    nuclei: [String; 2],
    solvent: Option<String>,
    /// Spectrometer frequency of F2's nucleus and of F1's, in MHz.
    frequencies_mhz: [f64; 2],
    /// Points along F2 and F1.
    points: [usize; 2],
    first_ppm: [f64; 2],
    delta_ppm: [f64; 2],
    noise: f64,
    /// Largest absolute value.
    max: f64,
    signed: bool,
    peaks: Vec<CrossPeak>,
    can_undo: bool,
    can_redo: bool,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct Opened {
    documents: Vec<DocumentInfo>,
    documents2d: Vec<Document2DInfo>,
    errors: Vec<String>,
}

/// Phase in degrees; `p0` is the phase at `pivot` (fraction of the spectrum).
#[derive(Serialize, Deserialize)]
struct PhaseParams {
    p0: f64,
    p1: f64,
    pivot: f64,
}

/// Opens every experiment found at `path`, reusing documents already open.
#[tauri::command(async)]
fn open(
    path: PathBuf,
    phase: bool,
    baseline: Option<Operation>,
    state: State<'_, Documents>,
) -> Result<Opened, String> {
    let experiments = bruker::find_experiments(&path).map_err(|e| e.to_string())?;
    let lock = || state.0.lock().map_err(|e| e.to_string());
    let mut opened = Opened::default();
    for exp in experiments {
        // Only a document still open counts as the same one; a closed slot is a
        // hole and its spectrum is read again from disk.
        let open_at = |docs: &[Option<Slot>]| {
            docs.iter()
                .position(|d| d.as_ref().is_some_and(|doc| doc.path() == exp))
        };
        let existing = open_at(&lock()?);
        let id = match existing {
            Some(id) => id,
            None => {
                // Read without holding the documents: a non-uniformly sampled 2D
                // takes seconds to fill in, and everything else would wait.
                let read = if exp.join("acqu2s").is_file() && exp.join("ser").is_file() {
                    Document2D::open(exp.clone()).map(Slot::Two)
                } else {
                    Document::open(exp.clone(), phase, baseline.clone()).map(Slot::One)
                };
                let doc = match read {
                    Ok(doc) => doc,
                    Err(e) => {
                        opened.errors.push(format!("{}: {e}", name(&exp)));
                        continue;
                    }
                };
                let mut docs = lock()?;
                // Opened meanwhile by another call: that one stands.
                match (open_at(&docs), docs.iter().position(Option::is_none)) {
                    (Some(id), _) => id,
                    (None, Some(hole)) => {
                        docs[hole] = Some(doc);
                        hole
                    }
                    (None, None) => {
                        docs.push(Some(doc));
                        docs.len() - 1
                    }
                }
            }
        };
        match lock()?.get(id).and_then(Option::as_ref) {
            Some(Slot::One(doc)) => opened.documents.push(info(id, doc)),
            Some(Slot::Two(doc)) => opened.documents2d.push(info_2d(id, doc)),
            None => {}
        }
    }
    Ok(opened)
}

#[tauri::command(async)]
fn set_operations(
    id: usize,
    operations: Vec<Operation>,
    state: State<'_, Documents>,
) -> Result<DocumentInfo, String> {
    with_document(&state, id, |doc| {
        let mut analysis = doc.state.analysis.clone();
        if operations != doc.state.operations {
            analysis.peak_fit = None;
        }
        doc.change(DocumentState {
            operations,
            analysis,
        })
    })
}

/// `amend` folds the change into the last one rather than recording a step of
/// its own, for a change made in two calls that the user asked for as one.
#[tauri::command(async)]
fn set_analysis(
    id: usize,
    mut analysis: Analysis,
    amend: Option<bool>,
    state: State<'_, Documents>,
) -> Result<DocumentInfo, String> {
    with_document(&state, id, |doc| {
        if analysis.peaks != doc.state.analysis.peaks {
            analysis.peak_fit = None;
        }
        let next = DocumentState {
            operations: doc.state.operations.clone(),
            analysis,
        };
        if amend == Some(true) && !doc.undo.is_empty() {
            doc.replace(next)?;
            doc.redo.clear();
            Ok(())
        } else {
            doc.change(next)
        }
    })
}

#[tauri::command(async)]
fn undo(id: usize, state: State<'_, Documents>) -> Result<DocumentInfo, String> {
    with_document(&state, id, |doc| {
        if let Some(previous) = doc.undo.pop() {
            let current = doc.replace(previous)?;
            doc.redo.push(current);
        }
        Ok(())
    })
}

#[tauri::command(async)]
fn redo(id: usize, state: State<'_, Documents>) -> Result<DocumentInfo, String> {
    with_document(&state, id, |doc| {
        if let Some(next) = doc.redo.pop() {
            let current = doc.replace(next)?;
            doc.undo.push(current);
        }
        Ok(())
    })
}

/// Decimated real part of a document as little-endian `f32` pairs.
#[tauri::command(async)]
fn trace(
    id: usize,
    start: usize,
    end: usize,
    columns: usize,
    state: State<'_, Documents>,
) -> Result<Response, String> {
    let docs = state.0.lock().map_err(|e| e.to_string())?;
    let doc = document(&docs, id)?;
    Ok(f32_response(&decimate::min_max(
        &doc.real,
        start..end,
        columns,
    )))
}

/// Like `trace`, but with `phase` in place of the document's phase step and
/// every step after it replayed, so the baseline is taken off as Apply will
/// take it: phasing against a rolling baseline misleads, and a baseline kept
/// from another phase leaves dips that are not there. At 131k points: 17 ms
/// with the Whittaker smoother, 19 with airPLS, 2-14 through points, 180 with
/// the iterated polynomial, whose preview then lags but is never wrong.
#[tauri::command(async)]
fn phase_trace(
    id: usize,
    phase: PhaseParams,
    start: usize,
    end: usize,
    columns: usize,
    state: State<'_, Documents>,
) -> Result<Response, String> {
    // Copied out, so other commands are not held up while this one replays.
    let (base, after) = {
        let docs = state.0.lock().map_err(|e| e.to_string())?;
        let doc = document(&docs, id)?;
        let ops = &doc.state.operations;
        let from = ops
            .iter()
            .position(|op| matches!(op, Operation::Phase { .. }))
            .or_else(|| ops.iter().position(|op| *op == Operation::Ft))
            .map_or(ops.len(), |i| i + 1);
        (doc.phase_base.clone(), ops[from..].to_vec())
    };
    let steps: Vec<Operation> = std::iter::once(Operation::Phase {
        p0: phase.p0,
        p1: phase.p1,
        pivot: phase.pivot,
    })
    .chain(after)
    .collect();
    let spectrum = process::process(&base, &steps).map_err(|e| e.to_string())?;
    let end = end.min(spectrum.data.len());
    let real: Vec<f64> = spectrum.data[start.min(end)..end]
        .iter()
        .map(|z| z.re)
        .collect();
    Ok(f32_response(&decimate::min_max(
        &real,
        0..real.len(),
        columns,
    )))
}

#[tauri::command(async)]
fn largest_peak(id: usize, state: State<'_, Documents>) -> Result<f64, String> {
    let docs = state.0.lock().map_err(|e| e.to_string())?;
    Ok(phase::largest_peak(&document(&docs, id)?.phase_base.data))
}

/// Automatic phase correction on top of `current`: what is left to correct once
/// `current` is applied, both orders.
#[tauri::command(async)]
fn auto_phase(
    id: usize,
    current: PhaseParams,
    state: State<'_, Documents>,
) -> Result<PhaseParams, String> {
    // Copy the data so other commands are not blocked while optimising.
    let (mut data, meta) = {
        let docs = state.0.lock().map_err(|e| e.to_string())?;
        let base = &document(&docs, id)?.phase_base;
        (base.data.clone(), base.meta.clone())
    };
    phase::apply(&mut data, current.p0, current.p1, current.pivot);
    let (p0, p1) = phase::auto_phase(&data, &meta);
    Ok(PhaseParams {
        p0: current.p0 + p0 + p1 * current.pivot,
        p1: current.p1 + p1,
        pivot: current.pivot,
    })
}

fn is_baseline(op: &Operation) -> bool {
    matches!(
        op,
        Operation::BaselineAirpls { .. }
            | Operation::BaselineWhittaker { .. }
            | Operation::BaselinePolynomial { .. }
            | Operation::BaselinePoints { .. }
    )
}

/// The document's spectrum with `baseline` in place of its baseline step (or
/// after the last step when it has none), or with the step taken out, replayed
/// from the transformed spectrum on a copy so no command waits on it.
fn with_baseline(
    state: &State<'_, Documents>,
    id: usize,
    baseline: Option<Operation>,
) -> Result<Vec<f64>, String> {
    let (base, mut ops) = {
        let docs = state.0.lock().map_err(|e| e.to_string())?;
        let doc = document(&docs, id)?;
        let ops = &doc.state.operations;
        let from = ops
            .iter()
            .position(|op| *op == Operation::Ft)
            .map_or(0, |i| i + 1);
        (doc.phase_base.clone(), ops[from..].to_vec())
    };
    let at = ops.iter().position(is_baseline);
    if let Some(i) = at {
        ops.remove(i);
    }
    if let Some(b) = baseline {
        ops.insert(at.unwrap_or(ops.len()), b);
    }
    let spectrum = process::process(&base, &ops).map_err(|e| e.to_string())?;
    Ok(real_part(&spectrum))
}

/// Like `trace`, with `baseline` in place of the baseline step, or none.
#[tauri::command(async)]
fn baseline_trace(
    id: usize,
    baseline: Option<Operation>,
    start: usize,
    end: usize,
    columns: usize,
    state: State<'_, Documents>,
) -> Result<Response, String> {
    let real = with_baseline(&state, id, baseline)?;
    let end = end.min(real.len());
    Ok(f32_response(&decimate::min_max(
        &real,
        start.min(end)..end,
        columns,
    )))
}

/// A hand-set baseline as the plot draws it while its points are set.
#[derive(Serialize)]
struct HandSet {
    /// For each point, in the order given: the value it stands at, and the
    /// baseline there (the same for segments and splines).
    points: Vec<[f64; 2]>,
    /// The baseline at `samples` points evenly from `start` to `end`.
    curve: Vec<f64>,
    /// What a freely placed point's `y` is a fraction of.
    top: f64,
}

#[tauri::command(async)]
fn hand_set_baseline(
    id: usize,
    baseline: Operation,
    start: usize,
    end: usize,
    samples: usize,
    state: State<'_, Documents>,
) -> Result<HandSet, String> {
    let real = with_baseline(&state, id, None)?;
    let n = real.len();
    let (anchors, curve) =
        process::hand_set(&real, &baseline).ok_or("Not a baseline through points")?;
    let Operation::BaselinePoints { points, .. } = &baseline else {
        return Err("Not a baseline through points".into());
    };
    let end = end.min(n.saturating_sub(1));
    let start = start.min(end);
    let samples = samples.max(2);
    Ok(HandSet {
        points: points
            .iter()
            .map(|p| {
                let i = process::index_at(p.at, n);
                let at = anchors.partition_point(|a| a.0 < i as f64);
                [anchors.get(at).map_or(0.0, |a| a.1), curve[i]]
            })
            .collect(),
        curve: (0..samples)
            .map(|k| curve[start + (end - start) * k / (samples - 1)])
            .collect(),
        top: real.iter().fold(0.0, |m: f64, v| m.max(v.abs())),
    })
}

/// Points to start a hand-set baseline from, as fractions of the spectrum,
/// spread through the stretches the automatic baseline finds free of signal.
#[tauri::command(async)]
fn propose_baseline_points(id: usize, state: State<'_, Documents>) -> Result<Vec<f64>, String> {
    let (sw_hz, axis) = {
        let docs = state.0.lock().map_err(|e| e.to_string())?;
        let spectrum = &document(&docs, id)?.spectrum;
        (spectrum.meta.sw_hz, spectrum.ppm_axis())
    };
    let real = with_baseline(&state, id, None)?;
    let n = real.len();
    // As the automatic baseline recognises signal, broad lines included.
    let wide =
        nmr_core::baseline::BROAD_LINE_FILTER * nmr_core::baseline::signal_width(&real, &axis);
    let dilation = (nmr_core::baseline::COBAS_FILTER_HZ * n as f64 / sw_hz)
        .max(wide)
        .round() as usize;
    Ok(nmr_core::baseline::propose_points(
        &real,
        process::WHITTAKER_LAMBDA,
        dilation,
        PROPOSED_POINTS,
    )
    .into_iter()
    .map(|i| i as f64 / n as f64)
    .collect())
}

/// Enough to follow a rolling baseline, few enough to take hold of one by one.
const PROPOSED_POINTS: usize = 40;

/// ppm positions of peaks above `threshold` between two ppm limits.
#[tauri::command(async)]
fn find_peaks(
    id: usize,
    from: f64,
    to: f64,
    threshold: f64,
    state: State<'_, Documents>,
) -> Result<Vec<f64>, String> {
    let docs = state.0.lock().map_err(|e| e.to_string())?;
    let doc = document(&docs, id)?;
    let axis = doc.spectrum.ppm_axis();
    let peaks = analysis::find_peaks(&doc.real, &axis, from, to, threshold, PEAK_SEPARATION);
    Ok(peaks.iter().map(|p| p.ppm).collect())
}

#[tauri::command(async)]
fn auto_peaks(
    id: usize,
    threshold: f64,
    method: PeakMethod,
    state: State<'_, Documents>,
) -> Result<DocumentInfo, String> {
    analyse_peaks(id, threshold, Some(method), false, &state)
}

#[tauri::command(async)]
fn fit_peaks(
    id: usize,
    threshold: f64,
    amend: Option<bool>,
    state: State<'_, Documents>,
) -> Result<DocumentInfo, String> {
    analyse_peaks(id, threshold, None, amend == Some(true), &state)
}

/// Fit outside the document lock, then commit only if its source is unchanged.
fn analyse_peaks(
    id: usize,
    threshold: f64,
    method: Option<PeakMethod>,
    amend: bool,
    state: &State<'_, Documents>,
) -> Result<DocumentInfo, String> {
    if !threshold.is_finite() || threshold < 0.0 {
        return Err("The peak threshold must be finite and nonnegative".into());
    }
    let (real, axis, meta, revision, before) = {
        let docs = state.0.lock().map_err(|e| e.to_string())?;
        let doc = document(&docs, id)?;
        (
            doc.real.clone(),
            doc.spectrum.ppm_axis(),
            doc.spectrum.meta.clone(),
            doc.revision,
            doc.state.analysis.clone(),
        )
    };
    if method.is_none() && before.peaks.is_empty() {
        return Err("Pick peaks before fitting them".into());
    }
    let mut next = before.clone();
    if let Some(method) = method {
        next.peak_method = method;
        let magnitude = meta
            .signed
            .then(|| real.iter().map(|y| y.abs()).collect::<Vec<_>>());
        let detection = magnitude.as_deref().unwrap_or(&real);
        let detected = analysis::find_peaks(
            detection,
            &axis,
            axis.first,
            axis.ppm(real.len() as f64),
            threshold,
            PEAK_SEPARATION,
        );
        match method {
            PeakMethod::Standard => {
                next.peaks = detected.iter().map(|p| p.ppm).collect();
                next.peak_fit = None;
            }
            PeakMethod::Deconvolution => {
                let fit = deconvolution::deconvolve(
                    &real,
                    &axis,
                    meta.obs_mhz,
                    threshold,
                    meta.signed,
                    None,
                );
                next.peaks = fit.peaks.iter().map(|p| p.ppm).collect();
                // A failed fit must not make an already detected signal disappear.
                next.peaks.extend(detected.iter().filter_map(|p| {
                    (!fit.regions.iter().any(|r| {
                        r.status == deconvolution::FitStatus::Converged
                            && p.ppm >= r.from.min(r.to)
                            && p.ppm <= r.from.max(r.to)
                    }))
                    .then_some(p.ppm)
                }));
                next.peak_fit = Some(fit);
            }
        }
        next.peaks.sort_by(|a, b| b.total_cmp(a));
        next.peaks
            .dedup_by(|a, b| (*a - *b).abs() < axis.delta.abs());
        next.labels = next
            .peaks
            .iter()
            .map(|p| {
                before
                    .peaks
                    .iter()
                    .position(|old| (old - p).abs() <= axis.delta.abs())
                    .and_then(|i| before.labels.get(i).cloned().flatten())
            })
            .collect();
    } else {
        next.peak_fit = Some(deconvolution::deconvolve(
            &real,
            &axis,
            meta.obs_mhz,
            threshold,
            meta.signed,
            Some(&before.peaks),
        ));
    }
    let mut docs = state.0.lock().map_err(|e| e.to_string())?;
    let doc = match docs.get_mut(id).and_then(Option::as_mut) {
        Some(Slot::One(doc)) => doc,
        _ => return Err("This spectrum is no longer open".into()),
    };
    if doc.revision != revision || doc.state.analysis != before {
        return Err("The spectrum changed while fitting peaks; try again".into());
    }
    let next = DocumentState {
        operations: doc.state.operations.clone(),
        analysis: next,
    };
    if amend && !doc.undo.is_empty() {
        doc.replace(next).map_err(|e| e.to_string())?;
        doc.redo.clear();
    } else {
        doc.change(next).map_err(|e| e.to_string())?;
    }
    Ok(info(id, doc))
}

/// Regions around the accepted peaks; detect peaks only when none were picked.
#[tauri::command(async)]
fn signal_regions(
    id: usize,
    from: f64,
    to: f64,
    threshold: f64,
    state: State<'_, Documents>,
) -> Result<Vec<(f64, f64)>, String> {
    let docs = state.0.lock().map_err(|e| e.to_string())?;
    let doc = document(&docs, id)?;
    let meta = &doc.spectrum.meta;
    let axis = doc.spectrum.ppm_axis();
    let reading = analysis::Reading {
        threshold,
        separation: PEAK_SEPARATION,
        max_j_hz: max_coupling(&meta.nucleus),
    };
    let magnitude = meta
        .signed
        .then(|| doc.real.iter().map(|y| y.abs()).collect::<Vec<_>>());
    let detection = magnitude.as_deref().unwrap_or(&doc.real);
    Ok(if doc.state.analysis.peaks.is_empty() {
        analysis::integral_regions(detection, &axis, meta.obs_mhz, from, to, reading)
    } else {
        analysis::integral_regions_from_peaks(
            detection,
            &axis,
            meta.obs_mhz,
            from,
            to,
            reading,
            &doc.state.analysis.peaks,
        )
    })
}

/// ppm of the highest point within `radius` points of `ppm`.
#[tauri::command(async)]
fn nearest_peak(
    id: usize,
    ppm: f64,
    radius: usize,
    state: State<'_, Documents>,
) -> Result<Option<f64>, String> {
    let docs = state.0.lock().map_err(|e| e.to_string())?;
    let doc = document(&docs, id)?;
    let peak = analysis::nearest_peak(&doc.real, &doc.spectrum.ppm_axis(), ppm, radius);
    Ok(peak.map(|p| p.ppm))
}

/// Running integral as little-endian `f32` `ppm, value` pairs.
#[tauri::command(async)]
fn integral_curve(
    id: usize,
    from: f64,
    to: f64,
    samples: usize,
    state: State<'_, Documents>,
) -> Result<Response, String> {
    let docs = state.0.lock().map_err(|e| e.to_string())?;
    let doc = document(&docs, id)?;
    if doc.state.analysis.integration_method == IntegrationMethod::Fitted {
        return Err("Cumulative curves are available for Sum areas".into());
    }
    let axis = doc.spectrum.ppm_axis();
    Ok(f32_response(&analysis::integral_curve(
        &doc.real, &axis, from, to, samples,
    )))
}

/// Replaces operations and analysis together as one undo step, for loading projects.
#[tauri::command(async)]
fn set_state(
    id: usize,
    operations: Vec<Operation>,
    analysis: Analysis,
    state: State<'_, Documents>,
) -> Result<DocumentInfo, String> {
    with_document(&state, id, |doc| {
        doc.change(DocumentState {
            operations,
            analysis,
        })
    })
}

/// Base64, to hand a drawing over as a data URL. Writing it out is shorter than
/// taking a dependency for it.
///
/// Only macOS reads a drawing off the pasteboard, so the encoder is otherwise
/// unused; the round-trip test below still needs it on every platform.
#[cfg(any(target_os = "macos", test))]
fn base64(bytes: &[u8]) -> String {
    const SET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(SET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The bytes of a `data:` URL, which is how the page holds a structure drawing.
fn from_data_url(url: &str) -> Option<Vec<u8>> {
    let (_, text) = url.split_once(";base64,")?;
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let digits: Vec<u8> = text
        .bytes()
        .filter(|c| *c != b'=')
        .map(value)
        .collect::<Option<_>>()?;
    let mut out = Vec::with_capacity(digits.len() * 3 / 4);
    for chunk in digits.chunks(4) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, d)| n | u32::from(*d) << (18 - 6 * i));
        out.extend(n.to_be_bytes()[1..chunk.len()].iter());
    }
    Some(out)
}

/// The font families installed on this computer, by name. A web view cannot list
/// them, and the PDF is set with these same fonts. Read once: looking through
/// every font file takes a moment.
#[tauri::command(async)]
fn system_fonts() -> Vec<String> {
    static FONTS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut names: Vec<String> = pdf::fonts()
                .faces()
                .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
                // A leading dot marks a family the system keeps for itself.
                .filter(|name| !name.starts_with('.'))
                .collect();
            names.sort_by_key(|name| name.to_lowercase());
            names.dedup();
            names
        })
        .clone()
}

/// Where each of documents `ids` goes inside a project file, in that order, which
/// is what the project's own list of documents points to.
#[tauri::command(async)]
fn project_layout(ids: Vec<usize>, state: State<'_, Documents>) -> Result<Vec<String>, String> {
    let docs = state.0.lock().map_err(|e| e.to_string())?;
    ids.iter()
        .enumerate()
        .map(|(k, &id)| Ok(project::inner_path(k, slot(&docs, id)?.path())))
        .collect()
}

/// A file kept in a project beside the data: a structure drawing, by its place
/// in the archive.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectExtra {
    name: String,
    data_url: String,
}

/// Writes a project with the data of documents `ids` in it.
#[tauri::command(async)]
fn save_project(
    path: PathBuf,
    manifest: String,
    ids: Vec<usize>,
    extras: Vec<ProjectExtra>,
    state: State<'_, Documents>,
) -> Result<(), String> {
    write_project(&path, &manifest, &ids, extras, &state)
}

fn write_project(
    path: &Path,
    manifest: &str,
    ids: &[usize],
    extras: Vec<ProjectExtra>,
    state: &State<'_, Documents>,
) -> Result<(), String> {
    let sources = {
        let docs = state.0.lock().map_err(|e| e.to_string())?;
        ids.iter()
            .map(|&id| Ok(slot(&docs, id)?.path().to_path_buf()))
            .collect::<Result<Vec<_>, String>>()?
    };
    let extras = extras
        .into_iter()
        .map(|e| {
            let bytes = from_data_url(&e.data_url).ok_or(format!("{} cannot be read", e.name))?;
            Ok((e.name, bytes))
        })
        .collect::<Result<Vec<_>, String>>()?;
    project::write(path, manifest, &sources, &extras)
}

/// A project to attach to a PDF, as `save_project` takes one.
#[derive(Deserialize)]
struct Attachment {
    manifest: String,
    ids: Vec<usize>,
    extras: Vec<ProjectExtra>,
}

/// Writes `pages` (SVG) as one PDF at `path`, with the project attached when
/// one is given, so the PDF opens as the project again.
#[tauri::command(async)]
fn save_pdf(
    path: PathBuf,
    pages: Vec<String>,
    project: Option<Attachment>,
    state: State<'_, Documents>,
) -> Result<(), String> {
    let project = match project {
        Some(p) => {
            let scratch = std::env::temp_dir().join(format!(
                "opennmr-attach-{}-{}.opennmr",
                std::process::id(),
                path.file_stem()
                    .map(|s| s.to_string_lossy())
                    .unwrap_or_default()
            ));
            write_project(&scratch, &p.manifest, &p.ids, p.extras, &state)?;
            let bytes = std::fs::read(&scratch).map_err(|e| e.to_string());
            let _ = std::fs::remove_file(&scratch);
            Some(bytes?)
        }
        None => None,
    };
    let pdf = pdf::write(&pages, project)?;
    std::fs::write(&path, pdf).map_err(|e| format!("Cannot write {}: {e}", path.display()))
}

/// Where the session is kept while it runs: removed when the app closes, so it
/// is still there after a crash and the start screen offers it back.
fn recovery_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("recovery.json"))
}

/// Keeps `manifest` (a project that points to its data on disk) as the session
/// to recover, or forgets it when there is nothing open.
#[tauri::command(async)]
fn keep_session(app: tauri::AppHandle, manifest: Option<String>) -> Result<(), String> {
    let file = recovery_file(&app)?;
    match manifest {
        // Written beside it and renamed, so a crash mid-write leaves the last one.
        Some(text) => {
            let partial = file.with_extension("partial");
            std::fs::write(&partial, text).map_err(|e| e.to_string())?;
            std::fs::rename(&partial, &file).map_err(|e| e.to_string())
        }
        None => match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        },
    }
}

/// The session the app was running when it last stopped without closing.
#[tauri::command(async)]
fn last_session(app: tauri::AppHandle) -> Result<Option<String>, String> {
    match std::fs::read_to_string(recovery_file(&app)?) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[derive(Serialize)]
struct OpenedProject {
    manifest: String,
    /// Where the data was unpacked; none for a project that points to it on disk.
    root: Option<PathBuf>,
}

/// Reads a project file, unpacking the data it carries into a folder of its own
/// under the system's temporary folder.
#[tauri::command(async)]
fn read_project(path: PathBuf) -> Result<OpenedProject, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let into = std::env::temp_dir()
        .join("opennmr")
        .join(format!("{}-{stamp}", std::process::id()));
    // A PDF written here carries its project inside; that is what is opened.
    let bytes = std::fs::read(&path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    let opened = if bytes.starts_with(b"%PDF") {
        let zip = pdf::attached(&bytes)
            .ok_or(format!("{} carries no openNMR project", path.display()))?;
        std::fs::create_dir_all(&into).map_err(|e| e.to_string())?;
        let inner = into.join(pdf::ATTACHMENT);
        std::fs::write(&inner, zip).map_err(|e| e.to_string())?;
        project::open(&inner, &into)?
    } else {
        project::open(&path, &into)?
    };
    Ok(OpenedProject {
        manifest: opened.manifest,
        root: opened.root,
    })
}

/// What the pasteboard holds in `class`, if anything: osascript writes it out as
/// `«data PDF 2550…»`, which is the bytes in hex.
#[cfg(target_os = "macos")]
fn pasteboard(class: &str) -> Option<Vec<u8>> {
    let out = std::process::Command::new("osascript")
        .args(["-e", &format!("the clipboard as «class {class}»")])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let hex = text
        .trim()
        .strip_prefix('«')?
        .strip_suffix('»')?
        .split_whitespace()
        .last()?
        .to_string();
    let bytes: Option<Vec<u8>> = hex
        .as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect();
    bytes.filter(|b| !b.is_empty())
}

/// A drawing off the clipboard, as a data URL.
///
/// The web view hands a page no flavours at all when the copy came from another
/// application - measured: pasting from ChemDraw offered nothing whatsoever - so
/// the pasteboard is read here instead of in the page.
#[tauri::command(async)]
fn clipboard_structure() -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    // A molfile first, when the copy was one: it is the structure itself, where the
    // pictures below are only how it looks. Copy as MOL text in a drawing program
    // puts it on the pasteboard as plain text, which `pasteboard` cannot read: for
    // text osascript prints the text itself rather than `«data …»`.
    if let Some(text) = std::process::Command::new("pbpaste")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        && text.contains("V2000")
    {
        return Ok(Some(format!(
            "data:chemical/x-mdl-molfile;base64,{}",
            base64(text.as_bytes())
        )));
    }
    #[cfg(target_os = "macos")]
    // A picture first. ChemDraw puts one on the pasteboard beside the PDF, and a
    // PDF in a data URL is the least likely of the three to draw in a web view.
    for (class, mime) in [
        ("PNGf", "image/png"),
        ("TIFF", "image/tiff"),
        ("PDF ", "application/pdf"),
    ] {
        if let Some(bytes) = pasteboard(class) {
            return Ok(Some(format!("data:{mime};base64,{}", base64(&bytes))));
        }
    }
    Ok(None)
}

/// A molfile read: its atoms, bonds and the hydrogens each carries.
#[tauri::command]
fn read_molecule(text: String) -> Result<nmr_core::molecule::Molecule, String> {
    nmr_core::molecule::parse_molfile(&text).map_err(|e| e.to_string())
}

/// Reads a file as it stands, for a structure: a PDF or a PNG is not text.
#[tauri::command(async)]
fn read_bytes(path: PathBuf) -> Result<Response, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    Ok(Response::new(bytes))
}

/// Writes an exported file. The body is the raw content; the `path` header holds the
/// destination, percent-encoded because header values must be ASCII.
#[tauri::command]
fn save_file(request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let tauri::ipc::InvokeBody::Raw(content) = request.body() else {
        return Err("The file content was not sent as bytes".into());
    };
    let path = request
        .headers()
        .get("path")
        .and_then(|value| value.to_str().ok())
        .and_then(percent_decode)
        .ok_or("No valid destination path was given")?;
    std::fs::write(&path, content).map_err(|e| format!("Cannot write {path}: {e}"))
}

/// Decodes the output of JavaScript's `encodeURIComponent`.
fn percent_decode(encoded: &str) -> Option<String> {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            decoded.push(u8::from_str_radix(encoded.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn slot(docs: &[Option<Slot>], id: usize) -> Result<&Slot, String> {
    docs.get(id)
        .and_then(Option::as_ref)
        .ok_or_else(|| "This document is no longer open".to_string())
}

fn document(docs: &[Option<Slot>], id: usize) -> Result<&Document, String> {
    match slot(docs, id)? {
        Slot::One(doc) => Ok(doc),
        Slot::Two(_) => Err("This is a 2D experiment, not a spectrum".to_string()),
    }
}

fn document_2d(docs: &[Option<Slot>], id: usize) -> Result<&Document2D, String> {
    match slot(docs, id)? {
        Slot::Two(doc) => Ok(doc),
        Slot::One(_) => Err("This is a spectrum, not a 2D experiment".to_string()),
    }
}

/// Contour lines of a 2D at each of `levels`, as little-endian `f32`: first,
/// for each level, how many values its lines take, then the lines themselves,
/// level by level, as `x0, y0, x1, y1` in points along F2 and F1.
#[tauri::command(async)]
fn contours(id: usize, levels: Vec<f64>, state: State<'_, Documents>) -> Result<Response, String> {
    let docs = state.0.lock().map_err(|e| e.to_string())?;
    let doc = document_2d(&docs, id)?;
    let lines = contour::at_levels(&doc.real, doc.cols, &levels);
    let mut values: Vec<f32> = lines.iter().map(|l| l.len() as f32).collect();
    values.extend(lines.concat());
    Ok(f32_response(&values))
}

/// What the dataset says about how it was recorded.
#[tauri::command(async)]
fn metadata(id: usize, state: State<'_, Documents>) -> Result<Vec<(String, String)>, String> {
    let path = {
        let docs = state.0.lock().map_err(|e| e.to_string())?;
        slot(&docs, id)?.path().to_path_buf()
    };
    bruker::describe(&path).map_err(|e| e.to_string())
}

/// Shift, couplings and line width of multiplet `index` fitted to the spectrum
/// together, from what it reads as now. `None` when it has no first-order
/// pattern to fit.
#[tauri::command(async)]
fn fit_multiplet(
    id: usize,
    index: usize,
    state: State<'_, Documents>,
) -> Result<Option<MultipletFit>, String> {
    let docs = state.0.lock().map_err(|e| e.to_string())?;
    let doc = document(&docs, id)?;
    let axis = doc.spectrum.ppm_axis();
    let infos = multiplets_of(doc, &axis);
    let (Some(m), Some(now)) = (doc.state.analysis.multiplets.get(index), infos.get(index)) else {
        return Err("No such multiplet".into());
    };
    Ok(fit::fit_multiplet(
        &doc.real,
        &axis,
        doc.spectrum.meta.obs_mhz,
        m.from,
        m.to,
        (now.ppm, &now.kind, &now.js_hz, now.width_hz),
        fit::Free::All,
    )
    .map(|f| MultipletFit {
        ppm: f.ppm,
        js_hz: f.js_hz,
        width_hz: f.width_hz,
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MultipletFit {
    ppm: f64,
    js_hz: Vec<f64>,
    width_hz: f64,
}

/// Forgets a document, so that opening its folder again reads it afresh rather
/// than handing back the integrals and peaks it carried before it was closed.
#[tauri::command(async)]
fn close(id: usize, state: State<'_, Documents>) -> Result<(), String> {
    let mut docs = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(slot) = docs.get_mut(id) {
        *slot = None;
    }
    Ok(())
}

fn with_document(
    state: &Documents,
    id: usize,
    change: impl FnOnce(&mut Document) -> nmr_core::Result<()>,
) -> Result<DocumentInfo, String> {
    let mut docs = state.0.lock().map_err(|e| e.to_string())?;
    let doc = match docs.get_mut(id).and_then(Option::as_mut) {
        Some(Slot::One(doc)) => doc,
        Some(Slot::Two(_)) => return Err("This is a 2D experiment, not a spectrum".into()),
        None => return Err("This document is no longer open".into()),
    };
    change(doc).map_err(|e| e.to_string())?;
    Ok(info(id, doc))
}

fn f32_response(values: &[f32]) -> Response {
    Response::new(
        values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>(),
    )
}

fn real_part(spectrum: &Spectrum) -> Vec<f64> {
    spectrum.data.iter().map(|z| z.re).collect()
}

/// Both the measurement and its normalization use the selected calculation.
fn selected_area(doc: &Document, axis: &PpmAxis, from: f64, to: f64) -> Option<f64> {
    match doc.state.analysis.integration_method {
        IntegrationMethod::Sum => Some(analysis::integrate(&doc.real, axis, from, to)),
        IntegrationMethod::Fitted => doc.state.analysis.peak_fit.as_ref()?.integrate(from, to),
    }
    .filter(|v| v.is_finite())
}

fn integral_scale(doc: &Document, axis: &PpmAxis) -> Option<f64> {
    let analysis = &doc.state.analysis;
    let (index, value) = analysis
        .reference
        .filter(|r| r.index < analysis.integrals.len())
        .map_or((0, 1.0), |r| (r.index, r.value));
    let [from, to] = *analysis.integrals.get(index)?;
    let area = selected_area(doc, axis, from, to)?;
    (area != 0.0)
        .then_some(value / area)
        .filter(|v| v.is_finite())
}

fn integral_values(doc: &Document, axis: &PpmAxis) -> Vec<Option<f64>> {
    let scale = integral_scale(doc, axis);
    doc.state
        .analysis
        .integrals
        .iter()
        .map(|[a, b]| {
            selected_area(doc, axis, *a, *b)
                .zip(scale)
                .map(|(area, scale)| area * scale)
                .filter(|v| v.is_finite())
        })
        .collect()
}

/// The area worth calling one nucleus: the smallest that is not a sliver.
///
/// Taking the smallest outright made the unit whatever had scraped past the
/// threshold. Measured on the dataset in testdata, a blip a hundredth of a ppm
/// wide became 1H and every real signal came out in the forties, which is
/// proportional to the area and of no use to anyone.
fn one_nucleus(areas: &[f64]) -> Option<f64> {
    let mut sorted: Vec<f64> = areas.iter().copied().filter(|a| *a > 0.0).collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    sorted.into_iter().find(|a| *a >= median / 3.0)
}

/// Reads every multiplet range against the spectrum as it stands now.
fn multiplets_of(doc: &Document, axis: &PpmAxis) -> Vec<MultipletInfo> {
    let analysis = &doc.state.analysis;
    let areas: Vec<Option<f64>> = analysis
        .multiplets
        .iter()
        .map(|m| selected_area(doc, axis, m.from, m.to))
        .collect();
    // With integrals on the spectrum the multiplets share their scale, so the two
    // tables agree. With none, the smallest multiplet is one nucleus, which is how
    // a spectrum is read by hand.
    let scale = if analysis.integrals.is_empty() {
        one_nucleus(&areas.iter().flatten().copied().collect::<Vec<_>>()).map(|unit| 1.0 / unit)
    } else {
        integral_scale(doc, axis)
    };
    let threshold = doc.noise * MULTIPLET_NOISE;
    analysis
        .multiplets
        .iter()
        .zip(areas)
        .map(|(m, area)| {
            let read = analysis::multiplet(
                &doc.real,
                axis,
                doc.spectrum.meta.obs_mhz,
                m.from,
                m.to,
                analysis::Reading {
                    threshold,
                    separation: PEAK_SEPARATION,
                    max_j_hz: max_coupling(&doc.spectrum.meta.nucleus),
                },
            );
            // What was typed wins over what was read, so the table, the marks
            // and the report all quote the same thing.
            let ppm = m.ppm.unwrap_or(read.ppm);
            let kind = m.kind.clone().unwrap_or(read.kind);
            let js_hz = m.js_hz.clone().unwrap_or(read.js_hz);
            let measured = if read.width_hz > 0.0 {
                read.width_hz
            } else {
                1.0
            };
            // The width the whole multiplet fits best with, where it has a
            // first-order pattern to fit; measured at half height otherwise. A
            // width typed in is only measured against the spectrum.
            let fitted = fit::fit_multiplet(
                &doc.real,
                axis,
                doc.spectrum.meta.obs_mhz,
                m.from,
                m.to,
                (ppm, &kind, &js_hz, m.width_hz.unwrap_or(measured)),
                if m.width_hz.is_some() {
                    fit::Free::Nothing
                } else {
                    fit::Free::Width
                },
            );
            let width_hz = m
                .width_hz
                .unwrap_or_else(|| fitted.as_ref().map_or(measured, |f| f.width_hz));
            MultipletInfo {
                ppm,
                kind,
                js_hz,
                lines: read.lines,
                value: area
                    .zip(scale)
                    .map(|(area, scale)| area * scale)
                    .filter(|v| v.is_finite()),
                width_hz,
                explained: fitted.map(|f| f.explained),
            }
        })
        .collect()
}

fn info(id: usize, doc: &Document) -> DocumentInfo {
    let axis = doc.spectrum.ppm_axis();
    let (min, max) = doc
        .real
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &y| {
            (lo.min(y), hi.max(y))
        });
    DocumentInfo {
        id,
        name: name(&doc.path),
        path: doc.path.clone(),
        nucleus: doc.spectrum.meta.nucleus.clone(),
        solvent: doc.spectrum.meta.solvent.clone(),
        frequency_mhz: doc.spectrum.meta.obs_mhz,
        points: doc.real.len(),
        first_ppm: axis.first,
        delta_ppm: axis.delta,
        min,
        max,
        noise: doc.noise,
        revision: doc.revision,
        operations: doc.state.operations.clone(),
        integral_values: integral_values(doc, &axis),
        peak_intensities: doc
            .state
            .analysis
            .peaks
            .iter()
            .map(|&ppm| analysis::intensity_at(&doc.real, &axis, ppm))
            .collect(),
        peak_widths: doc
            .state
            .analysis
            .peaks
            .iter()
            .map(|&ppm| analysis::line_width_hz(&doc.real, &axis, doc.spectrum.meta.obs_mhz, ppm))
            .collect(),
        multiplet_info: multiplets_of(doc, &axis),
        analysis: doc.state.analysis.clone(),
        can_undo: !doc.undo.is_empty(),
        can_redo: !doc.redo.is_empty(),
    }
}

fn info_2d(id: usize, doc: &Document2D) -> Document2DInfo {
    Document2DInfo {
        id,
        name: name(&doc.path),
        path: doc.path.clone(),
        nuclei: doc.nuclei.clone(),
        solvent: doc.solvent.clone(),
        frequencies_mhz: doc.frequencies_mhz,
        points: [doc.cols, doc.real.len() / doc.cols.max(1)],
        first_ppm: [doc.axes[0].first, doc.axes[1].first],
        delta_ppm: [doc.axes[0].delta, doc.axes[1].delta],
        noise: doc.noise,
        max: doc.max,
        signed: doc.signed,
        peaks: doc.peaks.clone(),
        can_undo: !doc.undo.is_empty(),
        can_redo: !doc.redo.is_empty(),
    }
}

fn with_document_2d(
    state: &Documents,
    id: usize,
    change: impl FnOnce(&mut Document2D),
) -> Result<Document2DInfo, String> {
    let mut docs = state.0.lock().map_err(|e| e.to_string())?;
    match docs.get_mut(id).and_then(Option::as_mut) {
        Some(Slot::Two(doc)) => {
            change(doc);
            Ok(info_2d(id, doc))
        }
        Some(Slot::One(_)) => Err("This is a spectrum, not a 2D experiment".into()),
        None => Err("This document is no longer open".into()),
    }
}

/// The cross peak nearest `[f2, f1]` (ppm), within `radius` points along F2
/// and along F1.
#[tauri::command(async)]
fn nearest_peak_2d(
    id: usize,
    at: [f64; 2],
    radius: [usize; 2],
    state: State<'_, Documents>,
) -> Result<Option<[f64; 2]>, String> {
    let docs = state.0.lock().map_err(|e| e.to_string())?;
    let doc = document_2d(&docs, id)?;
    Ok(analysis::nearest_peak_2d(
        &doc.real, doc.cols, &doc.axes, at, radius,
    ))
}

/// The 2D's own projection along F2 (`dim` 0) or F1 (1), decimated as `trace`
/// decimates a spectrum.
#[tauri::command(async)]
fn projection_2d(
    id: usize,
    dim: usize,
    start: usize,
    end: usize,
    columns: usize,
    state: State<'_, Documents>,
) -> Result<Response, String> {
    let docs = state.0.lock().map_err(|e| e.to_string())?;
    let doc = document_2d(&docs, id)?;
    let values = doc.projections.get(dim).ok_or("A 2D has two dimensions")?;
    Ok(f32_response(&decimate::min_max(
        values,
        start..end,
        columns,
    )))
}

/// Replaces a 2D's cross peaks, as one step to undo.
#[tauri::command(async)]
fn set_peaks_2d(
    id: usize,
    peaks: Vec<CrossPeak>,
    state: State<'_, Documents>,
) -> Result<Document2DInfo, String> {
    with_document_2d(&state, id, |doc| {
        let previous = std::mem::replace(&mut doc.peaks, peaks);
        doc.undo.push(previous);
        doc.redo.clear();
    })
}

#[tauri::command(async)]
fn undo_2d(id: usize, state: State<'_, Documents>) -> Result<Document2DInfo, String> {
    with_document_2d(&state, id, |doc| {
        if let Some(previous) = doc.undo.pop() {
            doc.redo.push(std::mem::replace(&mut doc.peaks, previous));
        }
    })
}

#[tauri::command(async)]
fn redo_2d(id: usize, state: State<'_, Documents>) -> Result<Document2DInfo, String> {
    with_document_2d(&state, id, |doc| {
        if let Some(next) = doc.redo.pop() {
            doc.undo.push(std::mem::replace(&mut doc.peaks, next));
        }
    })
}

/// "sample / expno", the way an experiment is usually named.
fn name(path: &Path) -> String {
    let part = |p: Option<&Path>| {
        p.and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    if part(path.parent()) == "pdata" {
        let experiment = path.parent().and_then(Path::parent).unwrap_or(path);
        return format!("{} / pdata {}", name(experiment), part(Some(path)));
    }
    format!("{} / {}", part(path.parent()), part(Some(path)))
}

/// macOS puts the standard Edit menu on every app, and its Undo takes Cmd+Z
/// before the web view sees it, so the app's own undo never runs. This menu
/// keeps the usual items but sends Undo, Redo and Open to the front end.
#[cfg(target_os = "macos")]
fn menu(handle: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{MenuBuilder, MenuItem, SubmenuBuilder};

    let open = MenuItem::with_id(handle, "open", "Open…", true, Some("CmdOrCtrl+O"))?;
    let undo = MenuItem::with_id(handle, "undo", "Undo", true, Some("CmdOrCtrl+Z"))?;
    let redo = MenuItem::with_id(handle, "redo", "Redo", true, Some("Shift+CmdOrCtrl+Z"))?;
    let application = SubmenuBuilder::new(handle, "openNMR")
        .about(None)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;
    let file = SubmenuBuilder::new(handle, "File")
        .item(&open)
        .separator()
        .close_window()
        .build()?;
    let edit = SubmenuBuilder::new(handle, "Edit")
        .item(&undo)
        .item(&redo)
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let view = SubmenuBuilder::new(handle, "View").fullscreen().build()?;
    let window = SubmenuBuilder::new(handle, "Window")
        .minimize()
        .separator()
        .close_window()
        .build()?;
    MenuBuilder::new(handle)
        .items(&[&application, &file, &edit, &view, &window])
        .build()
}

fn main() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Documents::default())
        .invoke_handler(tauri::generate_handler![
            open,
            project_layout,
            system_fonts,
            save_project,
            save_pdf,
            read_project,
            keep_session,
            last_session,
            set_operations,
            set_analysis,
            undo,
            redo,
            trace,
            phase_trace,
            largest_peak,
            auto_phase,
            baseline_trace,
            hand_set_baseline,
            propose_baseline_points,
            find_peaks,
            auto_peaks,
            fit_peaks,
            signal_regions,
            nearest_peak,
            integral_curve,
            set_state,
            read_bytes,
            clipboard_structure,
            read_molecule,
            save_file,
            close,
            metadata,
            fit_multiplet,
            contours,
            nearest_peak_2d,
            projection_2d,
            set_peaks_2d,
            undo_2d,
            redo_2d
        ]);
    #[cfg(target_os = "macos")]
    let builder = builder.menu(menu).on_menu_event(|app, event| {
        use tauri::Emitter;
        let _ = app.emit("menu", event.id().0.as_str());
    });
    builder
        .build(tauri::generate_context!())
        .expect("failed to start openNMR")
        .run(|app, event| {
            // Closed as intended: there is no session to recover.
            if let tauri::RunEvent::Exit = event
                && let Ok(file) = recovery_file(app)
            {
                let _ = std::fs::remove_file(file);
            }
        });
}

#[cfg(test)]
mod tests {
    fn fitted_document() -> Document {
        let raw = Spectrum {
            data: (0..2048)
                .map(|i| {
                    let line =
                        |at: f64, height: f64| height / (1.0 + ((i as f64 - at) / 8.0).powi(2));
                    nmr_core::Complex64::new(line(600.0, 1.0) + line(1400.0, 0.5), 0.0)
                })
                .collect(),
            meta: nmr_core::Metadata {
                nucleus: "1H".into(),
                sw_hz: 819.2,
                obs_mhz: 400.0,
                carrier_hz: 0.0,
                group_delay: 0.0,
                solvent: None,
                phase: None,
                conjugate: false,
                signed: false,
                size: None,
                line_broadening: None,
                delay_at_ft: false,
            },
            domain: nmr_core::Domain::Frequency,
        };
        let real = real_part(&raw);
        let axis = raw.ppm_axis();
        let fit = deconvolution::deconvolve(&real, &axis, 400.0, 0.005, false, None);
        assert!(
            fit.regions
                .iter()
                .all(|r| r.status == deconvolution::FitStatus::Converged)
        );
        Document {
            path: PathBuf::from("synthetic"),
            raw: raw.clone(),
            state: DocumentState {
                operations: vec![],
                analysis: Analysis {
                    integrals: vec![
                        [axis.ppm(550.0), axis.ppm(650.0)],
                        [axis.ppm(1390.0), axis.ppm(1410.0)],
                    ],
                    peaks: fit.peaks.iter().map(|p| p.ppm).collect(),
                    peak_fit: Some(fit),
                    integration_method: IntegrationMethod::Fitted,
                    ..Analysis::default()
                },
            },
            undo: vec![],
            redo: vec![],
            revision: 0,
            phase_base: raw.clone(),
            spectrum: raw,
            real,
            noise: 0.001,
        }
    }

    #[test]
    fn legacy_analysis_uses_standard_peaks_and_sum() {
        use serde::de::value::{Error, MapDeserializer, SeqDeserializer};
        let fields = ["integrals", "peaks"].into_iter().map(|key| {
            (
                key,
                SeqDeserializer::<_, Error>::new(std::iter::empty::<f64>()),
            )
        });
        let legacy = Analysis::deserialize(MapDeserializer::<_, Error>::new(fields)).unwrap();
        assert!(legacy.peak_method == PeakMethod::Standard);
        assert!(legacy.integration_method == IntegrationMethod::Sum);
        assert!(legacy.peak_fit.is_none());
    }

    #[test]
    fn legacy_fit_regions_without_a_reason_remain_readable() {
        let region: deconvolution::FitRegion =
            serde_json::from_str(r#"{"from":1.0,"to":0.0,"status":"unresolved","rms":null}"#)
                .unwrap();
        assert_eq!(region.reason, None);
        assert_eq!(region.status, deconvolution::FitStatus::Unresolved);
    }

    #[test]
    fn fitted_normalization_uses_components_and_never_falls_back() {
        let mut doc = fitted_document();
        let axis = doc.spectrum.ppm_axis();
        let fitted = integral_values(&doc, &axis);
        assert_eq!(fitted[0], Some(1.0));
        assert!((fitted[1].unwrap() - 0.5).abs() < 0.001);
        doc.state.analysis.integration_method = IntegrationMethod::Sum;
        assert!(integral_values(&doc, &axis)[1].unwrap() < 0.4);
        doc.state.analysis.integration_method = IntegrationMethod::Fitted;
        doc.state.analysis.peak_fit.as_mut().unwrap().regions[1].status =
            deconvolution::FitStatus::Failed;
        assert_eq!(integral_values(&doc, &axis), vec![Some(1.0), None]);
        doc.state.analysis.reference = Some(IntegralReference {
            index: 1,
            value: 3.0,
        });
        assert_eq!(integral_values(&doc, &axis), vec![None, None]);
        doc.state.analysis.peak_fit = None;
        assert_eq!(integral_values(&doc, &axis), vec![None, None]);
    }

    #[test]
    fn reprocessing_invalidates_models_and_undo_restores_them() {
        let mut doc = fitted_document();
        let saved = doc.state.clone();
        doc.change(DocumentState {
            operations: vec![Operation::Reference {
                from_ppm: 0.0,
                to_ppm: 1.0,
            }],
            analysis: saved.analysis.clone(),
        })
        .unwrap();
        assert!(doc.state.analysis.peak_fit.is_none());
        let previous = doc.undo.pop().unwrap();
        let changed = doc.replace(previous).unwrap();
        assert_eq!(doc.state.analysis.peak_fit, saved.analysis.peak_fit);
        doc.replace(changed).unwrap();
        assert!(doc.state.analysis.peak_fit.is_none());
        let mut corrupt = saved.clone();
        corrupt.analysis.peak_fit.as_mut().unwrap().peaks[0].area *= 2.0;
        doc.replace(corrupt).unwrap();
        assert!(doc.state.analysis.peak_fit.is_none());
    }

    #[test]
    fn system_fonts_are_listed_once_each_in_order() {
        let fonts = super::system_fonts();
        assert!(
            fonts
                .windows(2)
                .all(|w| w[0].to_lowercase() < w[1].to_lowercase())
        );
        assert!(fonts.iter().all(|name| !name.starts_with('.')));
    }

    #[test]
    fn a_data_url_reads_back_what_was_written() {
        for bytes in [
            vec![],
            vec![0u8],
            vec![1, 2],
            vec![255, 0, 7],
            (0..=255).collect(),
        ] {
            let url = format!("data:image/png;base64,{}", super::base64(&bytes));
            assert_eq!(super::from_data_url(&url), Some(bytes));
        }
        assert_eq!(super::from_data_url("data:image/png;base64,***"), None);
    }

    #[test]
    fn one_nucleus_is_the_smallest_real_signal() {
        // A sliver of noise among real signals does not become the unit.
        let areas = vec![41.0, 24.0, 1.0, 46.0, 90.0, 21.0, 2.4];
        assert_eq!(one_nucleus(&areas), Some(21.0));
        // With nothing to compare against, the only area is the unit.
        assert_eq!(one_nucleus(&[7.0]), Some(7.0));
        assert_eq!(one_nucleus(&[]), None);
        // Areas all of a size keep the smallest of them.
        assert_eq!(one_nucleus(&[10.0, 11.0, 9.0]), Some(9.0));
    }

    use super::*;

    #[test]
    fn only_protons_are_held_to_a_tight_coupling() {
        // The guard that keeps two signals caught in one range from being read as
        // one split signal: measured on the dataset in testdata, water and
        // methanol together came back as a doublet with a 35.7 Hz coupling, which
        // is not a proton coupling. Everything that is not 1H couples far harder,
        // so only 1H is held to the tight bound - and the test is on the name
        // exactly, so a nucleus that is not spelled 1H gets the loose one.
        assert_eq!(max_coupling("1H"), 20.0);
        for heavier in ["13C", "19F", "31P", "15N", ""] {
            assert_eq!(max_coupling(heavier), 300.0, "{heavier}");
        }
    }

    #[test]
    fn names_show_experiment_and_pdata() {
        assert_eq!(name(Path::new("/data/L1D_Cl_00/1")), "L1D_Cl_00 / 1");
        assert_eq!(
            name(Path::new("/data/L1D_Cl_00/1/pdata/1")),
            "L1D_Cl_00 / 1 / pdata 1"
        );
    }

    #[test]
    fn decodes_encode_uri_component() {
        assert_eq!(
            percent_decode("%2FUsers%2Fj%C3%B6rg%2Fspectrum%20a.svg").as_deref(),
            Some("/Users/jörg/spectrum a.svg")
        );
        assert_eq!(percent_decode("bad%2"), None);
        assert_eq!(percent_decode("%FF"), None);
    }
}
