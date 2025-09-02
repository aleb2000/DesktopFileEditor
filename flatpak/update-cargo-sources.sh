if [ -z "$1" ]; then
	wget -O Cargo.lock https://raw.githubusercontent.com/aleb2000/DesktopFileEditor/refs/heads/master/Cargo.lock
else
	cp "$1" ./Cargo.lock
fi

python3 flatpak-builder-tools/cargo/flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json
