def setup
  size(320, 180)
  colors = { "present" => 0.5 }
  # Missing map keys are a located runtime error in every build mode.
  background(colors["missing"], 0.1, 0.2)
end
