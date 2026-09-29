import hashlib
import os
import shutil
import tempfile
from pathlib import Path
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
BENCH_DIRECTORY = ROOT / "bench"
FIXTURE_REVISION = "2b4e2bbedb17288de315b5f50f7ae7c715bcf6e7"
FIXTURE_BASE_URL = (
    "https://raw.githubusercontent.com/JosephBARBIERDARNAL/page-fixtures/"
    f"{FIXTURE_REVISION}/pdf"
)
FIXTURE_SHA256 = {
    "gao-2024.pdf": "1c39995431cd7b758e574a09dc5e70d65729c4f880e0e9c7ef30705068ef3882",
    "health-of-canadians-2025.pdf": "1a7f93bd8597bcd154c8d7e5cddc678e17f17c96e1d88596cf107adbddaff6e9",
    "standards-for-development-world-bank.pdf": "9613c40cc2ad947188dfddffaba3b4a097b29f51aecbfec8c90b69a8318be536",
    "surveillance-drug-risk-2017.pdf": "6a00ec5c12878beac1b99acc4d536e9581117ef4c2f3a3c24bb833e67bb88ff9",
    "world-health-statistics-2025.pdf": "29d2c4edd1ebc1f7f2ac2f01303290da239fe31a4d23365ba9933bb64d1d101e",
}
FULL_BENCHMARK_FILENAMES = tuple(FIXTURE_SHA256)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as pdf:
        for chunk in iter(lambda: pdf.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def ensure_fixture(filename: str) -> Path:
    expected_sha256 = FIXTURE_SHA256.get(filename)
    if expected_sha256 is None:
        raise ValueError(f"unknown page-fixtures PDF: {filename}")

    destination = BENCH_DIRECTORY / filename
    if destination.is_file():
        actual_sha256 = sha256_file(destination)
        if actual_sha256 != expected_sha256:
            raise ValueError(
                f"SHA-256 mismatch for {destination}: "
                f"expected {expected_sha256}, got {actual_sha256}"
            )
        return destination

    BENCH_DIRECTORY.mkdir(parents=True, exist_ok=True)
    url = f"{FIXTURE_BASE_URL}/{filename}"
    temporary_path = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="wb", dir=BENCH_DIRECTORY, prefix=f".{filename}.", delete=False
        ) as temporary:
            temporary_path = Path(temporary.name)
            request = Request(url, headers={"User-Agent": "page-benchmark"})
            with urlopen(request, timeout=60) as response:
                shutil.copyfileobj(response, temporary)

        with temporary_path.open("rb") as downloaded:
            if b"%PDF-" not in downloaded.read(1024):
                raise ValueError(f"downloaded fixture is not a PDF: {url}")

        actual_sha256 = sha256_file(temporary_path)
        if actual_sha256 != expected_sha256:
            raise ValueError(
                f"SHA-256 mismatch for downloaded {filename}: "
                f"expected {expected_sha256}, got {actual_sha256}"
            )

        os.replace(temporary_path, destination)
        temporary_path = None
        print(f"Downloaded {filename} to {destination}", flush=True)
        return destination
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)
