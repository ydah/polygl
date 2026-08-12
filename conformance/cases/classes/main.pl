use strict;
use warnings;

package Point;

sub new {
    my ($class, $x, $y) = @_;
    my $self = { x => $x, y => $y };
    return bless $self, $class;
}

sub move {
    my ($self, $dx) = @_;
    $self->{x} = $self->{x} + $dx;
}

package main;

sub setup {
    my $point = Point->new(1.0, 2.0);
    $point->move(3.0);
}
