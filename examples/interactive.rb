class Crosshair
  def initialize(radius)
    @radius = radius
  end

  def paint
    offsets = [-12.0, 0.0, 12.0]
    offsets.each do |offset|
      circle(offset, 0.0, 2.5)
    end
    line(-@radius, 0.0, @radius, 0.0)
    line(0.0, -@radius, 0.0, @radius)
  end
end

def setup
  size(640, 360)
end

def draw(dt)
  labels = {
    "hint" => "Move the pointer; hold Space to change color",
    "event" => "Click to draw an event marker"
  }
  background(0.04, 0.05, 0.09)
  if key_down(" ")
    stroke(1.0, 0.45, 0.25, 1.0)
  else
    stroke(0.2, 0.8, 1.0, 1.0)
  end
  push_matrix()
  translate(mouse_x(), mouse_y())
  rotate(time() * 0.6 + dt * 0.0)
  Crosshair.new(22.0).paint
  pop_matrix()
  fill(0.9, 0.92, 1.0, 1.0)
  text(labels["hint"], 16.0, 24.0)
  text(labels["event"], 16.0, 46.0)
end

def on_event(event)
  if event.kind == "pointerdown"
    fill(1.0, 0.8, 0.2, 0.9)
    circle(event.x, event.y, 14.0)
  end
end
