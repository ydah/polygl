<?php
const CUBE = mesh_box(1.2, 1.2, 1.2);
const COOL = material_basic(vec4(0.15, 0.5, 1.0, 1.0));
const WARM = material_basic(vec4(1.0, 0.28, 0.06, 1.0));
const LEFT = node_add(CUBE, COOL);
const RIGHT = node_add(CUBE, WARM);

function setup() {
    size(48, 32);
    background(0.02, 0.03, 0.06);
    camera_perspective(0.9, 0.1, 50.0);
    camera_look_at(vec3(0.0, 1.8, 6.0), vec3(0.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0));
    light_directional(vec3(-0.7, -1.0, -0.5), vec3(1.0, 0.9, 0.75));
    node_set_pos(LEFT, -1.0, 0.0, 0.0);
    node_set_pos(RIGHT, 1.0, 0.0, 0.0);
    node_set_rot(LEFT, 0.25, 0.45, 0.0);
    node_set_rot(RIGHT, -0.2, -0.5, 0.15);
}
