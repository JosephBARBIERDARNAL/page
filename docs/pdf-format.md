---
title: "PDF Format"
---

Each PDF standard serves a specific purpose and defines a different set of requirements that a document must meet to be considered conformant. For example, a PDF containing an informative image without alternative text is not conformant with PDF/UA, because accessible alternatives for non-text content are required.

It is important to understand the differences between these standards because, depending on the use case, only some of them may be relevant. Here, we'll briefly describe the specific characteristics and conformance levels of the main PDF standards.

## PDF/A

PDF/A is a family of ISO standards designed for the **long-term preservation and archiving of electronic documents**. Unlike regular PDF, PDF/A restricts features that could make a document dependent on external resources or difficult to reproduce reliably in the future.

### PDF/A-1

|     Property     |    Value     |
| :--------------: | :----------: |
|     Version      |   PDF 1.4    |
|   ISO Standard   | 19005-1:2005 |
| Publication year |     2005     |

PDF/A-1 is the first PDF/A standard and is based on PDF 1.4. It notably requires fonts to be embedded and prohibits features such as encryption, external content dependencies, audio, video, and JavaScript.

- **PDF/A-1b:** The "b" stands for **Basic** conformance. It focuses on preserving the document's visual appearance so that it can be reliably reproduced in the future.

- **PDF/A-1a:** The "a" stands for **Accessible** conformance. It includes all PDF/A-1b requirements and adds requirements related to the document's logical structure, such as tagged content, reading order, and language information. Despite its name, PDF/A-1a conformance does not by itself guarantee full accessibility.

### PDF/A-2

|     Property     |    Value     |
| :--------------: | :----------: |
|     Version      |   PDF 1.7    |
|   ISO Standard   | 19005-2:2011 |
| Publication year |     2011     |

PDF/A-2 builds on PDF/A-1 and is based on PDF 1.7. It supports additional PDF features, including JPEG 2000 compression, transparency, layers, and the embedding of other PDF/A files.

- **PDF/A-2a:** Provides **Level A** conformance. In addition to preserving the document's visual appearance, it requires information about its logical structure and reading order.

- **PDF/A-2b:** Provides **Level B** conformance. It focuses on preserving the document's visual appearance without requiring its logical structure to be preserved.

- **PDF/A-2u:** Provides **Level U** conformance. It includes the requirements of PDF/A-2b and additionally requires Unicode mappings for textual content, allowing text to be reliably extracted, searched, and copied.

### PDF/A-3

|     Property     |    Value     |
| :--------------: | :----------: |
|     Version      |   PDF 1.7    |
|   ISO Standard   | 19005-3:2012 |
| Publication year |     2012     |

PDF/A-3 is largely based on PDF/A-2 but introduces one important feature: it allows **arbitrary file formats to be embedded as attachments**. This makes it useful for documents that need to preserve both a human-readable PDF representation and the original or associated machine-readable data.

For example, an electronic invoice can contain its visual representation as PDF while embedding the corresponding XML invoice data.

- **PDF/A-3a:** Provides **Level A** conformance. It requires preservation of both the visual appearance and the document's logical structure.

- **PDF/A-3b:** Provides **Level B** conformance. It focuses on reliable preservation of the document's visual appearance.

- **PDF/A-3u:** Provides **Level U** conformance. It includes the requirements of PDF/A-3b and requires Unicode mappings for textual content.

### PDF/A-4

|     Property     |    Value     |
| :--------------: | :----------: |
|     Version      |   PDF 2.0    |
|   ISO Standard   | 19005-4:2020 |
| Publication year |     2020     |

PDF/A-4 is based on **PDF 2.0** and modernizes PDF/A by taking advantage of the newer PDF specification.

Unlike previous versions, PDF/A-4 no longer uses the `a`, `b`, and `u` conformance levels. The base PDF/A-4 standard defines a single conformance level, with two specialized profiles: PDF/A-4e and PDF/A-4f.

- **PDF/A-4:** The base conformance level for long-term preservation of PDF 2.0 documents. It does not require tagged PDF or a logical structure.

- **PDF/A-4e:** The "e" stands for **Engineering**. It is intended primarily for engineering and technical documents and supports features useful for this domain, including RichMedia annotations for 3D content.

- **PDF/A-4f:** The "f" stands for **Files**. It extends PDF/A-4 by allowing arbitrary files to be embedded in the PDF as associated files, making it the PDF/A-4 counterpart to the attachment capabilities commonly associated with PDF/A-3.

## PDF/UA

PDF/UA is the ISO standard for **accessible PDF documents**. "UA" stands for **Universal Accessibility**.

Unlike PDF/A, which primarily concerns long-term preservation, PDF/UA defines requirements that make PDF content accessible to people using assistive technologies such as screen readers.

Among other things, PDF/UA requires documents to provide an appropriate logical structure, correctly identify content using tags, provide meaningful text alternatives for non-text content when required, specify languages, and ensure that content can be navigated and interpreted correctly by assistive technologies.

PDF/A and PDF/UA are not mutually exclusive. A document can conform to both standards when it satisfies the requirements of each.

### PDF/UA-1

|     Property     |      Value       |
| :--------------: | :--------------: |
|     Version      |     PDF 1.7      |
|   ISO Standard   | ISO 14289-1:2014 |
| Publication year |       2014       |

PDF/UA-1 defines accessibility requirements for PDF documents based on PDF 1.7.

A conforming PDF/UA-1 document must use tagged PDF and provide a meaningful logical structure. Requirements cover areas such as headings, paragraphs, lists, tables, links, annotations, forms, alternative descriptions for non-text content, document and content languages, and the relationship between content and its semantic structure.

PDF/UA-1 conformance is primarily concerned with whether the PDF file itself satisfies the technical accessibility requirements of the standard. Accessibility in a broader context may also depend on the PDF reader, assistive technology, and the way the content is authored.

### PDF/UA-2

| Property |      Value       |
| :------: | :--------------: |
| Version  |     PDF 2.0      |
|   ISO    | ISO 14289-2:2024 |
|   Date   |       2024       |

PDF/UA-2 is the successor to PDF/UA-1 and is based on **PDF 2.0**.

It updates the accessibility requirements to align with the revised tagging and structure model introduced by PDF 2.0. PDF/UA-2 takes advantage of newer PDF 2.0 semantics and provides more precise requirements for representing accessible document structures.

As with PDF/UA-1, conformance requires the document's content and semantics to be represented in a way that assistive technologies can interpret. However, PDF/UA-2 is not simply PDF/UA-1 applied to PDF 2.0: some structural concepts and requirements differ because of changes introduced in the PDF 2.0 specification.

When implementing or validating PDF/UA support, it is therefore important to treat PDF/UA-1 and PDF/UA-2 as distinct standards rather than assuming that a document conforming to one automatically conforms to the other.
