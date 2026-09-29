import argparse
import json
import os
import platform
import statistics
import subprocess
import sys
import time
from pathlib import Path

from fixtures import FIXTURE_SHA256, ensure_fixture

ROOT = Path(__file__).resolve().parents[1]
PROFILE_LABELS = {
    "1b": "PDF/A-1b",
    "2b": "PDF/A-2b",
    "ua1": "PDF/UA-1",
}
EXPECTED_EXIT_CODES = {0, 2}
PROCESS_TIMEOUT_SECONDS = 60


def parse_args():
    parser = argparse.ArgumentParser(
        description="Compare page runtime against a baseline binary on pinned real-world PDFs."
    )
    parser.add_argument("--baseline-bin", type=Path, required=True)
    parser.add_argument("--candidate-bin", type=Path, required=True)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=ROOT / "bench" / "performance-regression.json",
    )
    parser.add_argument("--json-output", type=Path, required=True)
    parser.add_argument("--summary-output", type=Path)
    return parser.parse_args()


def validate_inputs(manifest, baseline_bin, candidate_bin):
    for binary in (baseline_bin, candidate_bin):
        if not binary.is_file():
            raise FileNotFoundError(f"validator binary does not exist: {binary}")
        if not binary.stat().st_mode & 0o111:
            raise PermissionError(f"validator binary is not executable: {binary}")

    for document in manifest["documents"]:
        filename = Path(document["path"]).name
        path = ensure_fixture(filename)
        manifest_path = (ROOT / document["path"]).resolve()
        if path.resolve() != manifest_path:
            raise ValueError(f"performance PDF must be stored under bench/: {path}")
        document["sha256"] = FIXTURE_SHA256[filename]


def run_page(binary, pdf, profile):
    command = [
        str(binary),
        str(pdf),
        "--profile",
        profile,
        "--max-reference-depth",
        "512",
        "--format",
        "details",
    ]
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            command,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
            timeout=PROCESS_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired:
        return {
            "seconds": time.perf_counter() - started,
            "exit_code": None,
            "error": f"timed out after {PROCESS_TIMEOUT_SECONDS} seconds",
        }
    except OSError as error:
        return {
            "seconds": time.perf_counter() - started,
            "exit_code": None,
            "error": str(error),
        }

    return {
        "seconds": time.perf_counter() - started,
        "exit_code": completed.returncode,
        "error": None,
    }


def run_workload(document, profile, baseline_bin, candidate_bin, warmups, runs):
    pdf = ROOT / document["path"]
    base_samples = []
    candidate_samples = []
    paired_ratios = []
    errors = []

    print(f"Benchmarking {pdf.name} with {PROFILE_LABELS.get(profile, profile)}")

    warmup_exit_codes = {"baseline": [], "candidate": []}
    for warmup_number in range(warmups):
        order = (
            (("baseline", baseline_bin), ("candidate", candidate_bin))
            if warmup_number % 2 == 0
            else (("candidate", candidate_bin), ("baseline", baseline_bin))
        )
        for binary_name, binary in order:
            sample = run_page(binary, pdf, profile)
            if sample["error"]:
                errors.append(f"{binary_name} warmup: {sample['error']}")
            elif sample["exit_code"] not in EXPECTED_EXIT_CODES:
                errors.append(f"{binary_name} warmup exited with {sample['exit_code']}")
            else:
                warmup_exit_codes[binary_name].append(sample["exit_code"])

    if (
        not errors
        and warmup_exit_codes["baseline"]
        and warmup_exit_codes["candidate"]
        and warmup_exit_codes["baseline"] != warmup_exit_codes["candidate"]
    ):
        errors.append("baseline and candidate warmup exit codes differ")

    if errors:
        return workload_result(
            document, profile, base_samples, candidate_samples, paired_ratios, errors
        )

    for run_number in range(runs):
        order = (
            (("baseline", baseline_bin), ("candidate", candidate_bin))
            if run_number % 2 == 0
            else (("candidate", candidate_bin), ("baseline", baseline_bin))
        )
        pair = {}
        for binary_name, binary in order:
            sample = run_page(binary, pdf, profile)
            pair[binary_name] = sample
            if sample["error"]:
                errors.append(f"run {run_number + 1} {binary_name}: {sample['error']}")
            elif sample["exit_code"] not in EXPECTED_EXIT_CODES:
                errors.append(
                    f"run {run_number + 1} {binary_name} exited with {sample['exit_code']}"
                )

        if errors:
            break

        baseline_sample = pair["baseline"]
        candidate_sample = pair["candidate"]
        if baseline_sample["exit_code"] != candidate_sample["exit_code"]:
            errors.append(
                f"run {run_number + 1} exit codes differ: "
                f"baseline {baseline_sample['exit_code']}, "
                f"candidate {candidate_sample['exit_code']}"
            )
            break

        base_samples.append(baseline_sample["seconds"])
        candidate_samples.append(candidate_sample["seconds"])
        paired_ratios.append(
            candidate_sample["seconds"] / max(baseline_sample["seconds"], 1e-9)
        )
        print(
            f"  run {run_number + 1}/{runs}: "
            f"baseline {baseline_sample['seconds']:.3f}s, "
            f"candidate {candidate_sample['seconds']:.3f}s"
        )

    return workload_result(
        document, profile, base_samples, candidate_samples, paired_ratios, errors
    )


