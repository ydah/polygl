def setup
  values = [1, 2, 3]
  labels = { "__proto__" => 4, "日本語" => 5 }
  values[0] = values[1]
  labels["__proto__"] = labels["日本語"]
  line(values[0], labels["__proto__"], values[2], labels["日本語"])
end
