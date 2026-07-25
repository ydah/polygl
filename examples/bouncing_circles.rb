def setup
  size(640, 360)
end

def draw(dt)
  background(0.04, 0.05, 0.09)
  phase = (time() * 120.0) % 1040.0
  x = phase
  if phase > 520.0
    x = 1040.0 - phase
  end
  fill(0.95, 0.35, 0.45, 0.9)
  circle(x + 60.0, 120.0, 36.0)
  fill(0.25, 0.75, 1.0, 0.8)
  circle(580.0 - x, 240.0, 28.0)
end
