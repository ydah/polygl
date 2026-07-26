# L2: language-specific HIR snapshots

Snapshots live at `<case>/<language>.hir`. They are ordinary HIR dumps and are
never shared across languages. Ruby, PHP, and Perl snapshots are generated from
their respective `main.rb`, `main.php`, and `main.pl` files after type inference
and monomorphization, even when Neutral HIR is identical.
