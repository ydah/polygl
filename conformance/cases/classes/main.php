<?php
class Point {
    function __construct(float $x, float $y) {
        $this->x = $x;
        $this->y = $y;
    }

    function move(float $dx) {
        $this->x = $this->x + $dx;
    }
}

function setup() {
    $point = new Point(1.0, 2.0);
    $point->move(3.0);
}
