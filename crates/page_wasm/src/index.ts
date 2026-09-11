import * as wasm from "../dist/page_validation.js";
import { createApi } from "./api.js";

export {
  FailureCategory,
  SafetyLimits,
  ValidationError,
  ValidationProfile,
  ValidationReport,
} from "./api.js";
export type {
  PdfDocument,
  PdfObjectId,
  SafetyLimitsOptions,
  ValidationCounts,
  ValidationFailure,
} from "./api.js";

const api = createApi(wasm);

export const initialize = api.initialize;
/** Alias for initialize, for callers accustomed to wasm-pack's init naming. */
export const init = api.init;
export const validatePdfBytes = api.validatePdfBytes;
export const isPdfCompliantBytes = api.isPdfCompliantBytes;
export const validate_pdf_bytes = api.validate_pdf_bytes;
export const is_pdf_compliant_bytes = api.is_pdf_compliant_bytes;
