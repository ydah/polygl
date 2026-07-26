def setup
  size(640, 480)
  material = material_shader("plasma")
  background(0.02, 0.02, 0.05)
end

def vertex_plasma
  vec4(0.0, 0.0, 0.0, 1.0)
end

def fragment_plasma
  phase = time() / 5.0
  vec4(phase, 0.15, 0.8, 1.0)
end
