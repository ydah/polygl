use strict;
use warnings;

sub vertex_invalid_host_call {
    my $value = random(0.0, 1.0);
    return vec4(0.0, 0.0, 0.0, 1.0);
}

sub fragment_invalid_host_call {
    return vec4(1.0, 0.0, 0.0, 1.0);
}
