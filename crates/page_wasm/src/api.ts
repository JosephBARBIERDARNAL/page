export interface WasmBindings {
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

const DEFAULT_SAFETY_LIMITS: SafetyLimitsOptions = {
  maxInputSize: 256 * 1024 * 1024,
  maxDecodedStreamSize: 32 * 1024 * 1024,
  maxTotalDecodedContentSize: 256 * 1024 * 1024,
  maxFormInvocations: 10_000,
  maxObjectCount: 1_000_000,
  maxReferenceDepth: 256,
  maxXrefRevisions: 1_024,
  maxTableSpan: 1_024,
  maxTableGridRows: 1_024,
  maxTableGridColumns: 1_024,
  maxTableGridCells: 1_000_000,
  maxUnicodeCmapMappings: 1_000_000,
};

export class SafetyLimits implements SafetyLimitsOptions {
  /** Disables all configurable safety limits. Use only with trusted files. */
  static unlimited(): SafetyLimits {
    return new SafetyLimits(
      Object.fromEntries(
        Object.keys(DEFAULT_SAFETY_LIMITS).map((name) => [name, Infinity]),
      ),
    );
  }

  static readonly DEFAULT_MAX_INPUT_SIZE = DEFAULT_SAFETY_LIMITS.maxInputSize;
  static readonly DEFAULT_MAX_DECODED_STREAM_SIZE =
    DEFAULT_SAFETY_LIMITS.maxDecodedStreamSize;
  static readonly DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE =
    DEFAULT_SAFETY_LIMITS.maxTotalDecodedContentSize;
  static readonly DEFAULT_MAX_FORM_INVOCATIONS =
    DEFAULT_SAFETY_LIMITS.maxFormInvocations;
  static readonly DEFAULT_MAX_OBJECT_COUNT = DEFAULT_SAFETY_LIMITS.maxObjectCount;
  static readonly DEFAULT_MAX_REFERENCE_DEPTH = DEFAULT_SAFETY_LIMITS.maxReferenceDepth;
  static readonly DEFAULT_MAX_XREF_REVISIONS = DEFAULT_SAFETY_LIMITS.maxXrefRevisions;
  static readonly DEFAULT_MAX_TABLE_SPAN = DEFAULT_SAFETY_LIMITS.maxTableSpan;
  static readonly DEFAULT_MAX_TABLE_GRID_ROWS = DEFAULT_SAFETY_LIMITS.maxTableGridRows;
  static readonly DEFAULT_MAX_TABLE_GRID_COLUMNS =
    DEFAULT_SAFETY_LIMITS.maxTableGridColumns;
  static readonly DEFAULT_MAX_TABLE_GRID_CELLS =
    DEFAULT_SAFETY_LIMITS.maxTableGridCells;
  static readonly DEFAULT_MAX_UNICODE_CMAP_MAPPINGS =
    DEFAULT_SAFETY_LIMITS.maxUnicodeCmapMappings;

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
    this.maxInputSize = validateLimit(
      options.maxInputSize ?? DEFAULT_SAFETY_LIMITS.maxInputSize,
      "maxInputSize",
    );
    this.maxDecodedStreamSize = validateLimit(
      options.maxDecodedStreamSize ?? DEFAULT_SAFETY_LIMITS.maxDecodedStreamSize,
      "maxDecodedStreamSize",
    );
    this.maxTotalDecodedContentSize = validateLimit(
      options.maxTotalDecodedContentSize ??
        DEFAULT_SAFETY_LIMITS.maxTotalDecodedContentSize,
      "maxTotalDecodedContentSize",
    );
    this.maxFormInvocations = validateLimit(
      options.maxFormInvocations ?? DEFAULT_SAFETY_LIMITS.maxFormInvocations,
      "maxFormInvocations",
    );
    this.maxObjectCount = validateLimit(
      options.maxObjectCount ?? DEFAULT_SAFETY_LIMITS.maxObjectCount,
      "maxObjectCount",
    );
    this.maxReferenceDepth = validateLimit(
      options.maxReferenceDepth ?? DEFAULT_SAFETY_LIMITS.maxReferenceDepth,
      "maxReferenceDepth",
    );
    this.maxXrefRevisions = validateLimit(
      options.maxXrefRevisions ?? DEFAULT_SAFETY_LIMITS.maxXrefRevisions,
      "maxXrefRevisions",
    );
    this.maxTableSpan = validateLimit(
      options.maxTableSpan ?? DEFAULT_SAFETY_LIMITS.maxTableSpan,
      "maxTableSpan",
    );
    this.maxTableGridRows = validateLimit(
      options.maxTableGridRows ?? DEFAULT_SAFETY_LIMITS.maxTableGridRows,
      "maxTableGridRows",
    );
    this.maxTableGridColumns = validateLimit(
      options.maxTableGridColumns ?? DEFAULT_SAFETY_LIMITS.maxTableGridColumns,
      "maxTableGridColumns",
    );
    this.maxTableGridCells = validateLimit(
      options.maxTableGridCells ?? DEFAULT_SAFETY_LIMITS.maxTableGridCells,
      "maxTableGridCells",
    );
    this.maxUnicodeCmapMappings = validateLimit(
      options.maxUnicodeCmapMappings ?? DEFAULT_SAFETY_LIMITS.maxUnicodeCmapMappings,
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
  rule: string;
  message: string;
}

export interface JsonValidationReport {
  file?: string;
  profile?: ValidationProfile;
  valid: boolean;
  rules?: ValidationCounts;
  checks?: ValidationCheckCounts;
  failures: { rule: string; message: string }[];
  error?: JsonValidationError;
}

interface RawValidationReport extends JsonValidationReport {
  profile: ValidationProfile;
  rules: ValidationCounts;
  checks: ValidationCheckCounts;
}

export class ValidationReport {
  readonly file: string | undefined;
  readonly source: string | null;
  readonly profile: ValidationProfile;
  readonly valid: boolean;
  readonly isCompliant: boolean;
  readonly rules: ValidationCounts;
  readonly checks: ValidationCheckCounts;
  readonly failures: ValidationFailure[];
  private readonly raw: RawValidationReport;

  constructor(raw: RawValidationReport) {
    this.raw = raw;
    this.file = raw.file;
    this.source = raw.file ?? null;
    this.profile = raw.profile as ValidationProfile;
    this.valid = raw.valid;
    this.isCompliant = raw.valid;
    this.rules = raw.rules;
    this.checks = raw.checks;
    this.failures = raw.failures.map((failure) => ({
      ruleId: failure.rule,
      message: failure.message,
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

export class ValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ValidationError";
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export function createApi(wasm: WasmBindings) {
  let initialization: Promise<void> | undefined;

  /** Initializes the WebAssembly module. Validation functions initialize it automatically. */
  function initialize(): Promise<void> {
    initialization = initialization ?? Promise.resolve();
    return initialization;
  }

  async function validatePdfBytes(
    bytes: Uint8Array,
    { profile, limits }: ValidationOptions = {},
  ): Promise<ValidationReport> {
    await initialize();
    const serializedLimits = serializeLimits(limits);
    try {
      const json = wasm.validatePdfBytes(bytes, profile, serializedLimits);
      return new ValidationReport(JSON.parse(json) as RawValidationReport);
    } catch (error) {
      throw asValidationError(error);
    }
  }

  async function isPdfCompliantBytes(
    bytes: Uint8Array,
    { profile, limits }: ValidationOptions = {},
  ): Promise<boolean> {
    await initialize();
    const serializedLimits = serializeLimits(limits);
    try {
      return wasm.isPdfCompliantBytes(bytes, profile, serializedLimits);
    } catch (error) {
      throw asValidationError(error);
    }
  }

  return {
    initialize,
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

function asValidationError(error: unknown): ValidationError {
  if (error instanceof ValidationError) {
    return error;
  }
  if (error instanceof Error && error.name === "ValidationError") {
    return new ValidationError(error.message);
  }
  return error instanceof Error
    ? new ValidationError(error.message)
    : new ValidationError(String(error));
}
