#!/usr/bin/env python3
"""Compare page and veraPDF verdicts and failed ISO rule sets for one PDF.

Run with `uv run --no-project scripts/compare_validators.py FILE --profile 1b`.
Exit codes: 0 = agreement, 2 = disagreement, 1 = execution/report error.
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
PROFILES = ("1b", "1a", "2b", "2a", "2u", "3b", "3a", "3u", "ua1")


def run_validator(
    command: list[str],
    name: str,
    timeout: float,
    output_dir: Path | None,
) -> tuple[int, dict[str, Any]]:
    result = subprocess.run(
        command,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        errors="replace",
        timeout=timeout,
        check=False,
    )
    if output_dir is not None:
        (output_dir / f"{name}.json").write_text(result.stdout, encoding="utf-8")
        (output_dir / f"{name}.stderr.txt").write_text(result.stderr, encoding="utf-8")
    expected_codes = {0, 2} if name == "page" else {0, 1}
    if result.returncode not in expected_codes:
        raise ValueError(
            f"{name} exited with {result.returncode}: {result.stderr.strip()}"
        )
    try:
        report = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise ValueError(f"{name} did not produce valid JSON: {error}") from error
    if not isinstance(report, dict):
        raise TypeError(f"{name} JSON report must be an object")
    return result.returncode, report


def page_result(code: int, report: dict[str, Any]) -> tuple[bool, set[str]]:
    if report.get("error") is not None:
        raise ValueError(f"page could not validate the PDF: {report['error']}")
    compliant = report["is_compliant"]
    if not isinstance(compliant, bool) or code != (0 if compliant else 2):
        raise ValueError("page verdict and exit code are inconsistent")
    return compliant, {failure["rule_id"] for failure in report["failures"]}


def verapdf_result(code: int, report: dict[str, Any]) -> tuple[bool, set[str]]:
    batch = report["report"]
    summary = batch["batchSummary"]
    if any(
        summary.get(key, 0)
        for key in (
            "outOfMemory",
            "veraExceptions",
            "failedEncryptedJobs",
            "failedParsingJobs",
        )
    ):
        raise ValueError("veraPDF reported a parsing, encryption, or execution failure")
    jobs = batch["jobs"]
    if len(jobs) != 1 or len(jobs[0].get("validationResult", [])) != 1:
        raise ValueError("expected exactly one veraPDF job and validation result")
    validation = jobs[0]["validationResult"][0]
    if validation.get("jobEndStatus") != "normal":
        raise ValueError(f"veraPDF job did not finish normally: {validation}")
    compliant = validation["compliant"]
    if not isinstance(compliant, bool) or code != (0 if compliant else 1):
        raise ValueError("veraPDF verdict and exit code are inconsistent")
    rules = {
        f"{rule['specification']}:{rule['clause']}:{rule['testNumber']}"
        for rule in validation["details"]["ruleSummaries"]
        if rule["status"].lower() == "failed"
    }
    return compliant, rules


def main() -> int:
    local_page = ROOT / "target" / "release" / "page"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("file", type=Path, help="PDF to validate")
    parser.add_argument("--profile", required=True, choices=PROFILES)
    parser.add_argument(
        "--page",
        default=str(local_page) if local_page.is_file() else "page",
        help="page executable (default: local release build when present, otherwise PATH)",
    )
    parser.add_argument("--verapdf", default="verapdf", help="veraPDF executable")
    parser.add_argument(
        "--max-reference-depth",
        type=int,
        default=512,
        help="page reference depth (default: 512, matching the benchmark)",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=300,
        help="seconds per validator (default: 300)",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        help="save raw reports, stderr, and comparison.json here",
    )
    parser.add_argument(
        "--json", action="store_true", help="print the comparison as JSON"
    )
    args = parser.parse_args()
    if args.timeout <= 0 or args.max_reference_depth <= 0:
        parser.error("--timeout and --max-reference-depth must be positive")
    if not args.file.is_file():
        parser.error(f"PDF does not exist: {args.file}")
    page_command = [
        args.page,
        str(args.file.resolve()),
        "--profile",
        args.profile,
        "--format",
        "json",
        "--max-reference-depth",
        str(args.max_reference_depth),
    ]
    vera_command = [
        args.verapdf,
        "--loglevel",
        "0",
        "--format",
        "json",
        "--flavour",
        args.profile,
        "--maxfailuresdisplayed",
        "-1",
        str(args.file.resolve()),
    ]
    try:
        if args.output_dir is not None:
            args.output_dir.mkdir(parents=True, exist_ok=True)
        page_code, page_report = run_validator(
            page_command, "page", args.timeout, args.output_dir
        )
        vera_code, vera_report = run_validator(
            vera_command, "verapdf", args.timeout, args.output_dir
        )
        page_compliant, page_rules = page_result(page_code, page_report)
        vera_compliant, vera_rules = verapdf_result(vera_code, vera_report)
        agreement = page_compliant == vera_compliant and page_rules == vera_rules
        comparison = {
            "file": str(args.file),
            "profile": args.profile,
            "agreement": agreement,
            "page_compliant": page_compliant,
            "verapdf_compliant": vera_compliant,
            "page_only": sorted(page_rules - vera_rules),
            "verapdf_only": sorted(vera_rules - page_rules),
            "shared_rules": sorted(page_rules & vera_rules),
            "commands": {"page": page_command, "verapdf": vera_command},
        }
        formatted = json.dumps(comparison, indent=2) + "\n"
        if args.output_dir is not None:
            (args.output_dir / "comparison.json").write_text(
                formatted, encoding="utf-8"
            )
        if args.json:
            print(formatted, end="")
        else:
            print(f"File: {args.file}\nProfile: {args.profile}")
            for name, compliant, rules in (
                ("page", page_compliant, page_rules),
                ("veraPDF", vera_compliant, vera_rules),
            ):
                verdict = "compliant" if compliant else "non-conformant"
                print(f"{name}: {verdict}; {len(rules)} failed rules")
            print(f"Shared failed rules: {len(page_rules & vera_rules)}")
            for label, rules in (
                ("page only", page_rules - vera_rules),
                ("veraPDF only", vera_rules - page_rules),
            ):
                print(f"\n{label}:")
                for rule in sorted(rules):
                    print(f"  {rule}")
                if not rules:
                    print("  (none)")
            print(
                "\nAgreement"
                if agreement
                else "\nDisagreement (verdict or failed rule set)"
            )
        return 0 if agreement else 2
    except (
        OSError,
        subprocess.TimeoutExpired,
        ValueError,
        KeyError,
        TypeError,
        IndexError,
    ) as error:
        print(f"comparison error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
