def mark(value)
  background(value, 0.0, 0.0)
  value
end

def mark_bool(value)
  background(99.0, 0.0, 0.0)
  value
end

def setup
  background(2147483647 + 1, -2147483648 - 1, 0.0)
  background(-7 / 3, 7 / -3, -7 / -3)
  background(-7 % 3, 7 % -3, -7 % -3)
  false && mark_bool(true)
  true || mark_bool(false)
  background(mark(1) + mark(2), 0.0, 0.0)
  background(0.0 / 0.0, 1.0 / 0.0, -1.0 / 0.0)
  background(-0.0, floor(-2.1), trunc(-2.9))
end
