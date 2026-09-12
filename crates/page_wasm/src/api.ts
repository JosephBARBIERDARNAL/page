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
  PDF_A_4 = "4",
  PDF_A_4E = "4e",
  PDF_A_4F = "4f",
  PDF_UA_1 = "ua1",
  PDF_UA_2 = "ua2",
}

export enum FailureCategory {
  OPERATIONAL = "operational",
  PARSER = "parser",
  METADATA = "metadata",
  CONFORMANCE = "conformance",
}

export interface SafetyLimitsOptions {
  maxInputSize: number;
  maxDecodedStreamSize: number;
  maxTotalDecodedContentSize: number;
  maxObjectCount: number;
  maxReferenceDepth: number;
  maxXrefRevisions: number;
  maxTableSpan: number;
  maxTableGridRows: number;
  maxTableGridColumns: number;
  maxTableGridCells: number;
}

const DEFAULT_SAFETY_LIMITS: SafetyLimitsOptions = {
  maxInputSize: 256 * 1024 * 1024,
  maxDecodedStreamSize: 32 * 1024 * 1024,
  maxTotalDecodedContentSize: 256 * 1024 * 1024,
  maxObjectCount: 1_000_000,
  maxReferenceDepth: 256,
  maxXrefRevisions: 1_024,
  maxTableSpan: 1_024,
  maxTableGridRows: 1_024,
  maxTableGridColumns: 1_024,
  maxTableGridCells: 1_000_000,
};

export class SafetyLimits implements SafetyLimitsOptions {
  static readonly DEFAULT_MAX_INPUT_SIZE = DEFAULT_SAFETY_LIMITS.maxInputSize;
  static readonly DEFAULT_MAX_DECODED_STREAM_SIZE =
    DEFAULT_SAFETY_LIMITS.maxDecodedStreamSize;
  static readonly DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE =
    DEFAULT_SAFETY_LIMITS.maxTotalDecodedContentSize;
  static readonly DEFAULT_MAX_OBJECT_COUNT = DEFAULT_SAFETY_LIMITS.maxObjectCount;
  static readonly DEFAULT_MAX_REFERENCE_DEPTH = DEFAULT_SAFETY_LIMITS.maxReferenceDepth;
  static readonly DEFAULT_MAX_XREF_REVISIONS = DEFAULT_SAFETY_LIMITS.maxXrefRevisions;
  static readonly DEFAULT_MAX_TABLE_SPAN = DEFAULT_SAFETY_LIMITS.maxTableSpan;
  static readonly DEFAULT_MAX_TABLE_GRID_ROWS = DEFAULT_SAFETY_LIMITS.maxTableGridRows;
  static readonly DEFAULT_MAX_TABLE_GRID_COLUMNS =
    DEFAULT_SAFETY_LIMITS.maxTableGridColumns;
  static readonly DEFAULT_MAX_TABLE_GRID_CELLS =
    DEFAULT_SAFETY_LIMITS.maxTableGridCells;

  maxInputSize: number;
  maxDecodedStreamSize: number;
  maxTotalDecodedContentSize: number;
  maxObjectCount: number;
  maxReferenceDepth: number;
  maxXrefRevisions: number;
  maxTableSpan: number;
  maxTableGridRows: number;
  maxTableGridColumns: number;
  maxTableGridCells: number;

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
  }

  toJSON(): Record<string, number> {
    return {
      max_input_size: this.maxInputSize,
      max_decoded_stream_size: this.maxDecodedStreamSize,
      max_total_decoded_content_size: this.maxTotalDecodedContentSize,
      max_object_count: this.maxObjectCount,
      max_reference_depth: this.maxReferenceDepth,
      max_xref_revisions: this.maxXrefRevisions,
      max_table_span: this.maxTableSpan,
      max_table_grid_rows: this.maxTableGridRows,
      max_table_grid_columns: this.maxTableGridColumns,
      max_table_grid_cells: this.maxTableGridCells,
    };
  }
}

export interface PdfObjectId {
  objectNumber: number;
  generation: number;
}

export interface ValidationFailure {
  ruleId: string;
  message: string;
  objectId: PdfObjectId | null;
  category: FailureCategory;
}

export interface ValidationCounts {
  total: number;
  passed: number;
  failed: number;
}

export interface PdfDocument {
  version: string;
  encrypted: boolean;
  pageCount: number;
  objectCount: number;
}

interface RawValidationReport {
  source: string | null;
  profile: string;
  is_compliant: boolean;
  preliminary: boolean;
  checks: {
    total: number;
    passed: number;
    failed: number;
  };
  document: {
    version: string;
    encrypted: boolean;
    page_count: number;
    object_count: number;
  } | null;
  failures: {
    rule_id: string;
    message: string;
    object_id: {
      object_number: number;
      generation: number;
    } | null;
    category: string;
  }[];
}

export class ValidationReport {
  readonly source: string | null;
  readonly profile: ValidationProfile;
  readonly isCompliant: boolean;
  readonly preliminary: boolean;
  readonly checks: ValidationCounts;
  readonly document: PdfDocument | null;
  readonly failures: ValidationFailure[];
  private readonly raw: RawValidationReport;

  constructor(raw: RawValidationReport) {
    this.raw = raw;
    this.source = raw.source;
    this.profile = raw.profile as ValidationProfile;
    this.isCompliant = raw.is_compliant;
    this.preliminary = raw.preliminary;
    this.checks = raw.checks;
    this.document = raw.document
      ? {
          version: raw.document.version,
          encrypted: raw.document.encrypted,
          pageCount: raw.document.page_count,
          objectCount: raw.document.object_count,
        }
      : null;
    this.failures = raw.failures.map((failure) => ({
      ruleId: failure.rule_id,
      message: failure.message,
      objectId: failure.object_id
        ? {
            objectNumber: failure.object_id.object_number,
            generation: failure.object_id.generation,
          }
        : null,
      category: failure.category as FailureCategory,
    }));
  }

  hasOperationalFailure(): boolean {
    return this.failures.some(
      (failure) => failure.category === FailureCategory.OPERATIONAL,
    );
  }

  exitCode(): number {
    if (this.hasOperationalFailure()) {
      return 1;
    }
    return this.isCompliant ? 0 : 2;
  }

  toJson(): string {
    return JSON.stringify(this.raw);
  }

  toJSON(): RawValidationReport {
    return this.raw;
  }
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
    profile?: ValidationProfile,
    limits?: SafetyLimits | Partial<SafetyLimitsOptions>,
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
    profile?: ValidationProfile,
    limits?: SafetyLimits | Partial<SafetyLimitsOptions>,
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
    init: initialize,
    validatePdfBytes,
    isPdfCompliantBytes,
    validate_pdf_bytes: validatePdfBytes,
    is_pdf_compliant_bytes: isPdfCompliantBytes,
  };
}

function validateLimit(value: number, name: string): number {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new RangeError(`${name} must be a non-negative safe integer`);
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
