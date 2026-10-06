export interface WasmBindings {
  defaultSafetyLimits(): string;
  isPdfCompliantBytes(
    bytes: Uint8Array,
    profile?: string | null,
    limitsJson?: string | null,
  ): boolean;
  validatePdfBytes(
    bytes: Uint8Array,
    profile?: string | null,
    limitsJson?: string | null,
  ): string;
}

export enum ValidationProfile {
  PDF_A_1A = "1a",
  PDF_A_1B = "1b",
  PDF_A_2A = "2a",
  PDF_A_2B = "2b",
  PDF_A_2U = "2u",
  PDF_A_3A = "3a",
  PDF_A_3B = "3b",
  PDF_A_3U = "3u",
  PDF_UA_1 = "ua1",
}

export interface SafetyLimitsOptions {
  maxInputSize: number;
  maxDecodedStreamSize: number;
  maxTotalDecodedContentSize: number;
  maxFormInvocations: number;
  maxObjectCount: number;
  maxReferenceDepth: number;
  maxXrefRevisions: number;
  maxTableSpan: number;
  maxTableGridRows: number;
  maxTableGridColumns: number;
  maxTableGridCells: number;
  maxUnicodeCmapMappings: number;
}

let defaultSafetyLimits: SafetyLimitsOptions | undefined;

function getDefaultSafetyLimits(): SafetyLimitsOptions {
  if (defaultSafetyLimits === undefined) {
    throw new Error("The WebAssembly module has not provided safety-limit defaults");
  }
  return defaultSafetyLimits;
}

export class SafetyLimits implements SafetyLimitsOptions {
  /** Disables all configurable safety limits. Use only with trusted files. */
  static unlimited(): SafetyLimits {
    return new SafetyLimits(
      Object.fromEntries(
        Object.keys(getDefaultSafetyLimits()).map((name) => [name, Infinity]),
      ),
    );
  }

  static get DEFAULT_MAX_INPUT_SIZE(): number {
    return getDefaultSafetyLimits().maxInputSize;
  }
  static get DEFAULT_MAX_DECODED_STREAM_SIZE(): number {
    return getDefaultSafetyLimits().maxDecodedStreamSize;
  }
  static get DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE(): number {
    return getDefaultSafetyLimits().maxTotalDecodedContentSize;
  }
  static get DEFAULT_MAX_FORM_INVOCATIONS(): number {
    return getDefaultSafetyLimits().maxFormInvocations;
  }
  static get DEFAULT_MAX_OBJECT_COUNT(): number {
    return getDefaultSafetyLimits().maxObjectCount;
  }
  static get DEFAULT_MAX_REFERENCE_DEPTH(): number {
    return getDefaultSafetyLimits().maxReferenceDepth;
  }
  static get DEFAULT_MAX_XREF_REVISIONS(): number {
    return getDefaultSafetyLimits().maxXrefRevisions;
  }
  static get DEFAULT_MAX_TABLE_SPAN(): number {
    return getDefaultSafetyLimits().maxTableSpan;
  }
  static get DEFAULT_MAX_TABLE_GRID_ROWS(): number {
    return getDefaultSafetyLimits().maxTableGridRows;
  }
  static get DEFAULT_MAX_TABLE_GRID_COLUMNS(): number {
    return getDefaultSafetyLimits().maxTableGridColumns;
  }
  static get DEFAULT_MAX_TABLE_GRID_CELLS(): number {
    return getDefaultSafetyLimits().maxTableGridCells;
  }
  static get DEFAULT_MAX_UNICODE_CMAP_MAPPINGS(): number {
    return getDefaultSafetyLimits().maxUnicodeCmapMappings;
  }

  maxInputSize: number;
  maxDecodedStreamSize: number;
  maxTotalDecodedContentSize: number;
  maxFormInvocations: number;
  maxObjectCount: number;
  maxReferenceDepth: number;
  maxXrefRevisions: number;
  maxTableSpan: number;
  maxTableGridRows: number;
  maxTableGridColumns: number;
  maxTableGridCells: number;
  maxUnicodeCmapMappings: number;

