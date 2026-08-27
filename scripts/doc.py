import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

PACKAGE_NAME = "page_validation"
OUTPUT_DIR = Path("docs/api/rust")

API_ITEMS = (
    "PdfError",
    "ValidationError",
    "SafetyLimits",
    "FailureCategory",
    "ValidationCounts",
    "ValidationFailure",
    "ValidationReport",
    "ValidationProfile",
    "ValidationInput",
    "ComplianceResult",
    "is_pdf_compliant",
    "validate_bytes",
    "validate_file",
)

KIND_LABELS = {
    "struct": "Struct",
    "enum": "Enum",
    "function": "Function",
    "trait": "Trait",
    "type_alias": "Type Alias",
    "constant": "Constant",
    "static": "Static",
    "union": "Union",
    "macro": "Macro",
}


@dataclass(frozen=True)
class ApiItem:
    id: str
    name: str
    kind: str
    path: tuple[str, ...]
    docs: str
    raw: dict[str, Any]


def run(
    command: list[str],
    *,
    cwd: Path,
    capture: bool = False,
    env: dict[str, str] | None = None,
) -> str:
    print("+", " ".join(command), file=sys.stderr)

    result = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        check=True,
        stdout=subprocess.PIPE if capture else None,
        env=env,
    )

    return result.stdout if capture else ""


def cargo_metadata(root: Path) -> dict[str, Any]:
    stdout = run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=root,
        capture=True,
    )

    return json.loads(stdout)


def select_package(metadata: dict[str, Any]) -> dict[str, Any]:
    for package in metadata["packages"]:
        if package["name"] == PACKAGE_NAME:
            return package

    raise SystemExit(f"package not found: {PACKAGE_NAME}")


def library_target(package: dict[str, Any]) -> dict[str, Any]:
    targets = [
        target
        for target in package["targets"]
        if any(
            kind in target["kind"]
            for kind in ("lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro")
        )
    ]

    if not targets:
        raise SystemExit(f"{PACKAGE_NAME} has no library target")

    if len(targets) > 1:
        names = ", ".join(target["name"] for target in targets)
        raise SystemExit(f"multiple library targets found: {names}")

    return targets[0]


def generate_rustdoc_json(root: Path, package: dict[str, Any]) -> None:
    env = dict(os.environ)
    env["RUSTDOCFLAGS"] = "-Z unstable-options --output-format json"

    run(
        [
            "cargo",
            "+nightly",
            "doc",
            "--no-deps",
            "--package",
            package["name"],
            "--lib",
        ],
        cwd=root,
        env=env,
    )


def rustdoc_json_path(metadata: dict[str, Any], target: dict[str, Any]) -> Path:
    target_dir = Path(metadata["target_directory"])
    crate_name = target["name"].replace("-", "_")

    return target_dir / "doc" / f"{crate_name}.json"


def item_kind(item: dict[str, Any]) -> str:
    inner = item.get("inner", {})

    if len(inner) != 1:
        return "unknown"

    return next(iter(inner))


def load_api_items(data: dict[str, Any]) -> dict[str, ApiItem]:
    index: dict[str, dict[str, Any]] = data["index"]
    paths: dict[str, dict[str, Any]] = data["paths"]

    wanted = set(API_ITEMS)

    candidates: dict[str, list[ApiItem]] = {name: [] for name in API_ITEMS}

    for item_id, item in index.items():
        name = item.get("name")

        if name not in wanted:
            continue

        summary = paths.get(item_id)

        if summary is None:
            continue

        path = tuple(summary.get("path", ()))

        candidates[name].append(
            ApiItem(
                id=item_id,
                name=name,
                kind=item_kind(item),
                path=path,
                docs=item.get("docs") or "",
                raw=item,
            )
        )

    result: dict[str, ApiItem] = {}

    for name, matches in candidates.items():
        if not matches:
            raise SystemExit(f"rustdoc item not found: {name}")

        matches.sort(
            key=lambda item: (not item.docs, len(item.path), "::".join(item.path))
        )

        selected = matches[0]

        equally_good = [
            item
            for item in matches
            if (bool(item.docs), len(item.path))
            == (bool(selected.docs), len(selected.path))
        ]

        if len(equally_good) > 1:
            paths_text = "\n  ".join("::".join(item.path) for item in equally_good)

            raise SystemExit(f"ambiguous rustdoc item {name!r}:\n  {paths_text}")

        result[name] = selected

    return result


def file_name(name: str) -> str:
    return f"{name}.md"


def rewrite_intra_doc_links(
    docs: str, item: ApiItem, selected_ids: dict[str, str]
) -> str:
    links: dict[str, str] = item.raw.get("links", {})

    for label, target_id in sorted(
        links.items(), key=lambda pair: len(pair[0]), reverse=True
    ):
        target_name = selected_ids.get(target_id)

        if target_name is None:
            continue

        destination = file_name(target_name)

        replacements = (
            (rf"(?<!\!)\[`{re.escape(label)}`\](?!\()", f"[`{label}`]({destination})"),
            (rf"(?<!\!)\[{re.escape(label)}\](?!\()", f"[{label}]({destination})"),
        )

        for pattern, replacement in replacements:
            docs = re.sub(pattern, replacement, docs)

    return docs


def render_item(item: ApiItem, selected_ids: dict[str, str]) -> str:
    docs = rewrite_intra_doc_links(item.docs.strip(), item, selected_ids)
    kind = KIND_LABELS.get(item.kind, item.kind.replace("_", " ").title())
    parts = [f"# `{item.name}`", "", f"**{kind}**", ""]

    if docs:
        parts.extend((docs, ""))
    else:
        parts.extend(("_No documentation provided._", ""))

    return "\n".join(parts)


def write_docs(output_dir: Path, items: dict[str, ApiItem]) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)

    selected_ids = {item.id: item.name for item in items.values()}

    for name in API_ITEMS:
        item = items[name]
        output = output_dir / file_name(name)

        output.write_text(render_item(item, selected_ids), encoding="utf-8")

        print(output)


def main() -> None:
    root = Path.cwd()

    metadata = cargo_metadata(root)
    package = select_package(metadata)
    target = library_target(package)

    generate_rustdoc_json(root, package)

    json_path = rustdoc_json_path(metadata, target)

    if not json_path.exists():
        raise SystemExit(f"rustdoc JSON not found: {json_path}")

    with json_path.open(encoding="utf-8") as file:
        data = json.load(file)

    items = load_api_items(data)

    write_docs(OUTPUT_DIR, items)


if __name__ == "__main__":
    main()
