# L3: neutral HIR equality

Expected normalized dumps live at `<case>/neutral.hir`. Only cases using the
Neutral subset may enter this layer. Until a second adapter lands, the Ruby
module is checked against this canonical snapshot; later adapters must also
produce the same normalized HIR.
