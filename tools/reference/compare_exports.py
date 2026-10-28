"""Compare fixed reference/openNMR exports on a common ppm grid."""

import argparse
import csv
import json
import math
from pathlib import Path
import statistics as stats
import struct
import sys


def finite(value, name):
    value = float(value)
    if not math.isfinite(value):
        raise ValueError(f"{name} must be finite")
    return value


def positive(value, name):
    value = finite(value, name)
    if value <= 0:
        raise ValueError(f"{name} must be positive")
    return value


def load_spectrum(spec, root):
    path = root / spec["path"]
    if spec["format"] == "reference-json":
        with path.open() as stream:
            document = json.load(stream)
        spectra = document["spectra"]
        if len(spectra) != 1:
            raise ValueError(f"{path}: expected exactly one spectrum")
        data = spectra[0]["data"]
        dimensions = data["dimensional_parameters"]
        if len(dimensions) != 1:
            raise ValueError(f"{path}: expected a 1D spectrum")
        dimension = dimensions[0]
        values = [finite(v, "intensity") for v in data["data"]["1r"]["array"]]
        if dimension["points"] != len(values) or not values:
            raise ValueError(f"{path}: point count does not match intensity data")
        frequency = positive(dimension["spectrometer_frequency"], "spectrometer_frequency")
        width = positive(dimension["spectral_width"], "spectral_width")
        lowest = finite(dimension["lowest_frequency"], "lowest_frequency")
        first = (lowest + width) / frequency
        step = -width / (frequency * len(values))
        axis = [first + i * step for i in range(len(values))]
    elif spec["format"] == "f64le":
        raw = path.read_bytes()
        if len(raw) % 8:
            raise ValueError(f"{path}: incomplete float64")
        values = [item[0] for item in struct.iter_unpack("<d", raw)]
        first = finite(spec["first_ppm"], "first_ppm")
        step = finite(spec["delta_ppm"], "delta_ppm")
        axis = [first + i * step for i in range(len(values))]
    elif spec["format"] in ("csv", "ascii"):
        with path.open() as stream:
            for _ in range(spec.get("skip_rows", 0)):
                next(stream)
            rows = csv.reader(stream) if spec["format"] == "csv" else (
                line.split() for line in stream if line.strip() and not line.lstrip().startswith("#")
            )
            pairs = [tuple(map(float, row)) for row in rows if row]
        if any(len(row) != 2 for row in pairs):
            raise ValueError(f"{path}: expected exactly ppm,intensity columns")
        axis = [row[0] for row in pairs]
        values = [row[1] for row in pairs]
    else:
        raise ValueError(f"{path}: format must be reference-json, csv, ascii or f64le")
    if len(values) < 8 or not all(math.isfinite(v) for v in axis + values):
        raise ValueError(f"{path}: need at least eight finite samples")
    step = axis[1] - axis[0]
    if step == 0 or any(not math.isclose(b - a, step, rel_tol=0, abs_tol=abs(step) * 1e-6)
                        for a, b in zip(axis, axis[1:])):
        raise ValueError(f"{path}: axis must be uniform and strictly monotonic")
    return axis, values


def indices(axis, bounds):
    if len(bounds) != 2:
        raise ValueError("window must have two ppm bounds")
    low, high = sorted(finite(v, "window bound") for v in bounds)
    if low == high or low < min(axis) or high > max(axis):
        raise ValueError(f"window {bounds} is empty or outside the spectrum")
    # Half-open point ranges match nmrglue integration and nmr-core::integrate.
    step = axis[1] - axis[0]
    a, b = sorted(round((v - axis[0]) / step) for v in (low, high))
    if a == b:
        raise ValueError(f"window {bounds} contains no samples")
    return a, b


def area(values, bounds, dx):
    a, b = bounds
    return math.fsum(values[a:b]) * dx


def noise(windows):
    differences = [b - a for values in windows for a, b in zip(values, values[1:])]
    middle = stats.median(differences)
    return stats.median(abs(v - middle) for v in differences) / (0.6744897501960817 * math.sqrt(2))


def baseline_metrics(values, sigma):
    n = len(values)
    x = [2 * i / (n - 1) - 1 for i in range(n)]
    mean_square = stats.mean(v * v for v in x)
    q = [v * v - mean_square for v in x]
    slope = math.fsum(a * b for a, b in zip(x, values)) / math.fsum(a * a for a in x)
    curvature = math.fsum(a * b for a, b in zip(q, values)) / math.fsum(a * a for a in q)
    offset = stats.mean(values) - curvature * mean_square
    positions = [-1, 1]
    if curvature and abs(slope / (2 * curvature)) < 1:
        positions.append(-slope / (2 * curvature))
    fitted = [offset + slope * t + curvature * t * t for t in positions]
    variation = (max(fitted) - min(fitted)) / sigma
    return {"offset_sigma": offset / sigma, "slope_sigma": slope / sigma,
            "curvature_sigma": curvature / sigma, "variation_sigma": variation,
            "pass": abs(offset) <= sigma and variation <= 1}


def area_noise(windows, count, dx, sigma):
    blocks = []
    for values in windows:
        centre = stats.mean(values)
        blocks.extend(math.fsum(v - centre for v in values[i:i + count]) * dx
                      for i in range(0, len(values) - count + 1, count))
    if len(blocks) >= 4:
        return stats.stdev(blocks), "quiet_blocks", len(blocks)
    # shortcut: short quiet windows cannot estimate correlated area noise; use a
    # fully correlated bound until longer fixed quiet windows are supplied.
    return sigma * count * dx, "conservative_correlated_bound", len(blocks)


