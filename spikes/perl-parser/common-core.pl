# @pgl $x: float
package Particle;

sub new {
    my ($class, $x, $y) = @_;
    my $self = { x => $x, y => $y };
    return bless $self, $class;
}

sub move {
    my ($self, $dx, $dy) = @_;
    $self->{x} = $self->{x} + $dx;
    $self->{y} = $self->{y} + $dy;
}

package main;

sub setup {
    my $count = 3;
    my $ratio = ($count * 2 + 1) / 4;
    my $wrapped = ($count + 1);
    my @values = (1, 2, 3);
    my %colors = (red => 1, blue => 2);
    my $first = $values[0];
    my $red = $colors{red};

    if ($count > 0 && $ratio < 2) {
        $count = $count + 1;
    } else {
        $count = 0;
    }

    while ($count < 5) {
        $count++;
    }

    for my $i (0 .. 2) {
        $values[$i] = $values[$i] + $count;
    }

    for (my $j = 0; $j < 3; $j++) {
        $values[$j] = $values[$j] * 2;
    }

    my $particle = Particle->new(10, 20);
    $particle->move(1, 2);
}
