use strict;
use warnings;

my $CUBE_MESH = mesh_box(1.4, 1.4, 1.4);
my $BLUE_MATERIAL = material_basic(vec4(0.15, 0.55, 1.0, 1.0));
my $ORANGE_MATERIAL = material_basic(vec4(1.0, 0.35, 0.08, 1.0));
my $LEFT_CUBE = node_add($CUBE_MESH, $BLUE_MATERIAL);
my $RIGHT_CUBE = node_add($CUBE_MESH, $ORANGE_MATERIAL);

sub setup {
    size(640, 360);
    background(0.025, 0.035, 0.07);
    camera_perspective(0.9, 0.1, 100.0);
    camera_look_at(vec3(0.0, 2.4, 7.0), vec3(0.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0));
    light_directional(vec3(-0.7, -1.0, -0.4), vec3(1.0, 0.92, 0.82));
    node_set_pos($LEFT_CUBE, -1.35, 0.0, 0.0);
    node_set_pos($RIGHT_CUBE, 1.35, 0.0, 0.0);
}

sub frame {
    my ($dt) = @_;
    background(0.025, 0.035, 0.07);
    my $angle = time();
    node_set_rot($LEFT_CUBE, $angle * 0.7, $angle, $angle * 0.2);
    node_set_rot($RIGHT_CUBE, -$angle * 0.4, -$angle * 0.8, $angle * 0.3);
}