def match_peaks(spec):
    tolerance = positive(spec["tolerance_ppm"], "peak tolerance")
    expected = [finite(p, "peak ppm") for p in spec["reference"]]
    actual = [finite(p, "peak ppm") for p in spec["opennmr"]]
    # In one dimension the earliest feasible pair gives maximum cardinality.
    expected.sort()
    actual.sort()
    missed, extra = [], []
    i = j = 0
    while i < len(expected) and j < len(actual):
        if abs(expected[i] - actual[j]) <= tolerance:
            i += 1
            j += 1
        elif expected[i] < actual[j]:
            missed.append(expected[i])
            i += 1
        else:
            extra.append(actual[j])
            j += 1
    missed.extend(expected[i:])
    extra.extend(actual[j:])
    return {"missed": missed, "extra": extra, "pass": not missed and not extra}


def compare_case(case, root):
    axis, reference = load_spectrum(case["reference"], root)
    candidate_axis, candidate = load_spectrum(case["opennmr"], root)
    dx = abs(axis[1] - axis[0])
    if len(axis) != len(candidate_axis) or any(abs(a - b) > dx * 1e-6
                                               for a, b in zip(axis, candidate_axis)):
        raise ValueError("axes differ; export an identical grid, do not interpolate")
    quiet = [indices(axis, bounds) for bounds in case["quiet"]]
    if not quiet or any(b - a < 8 for a, b in quiet):
        raise ValueError("need fixed quiet windows of at least eight samples each")
    if any(a < d and c < b for i, (a, b) in enumerate(quiet) for c, d in quiet[i + 1:]):
        raise ValueError("quiet windows must not overlap")
    normalization = indices(axis, case["normalization"])
    if any(a < normalization[1] and normalization[0] < b for a, b in quiet):
        raise ValueError("normalization and quiet windows must not overlap")
    norms = [positive(area(values, normalization, dx), "normalization area")
             for values in (reference, candidate)]
    normalized = [[v / scale for v in values]
                  for values, scale in zip((reference, candidate), norms)]
    quiet_values = [[values[a:b] for a, b in quiet] for values in normalized]
    sigmas = [positive(case["noise"][name], "noise") / scale if "noise" in case
              else positive(noise(windows), "estimated noise")
              for name, scale, windows in zip(("reference", "opennmr"), norms, quiet_values)]
    baseline = {name: [baseline_metrics(values, sigma) for values in windows]
                for name, windows, sigma in zip(("reference", "opennmr"), quiet_values, sigmas)}
    comparisons = []
    independent = False
    for region in case["integrals"]:
        bounds = indices(axis, region["bounds"])
        if bounds == normalization:
            raise ValueError("normalization reference cannot be an acceptance integral")
        if any(a < bounds[1] and bounds[0] < b for a, b in quiet):
            raise ValueError("integration and quiet windows must not overlap")
        independent |= bounds[1] <= normalization[0] or bounds[0] >= normalization[1]
        expected, actual = [area(values, bounds, dx) for values in normalized]
        estimates = [area_noise(windows, bounds[1] - bounds[0], dx, sigma)
                     for windows, sigma in zip(quiet_values, sigmas)]
        absolute_limit = 3 * math.hypot(*(v[0] for v in estimates))
        relative_limit = 0.02 * abs(expected)
        limit = max(absolute_limit, relative_limit)
        error = actual - expected
        comparisons.append({"name": region["name"], "reference": expected, "opennmr": actual,
                            "error": error, "relative_error": error / abs(expected) if expected else None,
                            "allowed_absolute_error": limit,
                            "criterion": "2_percent" if relative_limit >= absolute_limit else "noise_absolute",
                            "noise_method": [v[1] for v in estimates],
                            "quiet_block_counts": [v[2] for v in estimates], "pass": abs(error) <= limit})
    if not comparisons or not independent:
        raise ValueError("need an acceptance integral disjoint from the normalization reference")
    result = {"name": case["name"], "normalization_areas": dict(zip(("reference", "opennmr"), norms)),
              "noise": dict(zip(("reference", "opennmr"), sigmas)),
              "quiet": baseline, "integrals": comparisons}
    if "peaks" in case:
        result["peaks"] = match_peaks(case["peaks"])
    if "timing_ms" in case:
        result["timing_ms"] = {name: positive(case["timing_ms"][name], "timing_ms")
                               for name in ("reference", "opennmr")}
    result["pass"] = (all(item["pass"] for item in comparisons)
                      and all(item["pass"] for windows in baseline.values() for item in windows)
                      and result.get("peaks", {"pass": True})["pass"])
    return result


def compare_manifest(path):
    path = Path(path)
    manifest = json.loads(path.read_text())
    cases = manifest["cases"]
    if not isinstance(cases, list) or not cases:
        raise ValueError("manifest must contain a nonempty cases list")
    names = [case["name"] for case in cases]
    if len(set(names)) != len(names):
        raise ValueError("case names must be unique")
    results = [compare_case(case, path.parent) for case in cases]
    return {"cases": results, "pass": all(case["pass"] for case in results)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    args = parser.parse_args()
    try:
        result = compare_manifest(args.manifest)
    except (ValueError, KeyError, TypeError, OSError, StopIteration, OverflowError, csv.Error, struct.error) as error:
        print(f"Invalid comparison: {error}", file=sys.stderr)
        return 2
    print(json.dumps(result, indent=2, allow_nan=False))
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
