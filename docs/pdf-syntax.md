---
title: "PDF syntax"
---

!!! note

      This page is inspired by the [PDF Association guide / VSX extension](https://github.com/pdf-association/pdf-cos-syntax) for learning PDF syntax.


## The shape of a PDF file

A traditional PDF has four parts, in this order:

- A header, such as `%PDF-1.4`, declaring the file format version.
- A body containing indirect objects.
- A cross-reference table, or `xref`, indexing the byte offset of each object.
- A trailer containing the entry points needed to read the file, followed by `startxref` and `%%EOF`.

The parser starts at the end of the file, reads `startxref`, jumps to the cross-reference table, finds the trailer, and then follows references into the body.

Whitespace is generally insignificant between tokens. Newlines are useful for humans, however, and the byte offsets in the cross-reference table must count every byte, including spaces and newlines.

## A complete minimal PDF

Copy the following text into a file named `hello.pdf`. Use Unix line endings (`LF`) and save it as bytes without a text encoding conversion. The second line is an ordinary ASCII comment; a production PDF commonly uses four bytes with values above 127 there to signal that binary data may occur later.

```text
%PDF-1.4
%PDF
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>
endobj
4 0 obj
<< /Length 43 >>
stream
BT
/F1 24 Tf
72 720 Td
(Hello, PDF!) Tj
ET
endstream
endobj
5 0 obj
<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>
endobj
xref
0 6
0000000000 65535 f 
0000000014 00000 n 
0000000063 00000 n 
0000000120 00000 n 
0000000246 00000 n 
0000000338 00000 n 
trailer
<< /Size 6 /Root 1 0 R >>
startxref
408
%%EOF
```

Open the file in a PDF viewer. It should display `Hello, PDF!` near the top of a US Letter-sized page.

### Reading the example

`1 0 obj` begins object 1, generation 0. Its dictionary says that object 1 is the document catalog and that the page tree starts at object 2. The reference `2 0 R` means “object 2, generation 0”.

Object 2 is a `/Pages` node. It has one child, object 3, and `/Count 1` records the number of pages below it. Object 3 is the actual page and points back to its parent, to its content stream, and to its resources.

Object 4 is a stream containing page drawing instructions. `/Length 43` is the number of bytes between the newline after `stream` and the newline before `endstream`; it is not the length of the whole object.

Object 5 is a built-in Type 1 font. `/Helvetica` is one of the standard 14 PDF fonts, so the file does not need to embed a font program.

The `xref` table has one entry for object number 0 and one entry for each object through 5. The offsets are zero-based byte positions from the beginning of the file: object 1 starts at byte 14, object 2 at byte 63, and so on. Every entry is exactly 20 bytes in this traditional form: a ten-digit offset, a space, a five-digit generation number, a space, a status letter, and a line ending.

The object-0 entry is always the free-object head with generation `65535`. The `trailer` says there are six entries and identifies object 1 as the root catalog. `startxref` points to byte 408, where the word `xref` begins. `%%EOF` marks the end of the file.

## Tokens and basic objects

PDF syntax is made from a small set of tokens. The most common ones are:

```text
% comment
/Name
(literal string)
<486578>                 hexadecimal string: "Hex"
true false null
123 -4 3.14
[1 2 /Name (text)]       array
<< /Key value >>         dictionary
```

Names start with `/` and are case-sensitive: `/Type`, `/type`, and `/TYPE` are different names. A name can contain almost any byte except whitespace and delimiters; special bytes are written with a hexadecimal escape, such as `/A#20B` for `A B`.

Literal strings are enclosed in parentheses. A backslash escapes special characters: `\(`, `\)`, and `\\` represent delimiters and a backslash; `\n`, `\r`, `\t`, `\b`, and `\f` represent control characters. Parentheses nested inside a string must be escaped or balanced.

Arrays are ordered collections. Dictionaries are unordered name/value maps enclosed by `<<` and `>>`. A dictionary value can be another dictionary, an array, a string, a number, or an indirect reference.

An indirect object has this form:

```text
object-number generation-number obj
object-value
endobj
```

The generation is normally `0` in a newly written file. References use the form `object-number generation-number R`, for example `5 0 R`.

## The document and page trees

The catalog is the root object named by `/Root` in the trailer. A minimal catalog points to a page tree:

```text
<< /Type /Catalog /Pages 2 0 R >>
```

A page tree groups pages and may contain nested `/Pages` nodes. A leaf page has `/Type /Page`, a `/Parent`, and a `/MediaBox`:

```text
<<
  /Type /Page
  /Parent 2 0 R
  /MediaBox [0 0 612 792]
  /Resources << ... >>
  /Contents 4 0 R
>>
```

Coordinates are measured in points, where 72 points equal one inch. The default origin is the lower-left corner of the page. The media box above is 612 by 792 points, or 8.5 by 11 inches.

For two pages, add another page object, include it in `/Kids`, and change `/Count`:

```text
<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 2 >>
```

The new object must also receive a cross-reference entry, and all offsets at or after the inserted bytes must be recalculated.

## Content streams and graphics operators

A stream is a dictionary followed by the markers `stream` and `endstream`. Its dictionary must include the byte length of the stream data:

```text
4 0 obj
<< /Length 43 >>
stream
BT
/F1 24 Tf
72 720 Td
(Hello, PDF!) Tj
ET
endstream
endobj
```

The stream contains operators followed by their operands. In the example:

- `BT` and `ET` begin and end a text object.
- `/F1 24 Tf` selects resource `/F1` at 24 points.
- `72 720 Td` moves the text position to `(72, 720)`.
- `(Hello, PDF!) Tj` shows the string.

Here is a slightly richer text stream:

```text
BT
/F1 18 Tf
72 720 Td
(First line) Tj
0 -24 Td
(Second line) Tj
ET
```

Common path operators include `m` for move-to, `l` for line-to, `c` for a cubic Bézier curve, `re` for rectangle, `S` for stroke, `f` for fill, and `n` for end-path without painting. `q` saves the graphics state and `Q` restores it.

For example, this stream draws a filled rectangle:

```text
q
0.2 0.6 0.9 rg
72 600 200 80 re
f
Q
```

`rg` sets the non-stroking RGB color. The three components range from 0 to 1. A color set with `RG` applies to strokes instead.

## Resources and fonts

Operators refer to resources by name, so the page must map each name to an object. The example maps `/F1` to the Helvetica font object:

```text
/Resources <<
  /Font << /F1 5 0 R >>
>>
```

Other resource categories include `/XObject` for reusable images or forms, `/ColorSpace` for named color spaces, `/ExtGState` for graphics-state parameters, and `/Pattern` for tiling or shading patterns.
