"""Check the app's processing against the stored processed spectrum.

For every experiment in testdata/bruker with a pdata/N/1r, the FID is processed
the way nmr_core::process::default_1d does, the phase that matches the stored
spectrum best is searched for, and the correlation is printed. A value near 1
means openNMR reads and processes the dataset consistently; a value near 0
means a convention (quadrature sign, digital filter, axis) is wrong.

Usage: tools/reference/.venv/bin/python tools/reference/compare_processed.py
"""

from pathlib import Path

import nmrglue as ng
import numpy as np

root = Path(__file__).resolve().parents[2] / "testdata" / "bruker"


def process(exp):
    dic, data = ng.bruker.read(str(exp))
    a = dic["acqus"]
    data = data[: a["TD"] // 2]
    if a.get("DIGMOD", 0) != 0:
        data = np.conj(data)
    grpdly = a.get("GRPDLY", 0)
    if grpdly > 0:
        data = ng.bruker.rm_dig_filter(
            data, int(a["DECIM"]), a["DSPFVS"], grpdly, truncate_grpdly=False
        )
    lb_hz = 0.3 if a["NUC1"] == "1H" else 1.0
    data = ng.proc_base.em(data, lb_hz / a["SW_h"])
    data = ng.proc_base.zf_size(data, 2 ** int(np.ceil(np.log2(data.size))) * 2)
    return ng.proc_base.fft(data)


def best_phase(spec, reference):
    """Correlation of the best phased real part with `reference`, coarse then fine."""
    k = np.arange(spec.size) / spec.size
    best = (-1.0, 0.0, 0.0)
    for step, span in ((5.0, 180.0), (0.1, 5.0)):
        _, c0, c1 = best
        for p0 in np.arange(c0 - span, c0 + span, step):
            for p1 in np.arange(c1 - span, c1 + span, step):
                real = (spec * np.exp(1j * np.deg2rad(p0 + p1 * k))).real
                c = float(np.corrcoef(real, reference)[0, 1])
                if c > best[0]:
                    best = (c, p0, p1)
    return best


for acqus in sorted(root.rglob("acqus")):
    exp = acqus.parent
    if not (exp / "fid").is_file() or (exp / "acqu2s").exists():
        continue
    for one_r in sorted(exp.glob("pdata/*/1r")):
        _, pdata = ng.bruker.read_pdata(str(one_r.parent), scale_data=True)
        reference = pdata.real if np.iscomplexobj(pdata) else pdata
        spec = process(exp)
        if spec.size != reference.size:
            print(f"{exp.relative_to(root)}: {spec.size} points vs {reference.size} stored")
            continue
        c, p0, p1 = best_phase(spec, reference)
        print(f"{exp.relative_to(root)}: correlation {c:.4f} at p0 {p0:.1f}, p1 {p1:.1f}")
