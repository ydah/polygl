# L2: language-specific HIR snapshots

Snapshots live at `<case>/<language>.hir`. They are ordinary HIR dumps and are
never shared across languages. M1 snapshots are generated from
`conformance/cases/<case>/main.rb` after type inference and monomorphization.
