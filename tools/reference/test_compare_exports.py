"""Synthetic fixtures only: these tests make no reference-parity claim."""

import copy
import json
import math
from pathlib import Path
import statistics
import struct
import subprocess
import sys
import tempfile
import unittest

from compare_exports import area_noise, compare_manifest, load_spectrum, match_peaks


class ComparisonTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.axis = [10 - i * 0.01 for i in range(512)]
        self.values = [0.002 * math.sin(i * 1.71) + 0.001 * math.cos(i * 0.39)
                       + (5 if 150 <= i < 170 else 10 if 300 <= i < 320 else 0)
                       for i in range(512)]
        self.case = {
            "name": "synthetic",
            "reference": {"path": "reference.csv", "format": "csv", "skip_rows": 1},
            "opennmr": {"path": "candidate.bin", "format": "f64le",
                        "first_ppm": 10, "delta_ppm": -0.01},
            "quiet": [[10, 8.8], [6.5, 4.9]],
            "normalization": [8.5, 8.3],
            "integrals": [{"name": "signal", "bounds": [7, 6.8]}],
            "noise": {"reference": 0.002, "opennmr": 0.002},
        }
        self.write(self.values, self.values)

    def write(self, reference, candidate):
        (self.root / "reference.csv").write_text("ppm,intensity\n" + "".join(
            f"{x:.12f},{y:.15g}\n" for x, y in zip(self.axis, reference)))
        (self.root / "candidate.bin").write_bytes(b"".join(struct.pack("<d", y) for y in candidate))

    def run_case(self, case=None):
        path = self.root / "manifest.json"
        path.write_text(json.dumps({"cases": [case or self.case]}))
        return compare_manifest(path)

    def test_common_normalization_removes_only_overall_scale(self):
        self.write(self.values, [2 * v for v in self.values])
        self.case["noise"]["opennmr"] *= 2
        result = self.run_case()
        self.assertTrue(result["pass"])
        self.assertAlmostEqual(result["cases"][0]["integrals"][0]["reference"], 2, delta=0.0001)
        self.assertAlmostEqual(result["cases"][0]["integrals"][0]["error"], 0, places=12)

    def test_signal_area_error_fails_two_percent_limit(self):
        values = [v * 1.05 if 300 <= i < 320 else v for i, v in enumerate(self.values)]
        self.write(self.values, values)
        result = self.run_case()
        integral = result["cases"][0]["integrals"][0]
        self.assertFalse(result["pass"])
        self.assertEqual(integral["criterion"], "2_percent")
        self.assertAlmostEqual(integral["relative_error"], 0.05, places=6)

    def test_offset_is_measured_in_noise_units(self):
        self.write(self.values, [v + 0.02 for v in self.values])
        result = self.run_case()
        quiet = result["cases"][0]["quiet"]["opennmr"][0]
        self.assertFalse(result["pass"])
        self.assertGreater(quiet["offset_sigma"], 9)

    def test_identical_curved_baselines_fail_individually(self):
        values = [v + 0.1 * ((i - 256) / 256) ** 2 for i, v in enumerate(self.values)]
        self.write(values, values)
        result = self.run_case()
        self.assertFalse(result["pass"])
        self.assertTrue(any(not window["pass"] for window in result["cases"][0]["quiet"]["reference"]))
        self.assertAlmostEqual(result["cases"][0]["integrals"][0]["error"], 0, places=12)

    def test_noise_integrals_keep_correlations_and_remove_offset(self):
        window = [100 + offset for offset in (-3, -1, 1, 3) for _ in range(8)]
        sigma, method, count = area_noise([window], 8, 0.1, 1)
        self.assertEqual((method, count), ("quiet_blocks", 4))
        self.assertAlmostEqual(sigma, statistics.stdev([-2.4, -0.8, 0.8, 2.4]))
        fallback, method, _ = area_noise([window[:8]], 8, 0.1, 2)
        self.assertEqual(method, "conservative_correlated_bound")
        self.assertAlmostEqual(fallback, 1.6)

    def test_weak_integral_uses_empirical_absolute_noise(self):
        values = [v - 10 + 0.0001 if 300 <= i < 320 else v for i, v in enumerate(self.values)]
        self.write(values, values)
        result = self.run_case()
        integral = result["cases"][0]["integrals"][0]
        self.assertEqual(integral["criterion"], "noise_absolute")
        self.assertEqual(integral["noise_method"], ["quiet_blocks", "quiet_blocks"])
        self.assertTrue(integral["pass"])

    def test_rejects_empty_missing_nonfinite_and_mismatched_references(self):
        path = self.root / "empty.json"
        path.write_text('{"cases": []}')
        with self.assertRaisesRegex(ValueError, "nonempty"):
            compare_manifest(path)
        missing = copy.deepcopy(self.case)
        missing["reference"]["path"] = "missing.csv"
        with self.assertRaises(FileNotFoundError):
            self.run_case(missing)
        bad_axis = copy.deepcopy(self.case)
        bad_axis["opennmr"]["first_ppm"] += 0.001
        with self.assertRaisesRegex(ValueError, "axes differ"):
            self.run_case(bad_axis)
        values = self.values.copy()
        values[5] = math.nan
        self.write(self.values, values)
        with self.assertRaisesRegex(ValueError, "finite samples"):
            self.run_case()

    def test_normalization_is_not_itself_a_test(self):
        self.case["integrals"][0]["bounds"] = self.case["normalization"]
        with self.assertRaisesRegex(ValueError, "acceptance integral"):
            self.run_case()

    def test_peak_matching_is_one_to_one(self):
        result = match_peaks({"reference": [1, 2, 3], "opennmr": [1.01, 1.02, 3],
                              "tolerance_ppm": 0.03})
        self.assertEqual(result["missed"], [2])
        self.assertEqual(result["extra"], [1.02])
        self.assertFalse(result["pass"])

    def test_reference_json_uses_full_precision_axis_and_checks_point_count(self):
        document = {"spectra": [{"data": {
            "dimensional_parameters": [{"points": 512, "spectrometer_frequency": 100,
                                        "spectral_width": 512, "lowest_frequency": 488}],
            "data": {"1r": {"array": self.values}},
        }}]}
        path = self.root / "reference.json"
        path.write_text(json.dumps(document))
        axis, values = load_spectrum({"path": path.name, "format": "reference-json"}, self.root)
        self.assertEqual(values, self.values)
        self.assertAlmostEqual(axis[0], 10.0)
        self.assertAlmostEqual(axis[-1], 4.89)
        document["spectra"][0]["data"]["dimensional_parameters"][0]["points"] = 513
        path.write_text(json.dumps(document))
        with self.assertRaisesRegex(ValueError, "point count"):
            load_spectrum({"path": path.name, "format": "reference-json"}, self.root)

    def test_cli_reports_success_mismatch_and_invalid_input(self):
        self.run_case()
        command = [sys.executable, str(Path(__file__).with_name("compare_exports.py")),
                   str(self.root / "manifest.json")]
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(json.loads(result.stdout)["pass"])
        self.write(self.values, [v + 0.02 for v in self.values])
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertFalse(json.loads(result.stdout)["pass"])
        (self.root / "manifest.json").write_text('{"cases": []}')
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn("nonempty", result.stderr)

    def test_ascii_and_ascending_axes(self):
        self.axis.reverse()
        self.values.reverse()
        self.write(self.values, self.values)
        (self.root / "candidate.txt").write_text("# ppm intensity\n" + "".join(
            f"{x:.12f} {y:.15g}\n" for x, y in zip(self.axis, self.values)))
        self.case["opennmr"] = {"path": "candidate.txt", "format": "ascii"}
        self.assertTrue(self.run_case()["pass"])


if __name__ == "__main__":
    unittest.main()
