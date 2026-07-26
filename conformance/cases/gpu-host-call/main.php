<?php
function vertex_invalid_host_call() {
    $value = random(0.0, 1.0);
    return vec4(0.0, 0.0, 0.0, 1.0);
}

function fragment_invalid_host_call() {
    return vec4(1.0, 0.0, 0.0, 1.0);
}
