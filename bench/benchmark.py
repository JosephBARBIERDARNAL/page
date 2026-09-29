import statistics
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

from fixtures import FULL_BENCHMARK_FILENAMES, ensure_fixture

ROOT = Path(__file__).resolve().parents[1]
RUNS = 10
WARMUP_RUNS = 2
OUTPUT_PATH = ROOT / "docs" / "benchmark.md"
PAGE_EXECUTABLE = ROOT / "target" / "release" / "page"
VERAPDF_EXECUTABLE = "verapdf"
PROFILES = ("1b", "2b", "ua1")
EXPECTED_EXIT_CODES = {
    "page": {0, 2},
    "veraPDF": {0, 1},
}


@dataclass(frozen=True)
class RunSample:
    elapsed_seconds: float


@dataclass(frozen=True)
class Summary:
    median_seconds: float

    @classmethod
    def from_samples(cls, samples: list[RunSample]) -> "Summary":
        if not samples:
            raise ValueError("benchmark produced no samples")
        return cls(statistics.median(sample.elapsed_seconds for sample in samples))


@dataclass(frozen=True)
class ProfileBenchmark:
    profile: str
    verapdf: Summary
    page: Summary


@dataclass(frozen=True)
class DocumentBenchmark:
    document: str
    size_bytes: int
    page_count: int
    profiles: list[ProfileBenchmark]


def progress(message: str) -> None:
    print(message, flush=True)


def run_validator(
    validator: str,
    executable: Path | str,
    file: Path,
    profile: str,
) -> RunSample:
    if validator == "page":
        command = [
            str(executable),
            str(file),
            "--profile",
            profile,
            "--max-reference-depth",
            "512",
            "--format",
            "details",
        ]
    else:
        command = [
            str(executable),
            "--loglevel",
            "0",
            "--disableerrormessages",
            "--format",
            "json",
            "--flavour",
            profile,
            str(file),
        ]

    started = time.perf_counter()
    completed = subprocess.run(
        command,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if completed.returncode < 0:
        raise RuntimeError(f"{validator} process terminated by signal")
    if completed.returncode not in EXPECTED_EXIT_CODES[validator]:
        raise RuntimeError(
            f"{validator} exited with {completed.returncode}; expected one of "
            f"{sorted(EXPECTED_EXIT_CODES[validator])}"
        )
    return RunSample(time.perf_counter() - started)


def benchmark_profile(file: Path, profile: str) -> ProfileBenchmark:
    for warmup in range(WARMUP_RUNS):
        progress(f"      warmup {warmup + 1}/{WARMUP_RUNS}")
        run_validator("page", PAGE_EXECUTABLE, file, profile)
        run_validator("veraPDF", VERAPDF_EXECUTABLE, file, profile)

    verapdf_samples = []
    page_samples = []
    for run_number in range(RUNS):
        progress(f"      measured run {run_number + 1}/{RUNS}")
        order = (
            (("veraPDF", VERAPDF_EXECUTABLE), ("page", PAGE_EXECUTABLE))
            if run_number % 2 == 0
            else (("page", PAGE_EXECUTABLE), ("veraPDF", VERAPDF_EXECUTABLE))
        )
        for validator, executable in order:
            sample = run_validator(validator, executable, file, profile)
            if validator == "page":
                page_samples.append(sample)
            else:
                verapdf_samples.append(sample)

    result = ProfileBenchmark(
        profile=profile,
        verapdf=Summary.from_samples(verapdf_samples),
        page=Summary.from_samples(page_samples),
    )
    progress(
        "      complete: page "
        f"{format_speedup(result.verapdf, result.page)} versus veraPDF"
    )
    return result


def format_mib(size_bytes: int) -> str:
    return f"{size_bytes / (1024 * 1024):.1f}"


def format_speedup(reference: Summary, candidate: Summary) -> str:
    return f"{reference.median_seconds / candidate.median_seconds:.1f}×"


def profile_result(benchmark: DocumentBenchmark, profile_name: str) -> ProfileBenchmark:
    for result in benchmark.profiles:
        if result.profile == profile_name:
            return result
    raise ValueError(f"missing {profile_name} benchmark result")


def markdown(results: list[DocumentBenchmark]) -> str:
    output = [
        f"Each profile cell is the relative speedup of page over veraPDF for that profile (veraPDF runtime divided by page runtime); **higher is faster**. Values use the median of {RUNS} measured runs with {WARMUP_RUNS} warmup runs.\n",
        "The benchmark uses publicly available documents, you can find them [here](https://github.com/JosephBARBIERDARNAL/page-fixtures).\n",
        "| Document | Size (MiB) | Pages | PDF/A-1b | PDF/A-2b | PDF/UA-1 |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for document in results:
        row = [
            f"| {document.document} | {format_mib(document.size_bytes)} | {document.page_count}"
        ]
        for profile_name in PROFILES:
            result = profile_result(document, profile_name)
            row.append(format_speedup(result.verapdf, result.page))
        output.append(" | ".join(row) + " |")

    output.extend(
        [
            "!!! info\n",
            "       When details of which rule failed are not required, validation is expected to be much, much faster. Internally, this mode is called **lazy mode** and brings an **additional 2× to 10× speed improvement**. This is not represented in this benchmark to keep it simpler.\n",
        ]
    )
    return "\n".join(output)


def page_count(file: Path) -> int:
    completed = subprocess.run(
        ["pdfinfo", str(file)],
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(f"pdfinfo failed for {file}: {completed.stderr.strip()}")
    for line in completed.stdout.splitlines():
        if line.startswith("Pages:"):
            return int(line.split(":", maxsplit=1)[1].strip())
    raise RuntimeError(f"pdfinfo did not report pages for {file}")


def main() -> int:
    files = [ensure_fixture(filename) for filename in FULL_BENCHMARK_FILENAMES]
    progress(
        f"Benchmarking {len(files)} documents across {len(PROFILES)} profiles "
        f"with {RUNS} measured runs and {WARMUP_RUNS} warmup runs each"
    )

    results = []
    for document_index, file in enumerate(files):
        document = file.stem
        size_bytes = file.stat().st_size
        pages = page_count(file)
        progress(
            f"  document {document_index + 1}/{len(files)}: {document} "
            f"({size_bytes / (1024 * 1024):.1f} MiB, {pages} pages)"
        )
        profiles = []
        for profile_index, profile_name in enumerate(PROFILES):
            progress(f"    profile {profile_index + 1}/{len(PROFILES)}: {profile_name}")
            profiles.append(benchmark_profile(file, profile_name))
        results.append(
            DocumentBenchmark(
                document=document,
                size_bytes=size_bytes,
                page_count=pages,
                profiles=profiles,
            )
        )

    OUTPUT_PATH.write_text(markdown(results), encoding="utf-8")
    progress(f"Wrote {OUTPUT_PATH.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, ValueError) as error:
        print(f"benchmark failed: {error}", file=sys.stderr)
        sys.exit(1)
