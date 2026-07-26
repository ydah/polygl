use strict;
use warnings;

sub vertex_invalid_string {
    my $label = "not available on the GPU";
    return vec4(0.0, 0.0, 0.0, 1.0);
}

sub fragment_invalid_string {
    return vec4(1.0, 0.0, 0.0, 1.0);
}
