def mutate(values)
  values[0] = 9
end

def setup
  values = [1, 2]
  mutate(values)
  special = {"__proto__" => 3, "constructor" => 4, "" => 5}
  total = 0
  (1..3).each do |value|
    total = total + value
  end
  index = 0
  while index < 2
    total = total + values[index]
    index = index + 1
  end
  background(values[0], values[1], total)
  background(special["__proto__"], special["constructor"], special[""])
end
