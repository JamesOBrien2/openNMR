"""Process every Bruker 1D experiment in testdata/bruker with nmrglue, using the
same steps as nmr_core::process::default_1d, and write reference output to
testdata/golden/<experiment path>/:

- default_1d.c128: spectrum as little-endian float64 real, imag pairs
- airpls.c128: default_1d, then airPLS baseline with lambda 1e7
- whittaker.f64: the single smoothing solve airPLS iterates on (all weights 1)
- polynomial.c128: default_1d, then polynomial baseline of order 5
- axis.f64: ppm of the first point, ppm step per point
- integrals.f64: nmrglue integrate() of default_1d over INTEGRAL_LIMITS
- peaks.f64: point indices from nmrglue peakpick.pick(algorithm="thres", msep=3)
  above 5 % of the maximum
- pdata/<N>/processed.c128 and processed_axis.f64: nmrglue read_pdata of the
  stored processed spectrum, scaled by NC_proc

The baseline references are independent ports of the published algorithms:
airPLS (Zhang, Chen & Liang, Analyst 135 (2010) 1138) as a scipy sparse solve,
and modpoly with a numpy QR projection.
"""

from pathlib import Path

import nmrglue as ng
import numpy as np
from scipy import sparse
from scipy.sparse.linalg import spsolve

root = Path(__file__).resolve().parents[2] / "testdata"
INTEGRAL_LIMITS = [(1.35, 1.10), (3.85, 3.55), (2.70, 2.50)]


def airpls(x, lam):
    n = x.size
    lam = lam * (n / 1e4) ** 4
    d = sparse.diags([1.0, -2.0, 1.0], [0, 1, 2], shape=(n - 2, n))
    penalty = lam * (d.T @ d)
    edges = np.r_[0 : int(np.ceil(n * 0.1)), int(np.floor(n - n * 0.1)) - 1 : n]
    w = np.ones(n)
    total = np.abs(x).sum()
    for j in range(1, 21):
        z = spsolve(sparse.csc_matrix(sparse.diags(w) + penalty), w * x)
        r = x - z
        dssn = abs(r[r < 0].sum())
        if dssn < 0.001 * total:
            break
        w[r >= 0] = 0
        w[edges] = 0.05
        w[r < 0] = np.exp(j * np.abs(r[r < 0]) / dssn)
    return z


def polynomial(y, order):
    x = np.linspace(-1, 1, y.size)
    q, _ = np.linalg.qr(np.polynomial.legendre.legvander(x, min(order, y.size - 1)))
    work = y.copy()
    fit = q @ (q.T @ work)
    for _ in range(200):
        work = np.minimum(work, fit)
        nxt = q @ (q.T @ work)
        change = np.linalg.norm(nxt - fit)
        fit = nxt
        if change <= 1e-9 * np.linalg.norm(nxt):
            break
    return fit


def save(path, spec):
    np.column_stack([spec.real, spec.imag]).astype("<f8").tofile(path)


for acqus in sorted((root / "bruker").rglob("acqus")):
    exp = acqus.parent
    if not (exp / "fid").is_file() or (exp / "acqu2s").exists():
        continue
    dic, data = ng.bruker.read(str(exp))
    a = dic["acqus"]
    # nmrglue keeps block padding; nmr-core truncates to TD.
    data = data[: a["TD"] // 2]
    n = data.size

    # The digital receiver (DIGMOD 1 or 3) stores the opposite quadrature sign,
    # as nmr_core's Metadata::conjugate describes.
    if a.get("DIGMOD", 0) != 0:
        data = np.conj(data)

    grpdly = a.get("GRPDLY", 0)
    dspfvs = a.get("DSPFVS", 0)
    decim = int(a.get("DECIM", 0))
    has_delay = grpdly > 0 or (dspfvs and dspfvs < 14)
    if has_delay:
        data = ng.bruker.rm_dig_filter(data, decim, dspfvs, grpdly, truncate_grpdly=False)

    lb_hz = 0.3 if a["NUC1"] == "1H" else 1.0
    data = ng.proc_base.em(data, lb_hz / a["SW_h"])
    spec = ng.proc_base.fft(data)
    # default_1d applies the recorded phase, when the dataset carries one.
    procs_file = exp / "pdata" / "1" / "procs"
    if procs_file.is_file():
        stored = ng.bruker.read_jcamp(str(procs_file))
        phc0, phc1 = stored.get("PHC0", 0.0), stored.get("PHC1", 0.0)
        if phc0 or phc1:
            spec = ng.proc_base.ps(spec, p0=phc0, p1=phc1)

    uc = ng.fileiobase.uc_from_udic(ng.bruker.guess_udic(dic, spec))
    out = root / "golden" / exp.relative_to(root / "bruker")
    out.mkdir(parents=True, exist_ok=True)
    save(out / "default_1d.c128", spec)
    save(out / "airpls.c128", spec - airpls(spec.real, 1e7))
    lam = 1e7 * (spec.size / 1e4) ** 4
    dm = sparse.diags([1.0, -2.0, 1.0], [0, 1, 2], shape=(spec.size - 2, spec.size))
    smooth = spsolve(sparse.csc_matrix(sparse.eye(spec.size) + lam * (dm.T @ dm)), spec.real)
    smooth.astype("<f8").tofile(out / "whittaker.f64")
    save(out / "polynomial.c128", spec - polynomial(spec.real, 5))
    real = spec.real
    values = ng.analysis.integration.integrate(real, uc, INTEGRAL_LIMITS)
    np.asarray(values, dtype="<f8").tofile(out / "integrals.f64")
    found = ng.analysis.peakpick.pick(
        real, 0.05 * real.max(), algorithm="thres", msep=3, est_params=False, cluster=False, table=False
    )
    np.array(sorted(int(p[0]) for p in found), dtype="<f8").tofile(out / "peaks.f64")
    np.array([uc.ppm(0), uc.ppm(1) - uc.ppm(0)], dtype="<f8").tofile(out / "axis.f64")
    print(f"{exp.relative_to(root)}: {spec.size} points, peak at {uc.ppm(np.abs(spec).argmax()):.3f} ppm")

    for one_r in sorted(exp.glob("pdata/*/1r")):
        pdir = one_r.parent
        pdic, pdata = ng.bruker.read_pdata(str(pdir), scale_data=True)
        pout = out / pdir.relative_to(exp)
        pout.mkdir(parents=True, exist_ok=True)
        # read_pdata returns 1r alone; nmr-core reads the imaginary part as well.
        imag = np.zeros_like(np.asarray(pdata, dtype=float))
        if (pdir / "1i").is_file():
            _, imag = ng.bruker.read_pdata(str(pdir), bin_files=["1i"], scale_data=True)
        save(pout / "processed.c128", np.asarray(pdata) + 1j * np.asarray(imag))
        p = pdic["procs"]
        puc = ng.fileiobase.unit_conversion(p["SI"], False, p["SW_p"], p["SF"], p["OFFSET"] * p["SF"] - p["SW_p"] / 2)
        np.array([puc.ppm(0), puc.ppm(1) - puc.ppm(0)], dtype="<f8").tofile(pout / "processed_axis.f64")
