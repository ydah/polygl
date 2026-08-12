def setup
  total = 0
  values = [1, 2]
  values.each do |value|
    total = total + value
  end
  2.times do |index|
    total = total + index
  end
  if false
    total = total + 100
  else
    total = total + 1
  end
  line(total, 0, 1, 1)
end
