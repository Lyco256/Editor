# Rendering pipeline

Views request semantic styles and write cells to a virtual framebuffer. The terminal backend maps
roles to the detected color/underline capabilities, compares the next and previous frames, and emits
only changed regions. Higher layers never emit escape sequences.

