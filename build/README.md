# Packaging

Installer definitions and packaging scripts live here. Compiled output belongs only in `target/`.

- `editor.iss` is the Inno Setup installer definition for Windows x64.
- `package.ps1` builds the release binary and produces a portable staging ZIP for CI/artifact
  inspection. The installer itself is built by `iscc` from the same release binary.
