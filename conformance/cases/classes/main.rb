class Point
  def initialize(x, y)
    @x = x
    @y = y
  end

  def move(dx)
    @x = @x + dx
  end
end

def setup
  point = Point.new(1.0, 2.0)
  point.move(3.0)
end
