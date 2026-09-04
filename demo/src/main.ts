import "./style.css";

const MAX_FILE_BYTES = 50 * 1024 * 1024;

type DemoRoot = HTMLElement & {
  dataset: DOMStringMap & { validatorInitialized?: string };
};

type ValidationReport = {
  isCompliant: boolean;
  failures?: Array<{
    ruleId?: string;
    message?: string;
    category?: string;
  }>;
};

function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KiB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

function createElement<K extends keyof HTMLElementTagNameMap>(
  tagName: K,
  className?: string,
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tagName);
  if (className) {
    element.className = className;
  }
  return element;
}

function renderReport(output: HTMLElement, report: ValidationReport): void {
  output.replaceChildren();
  output.className = report.isCompliant
    ? "validator-result validator-result--pass"
    : "validator-result validator-result--fail";

  const heading = createElement("h3");
  heading.textContent = report.isCompliant
    ? "Document is compliant"
    : "Document is not compliant";
  output.append(heading);

  const failures = report.failures ?? [];
  if (failures.length > 0) {
    const summary = createElement("p");
    summary.textContent = `${failures.length} validation failure${failures.length === 1 ? "" : "s"}.`;
    output.append(summary);

    const list = createElement("ul", "validator-failures");
    for (const failure of failures) {
      const item = createElement("li");
      const header = createElement("div", "validator-failure-header");
      const rule = createElement("code", "validator-failure-rule");
      rule.textContent = failure.ruleId ?? "—";
      header.append(rule);

      if (failure.category) {
        const category = createElement("span", "validator-failure-category");
        category.textContent = failure.category;
        header.append(category);
      }

      const message = createElement("span", "validator-failure-message");
      message.textContent = failure.message ?? "—";
      item.append(header, message);
      list.append(item);
    }
    output.append(list);
  }
}

function renderError(output: HTMLElement, error: unknown): void {
  output.replaceChildren();
  output.className = "validator-result validator-result--error";
  const message = createElement("p");
  message.textContent =
    error instanceof Error ? error.message : "Validation failed.";
  output.append(message);
}

function initializeDemo(root: DemoRoot): void {
  if (root.dataset.validatorInitialized === "true") {
    return;
  }
  root.dataset.validatorInitialized = "true";

  const fileInput = root.querySelector<HTMLInputElement>(
    "[data-validator-file]",
  );
  const profileInput = root.querySelector<HTMLSelectElement>(
    "[data-validator-profile]",
  );
  const validateButton = root.querySelector<HTMLButtonElement>(
    "[data-validator-submit]",
  );
  const fileName = root.querySelector<HTMLElement>(
    "[data-validator-file-name]",
  );
  const output = root.querySelector<HTMLElement>("[data-validator-output]");

  if (!fileInput || !profileInput || !validateButton || !fileName || !output) {
    return;
  }

  let selectedFile: File | undefined;

  fileInput.addEventListener("change", () => {
    selectedFile = fileInput.files?.[0];
    fileName.textContent = selectedFile
      ? `${selectedFile.name} (${formatBytes(selectedFile.size)})`
      : "No file selected";
    validateButton.disabled = !selectedFile;
    output.replaceChildren();
    output.className = "validator-result";
  });

  validateButton.addEventListener("click", async () => {
    if (!selectedFile) {
      return;
    }
    if (selectedFile.size > MAX_FILE_BYTES) {
      renderError(
        output,
        `Please choose a PDF smaller than ${formatBytes(MAX_FILE_BYTES)}.`,
      );
      return;
    }

    validateButton.disabled = true;
    validateButton.classList.add("validator-button--loading");
    validateButton.setAttribute("aria-busy", "true");
    output.replaceChildren();
    output.className = "validator-result validator-result--pending";

    try {
      const bytes = new Uint8Array(await selectedFile.arrayBuffer());
      const { validatePdfBytes } = await import("page-validation-wasm");
      const report = await validatePdfBytes(
        bytes,
        profileInput.value as import("page-validation-wasm").ValidationProfile,
      );
      renderReport(output, report as ValidationReport);
    } catch (error) {
      renderError(output, error);
    } finally {
      validateButton.disabled = false;
      validateButton.classList.remove("validator-button--loading");
      validateButton.removeAttribute("aria-busy");
    }
  });
}

function initializeCurrentPage(): void {
  const root = document.querySelector<DemoRoot>("[data-page-demo]");
  if (root) {
    initializeDemo(root);
  }
}

initializeCurrentPage();

const navigation = (
  globalThis as typeof globalThis & {
    document$?: { subscribe: (callback: () => void) => void };
  }
).document$;
navigation?.subscribe(initializeCurrentPage);
