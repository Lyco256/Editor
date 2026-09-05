# Application UI library

Role: registers pure view regions while remaining independent from mutable service internals. The
central module list is foundation-owned so Wave 2 branches can work in disjoint directories. UI code
reads models, builds frame cells, and emits actions only.

