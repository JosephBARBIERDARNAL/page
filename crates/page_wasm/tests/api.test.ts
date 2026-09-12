import { describe, expect, it } from "bun:test";

import * as wasm from "../dist-bun/page_validation.js";
import {
  createApi,
  FailureCategory,
  SafetyLimits,
  ValidationError,
  ValidationProfile,
} from "../src/api.js";

const { isPdfCompliantBytes, validatePdfBytes } = createApi(wasm);

function minimalPdf(): Uint8Array {
  const objects = [
    "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
    "2 0 obj\n<< /Type /Pages /Kids [] /Count 0 >>\nendobj\n",
  ];
  let data = "%PDF-1.4\n";
  const offsets: number[] = [];
  for (const pdfObject of objects) {
    offsets.push(data.length);
    data += pdfObject;
  }
  const xrefOffset = data.length;
  data += "xref\n0 3\n0000000000 65535 f \n";
  data += offsets
    .map((offset) => `${offset.toString().padStart(10, "0")} 00000 n \n`)
    .join("");
  data += `trailer\n<< /Size 3 /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF\n`;
  return new TextEncoder().encode(data);
}

describe("page-validation", () => {
  it("exposes the upstream safety-limit defaults", () => {
    const limits = new SafetyLimits();

    expect(limits.maxInputSize).toBe(256 * 1024 * 1024);
    expect(limits.maxObjectCount).toBe(1_000_000);
    expect(limits.maxReferenceDepth).toBe(256);
    expect(limits.maxTableSpan).toBe(SafetyLimits.DEFAULT_MAX_TABLE_SPAN);
    expect(limits.maxTableGridRows).toBe(SafetyLimits.DEFAULT_MAX_TABLE_GRID_ROWS);
    expect(limits.maxTableGridColumns).toBe(
      SafetyLimits.DEFAULT_MAX_TABLE_GRID_COLUMNS,
    );
    expect(limits.maxTableGridCells).toBe(SafetyLimits.DEFAULT_MAX_TABLE_GRID_CELLS);
  });

  it("serializes custom table safety limits", () => {
    const limits = new SafetyLimits({
      maxTableSpan: 9,
      maxTableGridRows: 10,
      maxTableGridColumns: 11,
      maxTableGridCells: 12,
    });

    expect(limits.toJSON()).toMatchObject({
      max_table_span: 9,
      max_table_grid_rows: 10,
      max_table_grid_columns: 11,
      max_table_grid_cells: 12,
    });
  });

  it("returns a typed report for byte input", async () => {
    const report = await validatePdfBytes(minimalPdf(), ValidationProfile.PDF_A_1B);

    expect(report.profile).toBe(ValidationProfile.PDF_A_1B);
    expect(report.isCompliant).toBe(false);
    expect(report.failures.length).toBeGreaterThan(0);
    expect(report.failures[0]?.category).toBe(FailureCategory.CONFORMANCE);
    expect(report.exitCode()).toBe(2);
  });

  it("returns the fast compliance result", async () => {
    await expect(
      isPdfCompliantBytes(minimalPdf(), ValidationProfile.PDF_A_1B),
    ).resolves.toBe(false);
  });

  it("raises ValidationError for malformed input", async () => {
    await expect(
      validatePdfBytes(
        new TextEncoder().encode("not a PDF"),
        ValidationProfile.PDF_A_1B,
      ),
    ).rejects.toBeInstanceOf(ValidationError);
  });

  it("preserves configuration errors before invoking WASM", async () => {
    let wasmCalls = 0;
    const api = createApi({
      validatePdfBytes: () => {
        wasmCalls += 1;
        return "";
      },
      isPdfCompliantBytes: () => {
        wasmCalls += 1;
        return false;
      },
    });
    const invalidLimits = { maxInputSize: -1 };

    await expect(
      api.validatePdfBytes(minimalPdf(), ValidationProfile.PDF_A_1B, invalidLimits),
    ).rejects.toBeInstanceOf(RangeError);
    await expect(
      api.isPdfCompliantBytes(minimalPdf(), ValidationProfile.PDF_A_1B, invalidLimits),
    ).rejects.toBeInstanceOf(RangeError);

    expect(wasmCalls).toBe(0);
  });
});