  constructor(options: Partial<SafetyLimitsOptions> = {}) {
    const defaults = getDefaultSafetyLimits();
    this.maxInputSize = validateLimit(
      options.maxInputSize ?? defaults.maxInputSize,
      "maxInputSize",
    );
    this.maxDecodedStreamSize = validateLimit(
      options.maxDecodedStreamSize ?? defaults.maxDecodedStreamSize,
      "maxDecodedStreamSize",
    );
    this.maxTotalDecodedContentSize = validateLimit(
      options.maxTotalDecodedContentSize ?? defaults.maxTotalDecodedContentSize,
      "maxTotalDecodedContentSize",
    );
    this.maxFormInvocations = validateLimit(
      options.maxFormInvocations ?? defaults.maxFormInvocations,
      "maxFormInvocations",
    );
    this.maxObjectCount = validateLimit(
      options.maxObjectCount ?? defaults.maxObjectCount,
      "maxObjectCount",
    );
    this.maxReferenceDepth = validateLimit(
      options.maxReferenceDepth ?? defaults.maxReferenceDepth,
      "maxReferenceDepth",
    );
    this.maxXrefRevisions = validateLimit(
      options.maxXrefRevisions ?? defaults.maxXrefRevisions,
      "maxXrefRevisions",
    );
    this.maxTableSpan = validateLimit(
      options.maxTableSpan ?? defaults.maxTableSpan,
      "maxTableSpan",
    );
    this.maxTableGridRows = validateLimit(
      options.maxTableGridRows ?? defaults.maxTableGridRows,
      "maxTableGridRows",
    );
    this.maxTableGridColumns = validateLimit(
      options.maxTableGridColumns ?? defaults.maxTableGridColumns,
      "maxTableGridColumns",
    );
    this.maxTableGridCells = validateLimit(
      options.maxTableGridCells ?? defaults.maxTableGridCells,
      "maxTableGridCells",
    );
    this.maxUnicodeCmapMappings = validateLimit(
      options.maxUnicodeCmapMappings ?? defaults.maxUnicodeCmapMappings,
      "maxUnicodeCmapMappings",
    );
  }

  toJSON(): Record<string, number | "unlimited"> {
    const limits = {
      max_input_size: this.maxInputSize,
      max_decoded_stream_size: this.maxDecodedStreamSize,
      max_total_decoded_content_size: this.maxTotalDecodedContentSize,
      max_form_invocations: this.maxFormInvocations,
      max_object_count: this.maxObjectCount,
      max_reference_depth: this.maxReferenceDepth,
      max_xref_revisions: this.maxXrefRevisions,
      max_table_span: this.maxTableSpan,
      max_table_grid_rows: this.maxTableGridRows,
      max_table_grid_columns: this.maxTableGridColumns,
      max_table_grid_cells: this.maxTableGridCells,
      max_unicode_cmap_mappings: this.maxUnicodeCmapMappings,
    };
    return Object.fromEntries(
      Object.entries(limits).map(([name, value]) => [
        name,
        validateLimit(value, name) === Infinity ? "unlimited" : value,
      ]),
    );
  }
}

export interface ValidationFailure {
  ruleId: string;
  message: string;
  objectId: { objectNumber: number; generation: number } | null;
  category: "metadata" | "conformance";
}

export interface ValidationCounts {
  total: number;
  passed: number;
  failed: number;
}

export interface ValidationCheckCounts {
  failed: number;
}

export interface JsonValidationError {
  kind: "parser" | "operational";
  rule_id: string;
  message: string;
}

export interface JsonValidationReport {
  source?: string;
  profile?: ValidationProfile;
  is_compliant: boolean;
  rules?: ValidationCounts;
  checks?: ValidationCheckCounts;
  failures: {
    rule_id: string;
    message: string;
    object_id: { object_number: number; generation: number } | null;
    category: "metadata" | "conformance";
  }[];
  error?: JsonValidationError;
}

interface RawValidationReport extends JsonValidationReport {
  profile: ValidationProfile;
  rules: ValidationCounts;
  checks: ValidationCheckCounts;
}

