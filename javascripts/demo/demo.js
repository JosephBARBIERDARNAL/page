function m(e) {
  return e < 1024 ? `${e} B` : e < 1024 * 1024 ? `${(e / 1024).toFixed(1)} KiB` : `${(e / (1024 * 1024)).toFixed(1)} MiB`;
}
function l(e, a) {
  const r = document.createElement(e);
  return a && (r.className = a), r;
}
function g(e, a) {
  e.replaceChildren(), e.className = a.isCompliant ? "validator-result validator-result--pass" : "validator-result validator-result--fail";
  const r = l("h3");
  r.textContent = a.isCompliant ? "Document is compliant" : "Document is not compliant", e.append(r);
  const t = a.failures ?? [];
  if (t.length > 0) {
    const s = l("p");
    s.textContent = `${t.length} validation failure${t.length === 1 ? "" : "s"}.`, e.append(s);
    const n = l("ul", "validator-failures");
    for (const i of t) {
      const o = l("li"), d = l("div", "validator-failure-header"), c = l("code", "validator-failure-rule");
      if (c.textContent = i.ruleId ?? "—", d.append(c), i.category) {
        const f = l("span", "validator-failure-category");
        f.textContent = i.category, d.append(f);
      }
      const u = l("span", "validator-failure-message");
      u.textContent = i.message ?? "—", o.append(d, u), n.append(o);
    }
    e.append(n);
  }
}
function v(e, a) {
  e.replaceChildren(), e.className = "validator-result validator-result--error";
  const r = l("p");
  r.textContent = a instanceof Error ? a.message : "Validation failed.", e.append(r);
}
function y(e) {
  if (e.dataset.validatorInitialized === "true")
    return;
  e.dataset.validatorInitialized = "true";
  const a = e.querySelector(
    "[data-validator-file]"
  ), r = e.querySelector(
    "[data-validator-profile]"
  ), t = e.querySelector(
    "[data-validator-submit]"
  ), s = e.querySelector(
    "[data-validator-file-name]"
  ), n = e.querySelector("[data-validator-output]");
  if (!a || !r || !t || !s || !n)
    return;
  let i;
  a.addEventListener("change", () => {
    i = a.files?.[0], s.textContent = i ? `${i.name} (${m(i.size)})` : "No file selected", t.disabled = !i, n.replaceChildren(), n.className = "validator-result";
  }), t.addEventListener("click", async () => {
    if (i) {
      if (i.size > 52428800) {
        v(
          n,
          `Please choose a PDF smaller than ${m(52428800)}.`
        );
        return;
      }
      t.disabled = !0, t.classList.add("validator-button--loading"), t.setAttribute("aria-busy", "true"), n.replaceChildren(), n.className = "validator-result validator-result--pending";
      try {
        const o = new Uint8Array(await i.arrayBuffer()), { validatePdfBytes: d } = await import("./index-BxDfnnUg.js"), c = await d(
          o,
          r.value
        );
        g(n, c);
      } catch (o) {
        v(n, o);
      } finally {
        t.disabled = !1, t.classList.remove("validator-button--loading"), t.removeAttribute("aria-busy");
      }
    }
  });
}
function p() {
  const e = document.querySelector("[data-page-demo]");
  e && y(e);
}
p();
const h = globalThis.document$;
h?.subscribe(p);