def workload_result(document, profile, base_samples, candidate_samples, ratios, errors):
    return {
        "document": document["path"],
        "sha256": document["sha256"],
        "profile": profile,
        "baseline_samples_seconds": base_samples,
        "candidate_samples_seconds": candidate_samples,
        "paired_ratios": ratios,
        "baseline_median_seconds": statistics.median(base_samples)
        if base_samples
        else None,
        "candidate_median_seconds": statistics.median(candidate_samples)
        if candidate_samples
        else None,
        "paired_median_ratio": statistics.median(ratios) if ratios else None,
        "errors": errors,
    }


def render_summary(report):
    status = "FAIL" if report["errors"] or report["regressions"] else "PASS"
    lines = [
        f"## Performance regression check: {status}",
        "",
        (
            f"Compared `{report['baseline_revision']}` with "
            f"`{report['candidate_revision']}` on `{report['runner']}`."
        ),
        "",
        f"Threshold: more than {report['max_slowdown_percent']:.0f}% median paired slowdown.",
        "",
        "| PDF | Profile | Base median | Candidate median | Paired slowdown | Result |",
        "| --- | --- | ---: | ---: | ---: | --- |",
    ]

    for result in report["results"]:
        base = result["baseline_median_seconds"]
        candidate = result["candidate_median_seconds"]
        ratio = result["paired_median_ratio"]
        if result["errors"] or ratio is None:
            slowdown = "—"
            result_status = "ERROR"
        else:
            slowdown = f"{(ratio - 1) * 100:+.1f}%"
            result_status = "REGRESSION" if result["regression"] else "ok"
        base_text = f"{base * 1000:.1f} ms" if base is not None else "—"
        candidate_text = f"{candidate * 1000:.1f} ms" if candidate is not None else "—"
        lines.append(
            f"| `{Path(result['document']).name}` | "
            f"{PROFILE_LABELS.get(result['profile'], result['profile'])} | "
            f"{base_text} | {candidate_text} | {slowdown} | {result_status} |"
        )

    if report["errors"]:
        lines.extend(["", "### Errors", ""])
        lines.extend(f"- {error}" for error in report["errors"])

    if report["regressions"]:
        lines.extend(["", "### Regressions", ""])
        lines.extend(f"- {regression}" for regression in report["regressions"])

    return "\n".join(lines) + "\n"


def write_report(report, json_output, summary_output):
    json_output.parent.mkdir(parents=True, exist_ok=True)
    json_output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if summary_output:
        summary_output.parent.mkdir(parents=True, exist_ok=True)
        with summary_output.open("a", encoding="utf-8") as summary:
            summary.write(render_summary(report))


def main():
    args = parse_args()
    try:
        manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
        validate_inputs(manifest, args.baseline_bin, args.candidate_bin)

        results = []
        for document in manifest["documents"]:
            for profile in manifest["profiles"]:
                results.append(
                    run_workload(
                        document,
                        profile,
                        args.baseline_bin,
                        args.candidate_bin,
                        manifest["warmup_runs"],
                        manifest["measured_runs"],
                    )
                )

        max_slowdown = manifest["max_slowdown_percent"]
        regressions = []
        for result in results:
            ratio = result["paired_median_ratio"]
            result["regression"] = (
                ratio is not None and (ratio - 1) * 100 > max_slowdown
            )
            if result["regression"]:
                regressions.append(
                    f"{Path(result['document']).name} / "
                    f"{PROFILE_LABELS.get(result['profile'], result['profile'])}: "
                    f"{(ratio - 1) * 100:.1f}% slower"
                )

        errors = [
            f"{Path(result['document']).name} / {result['profile']}: {error}"
            for result in results
            for error in result["errors"]
        ]
        report = {
            "schema_version": 1,
            "baseline_revision": "unknown",
            "candidate_revision": "unknown",
            "runner": platform.platform(),
            "warmup_runs": manifest["warmup_runs"],
            "measured_runs": manifest["measured_runs"],
            "max_slowdown_percent": max_slowdown,
            "results": results,
            "errors": errors,
            "regressions": regressions,
        }
        report["baseline_revision"] = os.environ.get(
            "PERFORMANCE_BASELINE_SHA", "unknown"
        )
        report["candidate_revision"] = os.environ.get("GITHUB_SHA", "local")
        write_report(report, args.json_output, args.summary_output)
        print(render_summary(report))
        return 1 if errors or regressions else 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        report = {
            "schema_version": 1,
            "baseline_revision": os.environ.get("PERFORMANCE_BASELINE_SHA", "unknown"),
            "candidate_revision": os.environ.get("GITHUB_SHA", "local"),
            "runner": platform.platform(),
            "results": [],
            "errors": [str(error)],
            "regressions": [],
            "max_slowdown_percent": 0,
        }
        write_report(report, args.json_output, args.summary_output)
        print(f"performance regression check failed: {error}", file=sys.stderr)
        print(render_summary(report))
        return 1


if __name__ == "__main__":
    sys.exit(main())
