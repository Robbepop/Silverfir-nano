print("os.clock() = " .. tostring(os.clock()))
print("os.time()  = " .. tostring(os.time()))

local t0 = os.clock()
local s = ""
for i = 1, 100000 do s = s .. "x" end
local t1 = os.clock()
print("os.clock() after work = " .. tostring(t1))
print("os.clock() delta = " .. tostring(t1 - t0))

local tt0 = os.time()
print("os.time() = " .. tostring(tt0))
print("done")
