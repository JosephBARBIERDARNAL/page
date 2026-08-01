#set document(title: "Joseph Barbier - Resume", author: "Joseph Barbier")

#let ink = rgb("#17232d")
#let muted = rgb("#586874")
#let accent = rgb("#176b72")
#let accent-dark = rgb("#0d4950")
#let soft = rgb("#edf5f5")
#let rule = rgb("#cddcdd")

#set page(paper: "a4", margin: (x: 14mm, y: 12mm), fill: white)
#set text(size: 9.5pt, fill: ink)
#set par(leading: 0.55em, spacing: 0pt)

#set list(
  marker: text(fill: accent, weight: "bold")[•],
  indent: 9pt,
  body-indent: 3pt,
  spacing: 10pt,
)

#show link: set text(fill: accent-dark, weight: "medium")
#show link: underline

#let section-title(title) = block(above: 25pt, below: 10pt)[
  #text(
    size: 12pt,
    weight: "bold",
    fill: accent,
    tracking: 0.08em,
  )[#upper(title)]
  #v(2pt)
  #line(length: 100%, stroke: 0.7pt + rule)
]

#let contact(label, value, target: none) = block(below: 10pt)[
  #text(size: 7.8pt, weight: "bold", fill: muted)[#upper(label)]
  #linebreak()
  #if target == none {
    text(size: 8.8pt)[#value]
  } else {
    link(target)[#text(size: 8.8pt)[#value]]
  }
]

#let skill(label, value) = block(below: 4pt)[
  #v(6pt)
  #text(size: 8.7pt, weight: "bold", fill: accent-dark)[#label]
  #v(7pt)
  #text(size: 8.6pt, fill: ink)[#value]
]

#let project(name, description) = block(below: 4pt)[
  #text(size: 8.7pt, weight: "bold")[#name: #text(
      size: 8.4pt,
      fill: muted,
      weight: "regular",
    )[#description]]

]

#let experience(title, organization, dates, body) = block(above: 20pt)[
  #grid(
    columns: (1fr, auto),
    gutter: 8pt,
    align: (left, right),
    [
      #text(size: 10.7pt, weight: "bold", fill: ink)[#title]
      #v(8pt)
      #text(size: 9pt, weight: "medium", fill: accent)[#organization]
    ],
    box(
      fill: soft,
      radius: 2pt,
      inset: (x: 4pt, y: 2.2pt),
    )[
      #text(size: 8.1pt, weight: "medium", fill: accent-dark)[#dates]
    ],
  )
  #v(7pt)
  #body
]

// Header
#grid(
  columns: (1fr, auto),
  gutter: 12pt,
  align: (left, bottom),
  [
    #text(
      size: 27pt,
      weight: "bold",
      fill: ink,
      tracking: -0.02em,
    )[Joseph Barbier]
    #v(0.5cm)
    #text(size: 11.7pt, weight: "medium", fill: accent)[Independent Data Consultant]
  ],
  [
    #align(right)[
      #text(size: 8.7pt, fill: muted)[Bordeaux, France]
      #linebreak()
      #link("mailto:joseph@ysunflower.com")[
        #text(size: 8.7pt)[#str("joseph@ysunflower.com")]
      ]
    ]
  ],
)

#v(12pt)
#rect(width: 100%, height: 2pt, fill: accent, radius: 1pt)
#v(12pt)

