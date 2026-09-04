---
description: Validate a PDF/A document locally in your browser with page.
---

# page demo

Upload a PDF to validate it <u>locally</u> in your browser. **The document is not sent to a server**.

<div data-page-demo>
  <div class="validator-controls">
    <div class="validator-field">
      <label for="validator-file">PDF file</label>
      <input id="validator-file" data-validator-file type="file" accept="application/pdf,.pdf">
    </div>
    <div class="validator-field">
      <label for="validator-profile">Validation profile</label>
      <select id="validator-profile" data-validator-profile>
        <option value="ua1" selected>PDF/UA-1</option>
        <option value="1b">PDF/A-1b</option>
        <option value="1a">PDF/A-1a</option>
        <option value="2b">PDF/A-2b</option>
        <option value="2a">PDF/A-2a</option>
        <option value="2u">PDF/A-2u</option>
        <option value="3b">PDF/A-3b</option>
        <option value="3a">PDF/A-3a</option>
        <option value="3u">PDF/A-3u</option>
      </select>
    </div>
    <button data-validator-submit type="button" disabled><span class="validator-spinner" aria-hidden="true"></span>Validate PDF</button>
  </div>
  <p class="validator-file-name" data-validator-file-name>No file selected</p>
  <div data-validator-output aria-live="polite"></div>
</div>
