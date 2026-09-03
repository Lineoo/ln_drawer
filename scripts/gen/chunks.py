import random
import math

# mysterious seed number :]
random.seed(70814 ^ 42)

# presets
half, samples = 64, [10, 7, 4, 2, 2, 2, 2, 3, 3] # random
# half, samples = 16, [8, 8, 4, 0, 0, 0, 0, 0, 0] # key

selected = []
for z in range(8, -1, -1):
    chunks = []
    zhalf = math.ceil(half / (1 << z))
    for x in range(-zhalf, zhalf):
        for y in range(-zhalf, zhalf):
            chunks.append((x, y, z))
    selected += random.sample(chunks, min(len(chunks), samples[z]))


with open("chunks.txt", "w") as f:
    for b in selected:
        f.write(f"({b[0]},{b[1]},{b[2]}),\n")
print("=> output has written to ./chunks.txt")