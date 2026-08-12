use strict;
use warnings;

sub setup {
    my @values = (1, 2);
    $values[0] = 9;
    my %special = ("__proto__" => 3, "constructor" => 4, "" => 5);
    my $total = 0;
    for my $value (1 .. 3) {
        $total = $total + $value;
    }
    my $index = 0;
    while ($index < 2) {
        $total = $total + $values[$index];
        $index = $index + 1;
    }
    background($values[0], $values[1], $total);
    background($special{"__proto__"}, $special{"constructor"}, $special{""});
}
