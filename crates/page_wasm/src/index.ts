import * as wasm from "../dist/page_validation.js";
import { createApi } from "./api.js";

export { SafetyLimits, PageError, ValidationProfile, ValidationReport } from "./api.js";
export type {
  JsonValidationError,
  JsonValidationReport,
  SafetyLimitsOptions,
  ValidationCheckCounts,
  ValidationCounts,
  ValidationErrorKind,
  ValidationFailure,
  ValidationOptions,
} from "./api.js";

const api = createApi(wasm);

export const initialize = api.initialize;
export const validatePdfBytes = api.validatePdfBytes;
export const isPdfCompliantBytes = api.isPdfCompliantBytes;
