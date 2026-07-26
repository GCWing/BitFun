Noto Sans SC Variable Font
==========================

This bundled directory keeps the variable Noto Sans SC referenced by
../fonts.css. It replaced the three static weights (Regular/Medium/SemiBold,
~12.7 MB total) with a single weight axis spanning 100-900, cutting roughly
8 MB from the installed application.

Included font files:
  variable/noto-sans-sc-<subset>-wght-normal.woff2

The files are the fontsource subset build: one woff2 per unicode-range, so a
session only decodes the ranges it actually renders. ../fonts.css declares one
@font-face per subset, all under the family name "Noto Sans SC" with
font-weight: 100 900 — the same family name the UI has always used.

Learn more about variable fonts
-------------------------------

  https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Fonts/Variable_Fonts_Guide
  https://developers.google.com/fonts/docs/getting_started

License
-------
Please read the full license text (OFL.txt, and variable/LICENSE.txt for the
fontsource build) to understand the permissions, restrictions and requirements
for usage, redistribution, and modification.

You can use them in your products & projects – print or digital,
commercial or otherwise.

This isn't legal advice, please consider consulting a lawyer and see the full
license for all details.
