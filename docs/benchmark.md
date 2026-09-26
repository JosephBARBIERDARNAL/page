Each result is a relative speedup versus veraPDF for the same profile; higher is faster. Values use the median of 10 measured runs with 2 warmups.

!!! note "What is page FF?"

    `page FF` is page's fail-fast mode: it stops after the first failure and reports only whether the document is compliant. `page` runs the exhaustive path and collects all implemented failures.

The corpus includes the existing documents and a deterministic feature-heavy PDF with multiple pages, images, an embedded font, structure elements, optional-content groups, a name tree, and embedded files.

## feature-heavy

- Size: 4.0 MiB
- Pages: 10
- PDF/A-1b: `page` **15.85×**, `page FF` **17.42×** versus veraPDF
- PDF/A-2b: `page` **16.11×**, `page FF` **18.21×** versus veraPDF
- PDF/UA-1: `page` **15.32×**, `page FF` **17.28×** versus veraPDF

## document 1

- Size: 21.4 MiB
- Pages: 756
- PDF/A-1b: `page` **4.56×**, `page FF` **8.80×** versus veraPDF
- PDF/A-2b: `page` **4.65×**, `page FF` **9.19×** versus veraPDF
- PDF/UA-1: `page` **6.10×**, `page FF` **12.64×** versus veraPDF

## document 2

- Size: 6.5 MiB
- Pages: 51
- PDF/A-1b: `page` **7.82×**, `page FF` **17.11×** versus veraPDF
- PDF/A-2b: `page` **7.71×**, `page FF` **16.95×** versus veraPDF
- PDF/UA-1: `page` **7.28×**, `page FF` **16.23×** versus veraPDF

## document 3

- Size: 38.2 MiB
- Pages: 518
- PDF/A-1b: `page` **2.86×**, `page FF` **5.88×** versus veraPDF
- PDF/A-2b: `page` **3.11×**, `page FF` **6.41×** versus veraPDF
- PDF/UA-1: `page` **2.56×**, `page FF` **5.27×** versus veraPDF

## document 4

- Size: 9.6 MiB
- Pages: 332
- PDF/A-1b: `page` **4.02×**, `page FF` **25.40×** versus veraPDF
- PDF/A-2b: `page` **4.24×**, `page FF` **27.01×** versus veraPDF
- PDF/UA-1: `page` **3.72×**, `page FF` **23.97×** versus veraPDF

## document 5

- Size: 5.7 MiB
- Pages: 88
- PDF/A-1b: `page` **2.03×**, `page FF` **20.75×** versus veraPDF
- PDF/A-2b: `page` **2.08×**, `page FF` **21.35×** versus veraPDF
- PDF/UA-1: `page` **1.94×**, `page FF` **19.69×** versus veraPDF