#grid(
  columns: (33%, auto),
  gutter: 12pt,
  // Sidebar
  [
    #block(
      width: 100%,
      fill: soft,
      radius: 3pt,
      inset: 9pt,
    )[
      #section-title[PROFILE]
      #text(size: 8.9pt)[
        Data scientist and founder of *Yellow Sunflower*, specializing in reproducible reporting, scientific visualization, and open-source tooling. I build software that helps research teams and organizations turn complex data into reliable analyses and polished reports.
      ]

      #section-title[CONTACT]
      #contact(
        "Website",
        "barbierjoseph.com",
        target: "https://barbierjoseph.com/",
      )
      #contact(
        "LinkedIn",
        "joseph-barbier",
        target: "https://www.linkedin.com/in/joseph-barbier-4ab531224/",
      )
      #contact(
        "GitHub",
        "JosephBARBIERDARNAL",
        target: "https://github.com/JosephBARBIERDARNAL",
      )

      #section-title[CORE SKILLS]
      #v(-5pt)
      #skill(
        "Data",
        "Python · R · Data Visualization · Statistics · Machine Learning",
      )
      #skill("Reporting", "Typst · Quarto · HTML/CSS/JS")
      #skill("Tooling & Others", "Git · CI/CD · Rust")

      #section-title[OPEN-SOURCE WORK]
      #text(size: 8.8pt, fill: muted)[
        Creator and maintainer of open-source Python and R libraries for data visualization, reporting, and developer tooling, accumulating *1,000+ GitHub stars* and nearly *1 million downloads*.
      ]

      #section-title[EDUCATION]
      #text(size: 8.9pt, weight: "bold")[MSc, Applied Mathematics & Statistics]
      #linebreak()
      #text(size: 8.4pt, fill: muted)[University of Bordeaux · 2022-2024]
      #v(8pt)
      #text(size: 8.9pt, weight: "bold")[BSc, Economics]
      #linebreak()
      #text(size: 8.4pt, fill: muted)[University of Bordeaux · 2019-2022]

      #section-title[LANGUAGES]
      #text(size: 8.7pt)[French: Native]
      #linebreak()
      #text(size: 8.7pt)[English: Professional]
    ]
  ],
  // Main column
  [
    #section-title[PROFESSIONAL EXPERIENCE]

    #experience(
      "Independent Data Consultant",
      "Yellow Sunflower",
      "Since 2024",
    )[
      - Design automated reporting systems that transform spreadsheets,
        databases, APIs, and operational data into reliable PDF and web outputs.
      - Develop and maintain open-source Python and R libraries focused on reporting, visualization, and reproducible workflows.
      - Talks and trainings at international events
    ]

    #experience(
      "Co-creator",
      "Matplotlib Journey",
      "Since 2024",
    )[
      - Co-created #link("https://www.matplotlib-journey.com/")[
          matplotlib-journey.com
        ] with Yan Holtz, a leading data visualization specialist.
      - Enrolled 600+ students worldwide since launch.
      - Built a complete interactive learning platform featuring in-browser Python
        sandboxes, hands-on lessons, quizzes, and zero-install coding powered by
        Pyodide.
    ]

    #experience(
      "Data Science and Open Source Intern",
      "Dataviz Universe",
      "Mar 2024 - Aug 2024",
    )[
      - Developed open-source software for data analysis, including Lifelihood,
        an R package for ecology modelling, and the first version of pypalettes.
      - Created advanced Matplotlib visualizations and applied information-design
        principles to communicate data clearly.
      - Developed and improved content and tooling for the Python and R Graph
        Gallery websites.
    ]

    #experience(
      "Data Science Intern - E-commerce",
      "Wanteeed",
      "Aug 2023 - Jan 2024",
    )[
      - Queried, cleaned, and transformed product and customer data with
        BigQuery SQL.
      - Built interactive Looker dashboards and defined business KPIs.
      - Developed and implemented predictive models in Python using Google Colab.
    ]

    #experience(
      "Freelance Data Visualization & Web Dev",
      "R Graph Gallery · Python Graph Gallery",
      "May 2023 - Aug 2023",
    )[
      - Delivered data visualization and web-development projects while
        completing a master's degree, averaging 10 hours per week.
      - Improved educational chart examples and supporting website content for
        Python and R audiences.
    ]

    #experience(
      "Research Intern - Socio-economics",
      "CIERI, Bordeaux",
      "May 2022 - Jul 2022",
    )[
      - Investigated relationships between socio-economic status and cancer
        through literature review and quantitative analysis.
      - Applied statistical inference and unsupervised learning methods in R.
    ]
  ],
)
