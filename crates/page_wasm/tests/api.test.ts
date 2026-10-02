import { describe, expect, it } from "bun:test";

import * as wasm from "../dist-bun/page_validation.js";
import {
  createApi,
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
    expect(limits.maxFormInvocations).toBe(SafetyLimits.DEFAULT_MAX_FORM_INVOCATIONS);
    expect(limits.maxTableSpan).toBe(SafetyLimits.DEFAULT_MAX_TABLE_SPAN);
    expect(limits.maxTableGridRows).toBe(SafetyLimits.DEFAULT_MAX_TABLE_GRID_ROWS);
    expect(limits.maxTableGridColumns).toBe(
      SafetyLimits.DEFAULT_MAX_TABLE_GRID_COLUMNS,
    );
    expect(limits.maxTableGridCells).toBe(SafetyLimits.DEFAULT_MAX_TABLE_GRID_CELLS);
    expect(limits.maxUnicodeCmapMappings).toBe(
      SafetyLimits.DEFAULT_MAX_UNICODE_CMAP_MAPPINGS,
    );
  });

  it("serializes custom safety limits", () => {
    const limits = new SafetyLimits({
      maxFormInvocations: 44,
      maxTableSpan: 9,
      maxTableGridRows: 10,
      maxTableGridColumns: 11,
      maxTableGridCells: 12,
      maxUnicodeCmapMappings: 13,
    });

    expect(limits.toJSON()).toMatchObject({
      max_form_invocations: 44,
      max_table_span: 9,
      max_table_grid_rows: 10,
      max_table_grid_columns: 11,
      max_table_grid_cells: 12,
      max_unicode_cmap_mappings: 13,
    });
  });

  it("creates independent unlimited presets and serializes every field", () => {
    const limits = SafetyLimits.unlimited();
    expect(Object.keys(limits).sort()).toEqual(Object.keys(new SafetyLimits()).sort());
    expect(Object.values(limits).every((value) => value === Infinity)).toBe(true);
    expect(Object.values(limits.toJSON()).every((value) => value === "unlimited")).toBe(
      true,
    );
    expect(JSON.stringify(limits)).not.toContain("null");

    limits.maxInputSize = 1;
    expect(limits.toJSON().max_input_size).toBe(1);
    expect(SafetyLimits.unlimited().maxInputSize).toBe(Infinity);
  });

  it("validates unlimited and mixed limits through the built Wasm module", async () => {
    const bytes = minimalPdf();
    const limits = SafetyLimits.unlimited();
    const report = await validatePdfBytes(bytes, {
      profile: ValidationProfile.PDF_A_1B,
      limits,
    });
    expect(report.isCompliant).toBe(false);
    expect(report.toJSON()).not.toHaveProperty("document");
    await expect(
      isPdfCompliantBytes(bytes, { profile: ValidationProfile.PDF_A_1B, limits }),
    ).resolves.toBe(false);

    limits.maxInputSize = 1;
    await expect(
      validatePdfBytes(bytes, { profile: ValidationProfile.PDF_A_1B, limits }),
    ).rejects.toThrow("1-byte limit");
    await expect(
      isPdfCompliantBytes(bytes, { profile: ValidationProfile.PDF_A_1B, limits }),
    ).rejects.toThrow("1-byte limit");

    const partialReport = await validatePdfBytes(bytes, {
      profile: ValidationProfile.PDF_A_1B,
      limits: { maxInputSize: Infinity },
    });
    expect(partialReport.isCompliant).toBe(false);
  });

  it("rejects invalid limit values during construction and after mutation", () => {
    for (const value of [NaN, -Infinity, -1, 0.5, Number.MAX_SAFE_INTEGER + 1]) {
      expect(() => new SafetyLimits({ maxInputSize: value })).toThrow(RangeError);
      const limits = SafetyLimits.unlimited();
      limits.maxInputSize = value;
      expect(() => JSON.stringify(limits)).toThrow(RangeError);
    }
  });

  it("rejects invalid unlimited tokens at the Wasm boundary", () => {
    for (const value of ["Infinity", "disabled", -1, 0.5]) {
      expect(() =>
        wasm.validatePdfBytes(
          minimalPdf(),
          "1b",
          JSON.stringify({ max_input_size: value }),
        ),
      ).toThrow("invalid safety limits");
    }
  });

  it("returns a typed report for byte input", async () => {
    const report = await validatePdfBytes(minimalPdf(), {
      profile: ValidationProfile.PDF_A_1B,
    });

    expect(report.profile).toBe(ValidationProfile.PDF_A_1B);
    expect(report.isCompliant).toBe(false);
    expect(report.failures.length).toBeGreaterThan(0);
    expect(report.failures[0]?.ruleId).toBeTruthy();
    expect(report.rules.total).toBeGreaterThan(0);
    expect(report.rules.failed).toBeGreaterThan(0);
    expect(report.checks.failed).toBeGreaterThan(0);
    expect(report).not.toHaveProperty("exitCode");

    const jsonReport = report.toJSON();
    expect(jsonReport).toMatchObject({
      profile: "1b",
      valid: false,
      rules: report.rules,
      checks: report.checks,
      failures: report.failures.map(({ ruleId, message }) => ({
        rule: ruleId,
        message,
      })),
    });
    expect(jsonReport).not.toHaveProperty("source");
    expect(jsonReport).not.toHaveProperty("document");
    expect(jsonReport).not.toHaveProperty("is_compliant");
    expect(JSON.parse(report.toJson())).toEqual(jsonReport);
  });

  it("returns the lazy compliance result", async () => {
    await expect(
      isPdfCompliantBytes(minimalPdf(), { profile: ValidationProfile.PDF_A_1B }),
    ).resolves.toBe(false);
  });

  it("raises ValidationError for malformed input", async () => {
    await expect(
      validatePdfBytes(new TextEncoder().encode("not a PDF"), {
        profile: ValidationProfile.PDF_A_1B,
      }),
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
      api.validatePdfBytes(minimalPdf(), {
        profile: ValidationProfile.PDF_A_1B,
        limits: invalidLimits,
      }),
    ).rejects.toBeInstanceOf(RangeError);
    await expect(
      api.isPdfCompliantBytes(minimalPdf(), {
        profile: ValidationProfile.PDF_A_1B,
        limits: invalidLimits,
      }),
    ).rejects.toBeInstanceOf(RangeError);

    expect(wasmCalls).toBe(0);
  });
});
