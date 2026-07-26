# L2: language-specific HIR snapshots

Snapshots live at `<case>/<language>.hir`. They are ordinary HIR dumps and are
never shared across languages. Ruby and PHP snapshots are generated from their
respective `main.rb` and `main.php` files after type inference and
monomorphization, even when Neutral HIR is identical.
