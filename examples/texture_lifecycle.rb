CHECKER = texture_load("assets/checker.svg")

def setup
  size(320, 180)
  background(0.08, 0.1, 0.16)
  texture_dispose(CHECKER)
  fill(0.85, 0.9, 1.0)
  text("Texture loaded, then disposed", 16.0, 28.0)
end
