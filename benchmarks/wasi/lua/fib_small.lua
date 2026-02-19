local function fib(n)
  if n < 2 then return n end
  return fib(n-1) + fib(n-2)
end

local result = fib(20)
print(string.format("fib(20) = %d", result))
