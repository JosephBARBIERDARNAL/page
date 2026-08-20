# PDF/UA-1 rule 7.2-42

`page` enforces that every table row has the same effective column count, placing cells in the table grid and applying each cell's `/ColSpan`; active `/RowSpan` cells continue to occupy their columns on later rows.

The veraPDF 1.30.2 PDF/UA-1 profile also contains companion test 7.2-43 with a different `wrongColumnSpan` branch; the focused unequal-`ColSpan` fixture triggers 7.2-42 directly, while 7.2-43 remains outside this implementation's scope.
