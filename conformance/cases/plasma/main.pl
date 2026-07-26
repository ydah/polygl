use strict;
use warnings;

sub setup {
    size(64, 64);
    my $material = material_shader("plasma");
    background(0.02, 0.02, 0.05);
}

sub vertex_plasma {
    return vec4(0.0, 0.0, 0.0, 1.0);
}

sub fragment_plasma {
    return vec4(time() / 5.0, 0.15, 0.8, 1.0);
}
