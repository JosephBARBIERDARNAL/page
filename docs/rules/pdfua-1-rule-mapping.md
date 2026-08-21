# PDF/UA-1 rule mapping

The locally implemented PDF/UA-1 rule mapping currently includes the following encryption predicate.

| Local rule | veraPDF rule | Strength | Implementation |
| `PDFUA1-ENCRYPTION-P-001` | `ISO 14289-1:2014:7.16:1` | exact | Encrypted files must expose an encryption-dictionary `/P` value with bit 10 (`512`) set. |

The check intentionally covers only rule 7.16-1; it does not infer or validate unrelated encryption-handler properties. Its fixtures include an opt-in differential test for veraPDF 1.30.2 via `VERAPDF_BIN`. The missing-`/P` fixture is intentionally recorded as a reference-parser discrepancy because veraPDF rejects the encrypted file before evaluating rule 7.16-1, while `page` retains the encryption metadata and reports the local rule failure.
