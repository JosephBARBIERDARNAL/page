# Third-party notices

Except for the third-party and reference materials expressly identified below, all project-authored material in this repository is licensed under the MIT License. The complete MIT text is provided in the repository's `LICENSE` and package-specific `LICENSE-MIT` files.

## Adobe CMap Resources

`page_validation` includes 118 Adobe CMap resources under
`src/predefined_cmaps/`. They are the byte-exact collection bundled by
veraPDF 1.30.2. Their upstream source is Adobe's CMap Resources project:

https://github.com/adobe-type-tools/cmap-resources

The individual CMap files retain their original copyright and license
notices. The collection is distributed under the following BSD 3-Clause
license:

Copyright 1990-2023 Adobe. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice,
   this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of Adobe nor the names of its contributors may be used to
   endorse or promote products derived from this software without specific
   prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

## Mozilla PDF.js encoding tables

`src/font_encodings.rs` adapts the Standard, MacRoman, MacExpert, and WinAnsi encoding tables from Mozilla PDF.js's `src/core/encodings.js`, with the table representation and lookup logic rewritten for Rust and the narrower historical MacRoman mappings preserved for behavior compatibility.

Copyright 2012 Mozilla Foundation.

The adapted material is licensed under the Apache License, Version 2.0; the complete license text is provided in the repository's `LICENSE-APACHE` file.

Upstream source: https://github.com/mozilla/pdf.js/blob/master/src/core/encodings.js

## veraPDF validation profiles and reference material

The XML validation-profile snapshots under `tests/fixtures/` and the generated rule-mapping material under `docs/rules/` are repository-only reference/test material derived from veraPDF validation profiles.

The veraPDF validation-profile repository identifies this material as licensed under CC BY 4.0. Attribution: veraPDF Consortium, https://github.com/veraPDF/veraPDF-validation-profiles, with local snapshots pinned to the versions named by their filenames and local generated mappings modified for this project.

License: https://creativecommons.org/licenses/by/4.0/

These files are excluded from the published `page_validation` crate.

## XMP schema property tables

`src/xmp2004_properties.txt` and `src/xmp2004_pdfa23_properties.txt` contain data-only namespace, property-name, and value-type tables used by the validator.

The tables represent Adobe XMP schema definitions and PDF/A predefined-property requirements; see https://developer.adobe.com/xmp/docs/xmp-namespaces/ and https://pdfa.org/resource/technical-note-tn0008-predefined-xmp-properties-in-pdfa-1/.

The table contents, selection, and representation are original to this project and are licensed under the MIT License. The referenced Adobe XMP and PDF/A materials are upstream references only; this notice does not claim ownership of their names or standards.

## Fonts embedded in repository-only PDF fixtures

Some generated PDF fixtures embed third-party fonts; those PDFs and their Typst sources are excluded from the published `page_validation` crate.

Libertinus Serif is licensed under the SIL Open Font License 1.1: https://github.com/alerque/libertinus.

DejaVu Sans Mono is distributed under the Bitstream Vera and related font notices: https://github.com/dejavu-fonts/dejavu-fonts/blob/master/LICENSE.

NewCM10-Regular is distributed under GPL-3.0-or-later with the Font Exception and Distribution Exception: https://ctan.org/pkg/newcomputermodern.

## URW StandardSymL test fixture

`tests/fixtures/fonts/usyr.pfa` and its accompanying `LICENSE.usyr` are repository-only test fixtures. The font is Copyright URW Software, 1997, and is distributed under GPL-2.0 with the embedded document-inclusion exception described in `LICENSE.usyr`.

These files are excluded from the published `page_validation` crate.

## Trademarks and standards

Adobe, Mozilla, PDF.js, veraPDF, and ISO are names or marks of their respective owners; PDF/A and PDF/UA are standards designations. They are used only to identify upstream material, standards, compatibility targets, and reference implementations. `page` is not affiliated with, sponsored by, or endorsed by Adobe, Mozilla, the PDF.js project, the veraPDF Consortium, or ISO. No trademark or other branding rights are granted by this repository.
