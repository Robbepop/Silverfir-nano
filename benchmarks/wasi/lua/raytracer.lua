-- Ray tracer benchmark - exercises floating point math, function calls, table allocation
-- Outputs a PPM image to stdout
-- Usage: lua raytracer.lua > output.ppm

local WIDTH = 256
local HEIGHT = 256
local SAMPLES = 2
local MAX_DEPTH = 5

local sqrt = math.sqrt
local huge = math.huge
local random = math.random
local floor = math.floor

-- Vector operations using tables {x, y, z}
local function vec(x, y, z) return {x, y, z} end
local function vadd(a, b) return {a[1]+b[1], a[2]+b[2], a[3]+b[3]} end
local function vsub(a, b) return {a[1]-b[1], a[2]-b[2], a[3]-b[3]} end
local function vmul(a, s) return {a[1]*s, a[2]*s, a[3]*s} end
local function vdot(a, b) return a[1]*b[1] + a[2]*b[2] + a[3]*b[3] end
local function vlen(a) return sqrt(vdot(a, a)) end
local function vnorm(a) local l = vlen(a); return {a[1]/l, a[2]/l, a[3]/l} end
local function vmulv(a, b) return {a[1]*b[1], a[2]*b[2], a[3]*b[3]} end

-- Ray: origin + t * direction
local function ray_at(origin, dir, t)
  return {origin[1]+dir[1]*t, origin[2]+dir[2]*t, origin[3]+dir[3]*t}
end

-- Sphere intersection, returns t or nil
local function hit_sphere(center, radius, origin, dir)
  local oc = vsub(origin, center)
  local a = vdot(dir, dir)
  local half_b = vdot(oc, dir)
  local c = vdot(oc, oc) - radius * radius
  local disc = half_b * half_b - a * c
  if disc < 0 then return nil end
  local sqrtd = sqrt(disc)
  local t = (-half_b - sqrtd) / a
  if t < 0.001 then
    t = (-half_b + sqrtd) / a
    if t < 0.001 then return nil end
  end
  return t
end

-- Scene: list of spheres {center, radius, color, material}
-- material: "diffuse", "metal", "light"
local spheres = {
  {center=vec(0, -100.5, -1), radius=100, color=vec(0.5, 0.5, 0.5), mat="diffuse"},   -- ground
  {center=vec(0, 0, -1.2),    radius=0.5, color=vec(0.7, 0.3, 0.3), mat="diffuse"},    -- center
  {center=vec(-1.1, 0, -1),   radius=0.5, color=vec(0.8, 0.8, 0.8), mat="metal"},      -- left
  {center=vec(1.1, 0, -1),    radius=0.5, color=vec(0.8, 0.6, 0.2), mat="metal"},      -- right
  {center=vec(0, 1.5, -1),    radius=0.5, color=vec(3, 3, 3),       mat="light"},       -- light
  -- small spheres
  {center=vec(-0.4, -0.3, -0.5), radius=0.2, color=vec(0.2, 0.8, 0.2), mat="diffuse"},
  {center=vec(0.5, -0.3, -0.4),  radius=0.2, color=vec(0.2, 0.2, 0.9), mat="diffuse"},
}

-- Find closest hit in scene
local function trace(origin, dir)
  local best_t = huge
  local best_s = nil
  for i = 1, #spheres do
    local s = spheres[i]
    local t = hit_sphere(s.center, s.radius, origin, dir)
    if t and t < best_t then
      best_t = t
      best_s = s
    end
  end
  return best_t, best_s
end

-- Random point in unit sphere (rejection sampling)
local function random_in_sphere()
  while true do
    local x = random() * 2 - 1
    local y = random() * 2 - 1
    local z = random() * 2 - 1
    if x*x + y*y + z*z < 1 then
      return vec(x, y, z)
    end
  end
end

-- Reflect vector v around normal n
local function reflect(v, n)
  return vsub(v, vmul(n, 2 * vdot(v, n)))
end

-- Compute color for a ray
local function ray_color(origin, dir, depth)
  if depth <= 0 then return vec(0, 0, 0) end

  local t, sphere = trace(origin, dir)
  if not sphere then
    -- Sky gradient
    local unit = vnorm(dir)
    local a = 0.5 * (unit[2] + 1.0)
    return vadd(vmul(vec(1, 1, 1), 1.0 - a), vmul(vec(0.5, 0.7, 1.0), a))
  end

  local hit_p = ray_at(origin, dir, t)
  local normal = vnorm(vsub(hit_p, sphere.center))

  if sphere.mat == "light" then
    return sphere.color
  elseif sphere.mat == "metal" then
    local reflected = reflect(vnorm(dir), normal)
    if vdot(reflected, normal) > 0 then
      return vmulv(sphere.color, ray_color(hit_p, reflected, depth - 1))
    end
    return vec(0, 0, 0)
  else -- diffuse
    local target = vadd(vadd(hit_p, normal), random_in_sphere())
    local new_dir = vsub(target, hit_p)
    return vmulv(vmul(sphere.color, 0.5), ray_color(hit_p, new_dir, depth - 1))
  end
end

-- Camera
local origin = vec(0, 0.5, 1)
local lower_left = vec(-2, -1, -2)
local horizontal = vec(4, 0, 0)
local vertical = vec(0, 2.25, 0)

-- Seed RNG
math.randomseed(42)

-- Render
local pixels = {}
local pixel_count = 0
for j = HEIGHT - 1, 0, -1 do
  for i = 0, WIDTH - 1 do
    local col = vec(0, 0, 0)
    for s = 1, SAMPLES do
      local u = (i + random()) / WIDTH
      local v = (j + random()) / HEIGHT
      local dir = vsub(
        vadd(vadd(lower_left, vmul(horizontal, u)), vmul(vertical, v)),
        origin
      )
      local c = ray_color(origin, dir, MAX_DEPTH)
      col = vadd(col, c)
    end
    col = vmul(col, 1.0 / SAMPLES)
    -- Gamma correction (gamma 2)
    local r = floor(sqrt(col[1]) * 255.99)
    local g = floor(sqrt(col[2]) * 255.99)
    local b = floor(sqrt(col[3]) * 255.99)
    if r > 255 then r = 255 end
    if g > 255 then g = 255 end
    if b > 255 then b = 255 end
    pixels[#pixels + 1] = r .. " " .. g .. " " .. b
    pixel_count = pixel_count + 1
  end
end

-- Checksum to verify correctness
local checksum = 0
for i = 1, #pixels do
  checksum = (checksum + string.byte(pixels[i], 1)) % 65536
end
if checksum == 4527 then
  print("ok")
else
  print("FAIL: expected checksum 4527, got " .. checksum)
end
