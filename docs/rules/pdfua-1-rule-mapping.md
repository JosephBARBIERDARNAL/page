# PDF/UA-1 rule mapping

The locally implemented PDF/UA-1 rule mapping currently includes the following encryption predicate.

| Local rule | veraPDF rule | Strength | Implementation |
| `PDFUA1-ENCRYPTION-P-001` | `ISO 14289-1:2014:7.16:1` | exact | Encrypted files must expose an encryption-dictionary `/P` value with bit 10 (`512`) set. |
| `PDFUA1-TRAPNET-ANNOTATION-001` | `ISO 14289-1:2014:7.18.2:1` | veraPDF-aligned | Visible, in-crop-box `/TrapNet` annotations are rejected. Hidden and outside-crop-box annotations follow veraPDF 1.30.2's explicit exemptions. |
| `PDFUA1-PAGE-TABS-001` | `ISO 14289-1:2014:7.18.3:1` | exact | Pages with a non-empty `/Annots` array must contain a direct `/Tabs /S` entry. |

The check intentionally covers only rule 7.16-1; it does not infer or validate unrelated encryption-handler properties. Its fixtures include an opt-in differential test for veraPDF 1.30.2 via `VERAPDF_BIN`. The missing-`/P` fixture is intentionally recorded as a reference-parser discrepancy because veraPDF rejects the encrypted file before evaluating rule 7.16-1, while `page` retains the encryption metadata and reports the local rule failure.

Rule 7.18.2-1 reuses the shared page annotation traversal. Its local scope intentionally mirrors veraPDF 1.30.2's profile predicate, which treats hidden and outside-crop-box annotations as exempt; visible, in-crop-box `/TrapNet` annotations fail. The pass and fail fixtures include an opt-in differential test through `VERAPDF_BIN`.

Rule 7.18.3-1 reuses the shared page list and annotation-array resolution. The local check treats a non-empty `/Annots` array as the presence of annotations and requires `/Tabs /S` directly in that page dictionary; it does not apply the requirement to pages without annotations. The pass and fail fixtures match veraPDF 1.30.2 for missing and non-`/S` `/Tabs` values through the opt-in differential test.
