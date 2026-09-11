import { describe, expect, it } from "vitest";

import {
  FailureCategory,
  SafetyLimits,
  ValidationError,
  ValidationProfile,
  isPdfCompliantBytes,
  validatePdfBytes,
} from "../src/index.js";

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
});
