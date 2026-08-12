<?php
function mutate($values) {
    $values[0] = 9;
}

function setup() {
    $values = [1, 2];
    mutate($values);
    $special = ["__proto__" => 3, "constructor" => 4, "" => 5];
    $total = 0;
    for ($value = 1; $value <= 3; $value++) {
        $total = $total + $value;
    }
    $index = 0;
    while ($index < 2) {
        $total = $total + $values[$index];
        $index = $index + 1;
    }
    background($values[0], $values[1], $total);
    background($special["__proto__"], $special["constructor"], $special[""]);
}
