wget -O Cargo.lock https://raw.githubusercontent.com/aleb2000/DesktopFileEditor/refs/heads/master/Cargo.lock
python3 flatpak-builder-tools/cargo/flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json