export class ValidationReport {
  readonly source: string | null;
  readonly profile: ValidationProfile;
  readonly isCompliant: boolean;
  readonly rules: ValidationCounts;
  readonly checks: ValidationCheckCounts;
  readonly failures: ValidationFailure[];
  private readonly raw: RawValidationReport;

  constructor(raw: RawValidationReport) {
    this.raw = raw;
    this.source = raw.source ?? null;
    this.profile = raw.profile as ValidationProfile;
    this.isCompliant = raw.is_compliant;
    this.rules = raw.rules;
    this.checks = raw.checks;
    this.failures = raw.failures.map((failure) => ({
      ruleId: failure.rule_id,
      message: failure.message,
      objectId:
        failure.object_id === null
          ? null
          : {
              objectNumber: failure.object_id.object_number,
              generation: failure.object_id.generation,
            },
      category: failure.category,
    }));
  }

  toJson(): string {
    return JSON.stringify(this.raw);
  }

  toJSON(): JsonValidationReport {
    return this.raw;
  }
}

/** Options shared by every validation function. */
export interface ValidationOptions {
  /** Profile to validate against; inferred from the document's XMP metadata when omitted. */
  profile?: ValidationProfile;
  /** Resource bounds enforced while parsing and inspecting the document. */
  limits?: SafetyLimits | Partial<SafetyLimitsOptions>;
}

export type ValidationErrorKind =
  "input_io" | "parser" | "safety_limit" | "profile" | "unknown";

export class PageError extends Error {
  readonly kind: ValidationErrorKind;
  readonly ruleId: string | undefined;

  constructor(message: string, kind: ValidationErrorKind = "unknown", ruleId?: string) {
    super(message);
    this.name = "PageError";
    this.kind = kind;
    this.ruleId = ruleId;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export function createApi(wasm: WasmBindings) {
  defaultSafetyLimits = JSON.parse(wasm.defaultSafetyLimits()) as SafetyLimitsOptions;

  async function validatePdfBytes(
    bytes: Uint8Array,
    { profile, limits }: ValidationOptions = {},
  ): Promise<ValidationReport> {
    const serializedLimits = serializeLimits(limits);
    try {
      const json = wasm.validatePdfBytes(bytes, profile, serializedLimits);
      return new ValidationReport(JSON.parse(json) as RawValidationReport);
    } catch (error) {
      throw asPageError(error);
    }
  }

  async function isPdfCompliantBytes(
    bytes: Uint8Array,
    { profile, limits }: ValidationOptions = {},
  ): Promise<boolean> {
    const serializedLimits = serializeLimits(limits);
    try {
      return wasm.isPdfCompliantBytes(bytes, profile, serializedLimits);
    } catch (error) {
      throw asPageError(error);
    }
  }

  return {
    validatePdfBytes,
    isPdfCompliantBytes,
  };
}

function validateLimit(value: number, name: string): number {
  if (value !== Infinity && (!Number.isSafeInteger(value) || value < 0)) {
    throw new RangeError(`${name} must be a non-negative safe integer or Infinity`);
  }
  return value;
}

function serializeLimits(
  limits: SafetyLimits | Partial<SafetyLimitsOptions> | undefined,
): string | undefined {
  if (limits === undefined) {
    return undefined;
  }
  return JSON.stringify(
    limits instanceof SafetyLimits ? limits : new SafetyLimits(limits),
  );
}

function asPageError(error: unknown): PageError {
  if (error instanceof PageError) {
    return error;
  }
  if (error instanceof Error && error.name === "PageError") {
    const typedError = error as Error & { kind?: unknown; ruleId?: unknown };
    return new PageError(
      error.message,
      isValidationErrorKind(typedError.kind) ? typedError.kind : "unknown",
      typeof typedError.ruleId === "string" ? typedError.ruleId : undefined,
    );
  }
  return error instanceof Error
    ? new PageError(error.message)
    : new PageError(String(error));
}

function isValidationErrorKind(value: unknown): value is ValidationErrorKind {
  return (
    value === "input_io" ||
    value === "parser" ||
    value === "safety_limit" ||
    value === "profile"
  );
}
