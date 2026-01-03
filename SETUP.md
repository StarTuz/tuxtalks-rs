# TuxTalks-rs Setup Guide

## Prerequisites

### 1. Install vosk-api library

```bash
sudo pacman -S vosk-api   # Arch/Garuda
```

### 2. Download Vosk Model

```bash
mkdir -p ~/.local/share/vosk
cd ~/.local/share/vosk
wget https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip -O model.zip
unzip model.zip
mv vosk-model-small-en-us-0.15 model
rm model.zip
```

For better accuracy, use `vosk-model-en-us-0.22` (larger, ~1GB).

## Build & Run

```bash
cd ~/Code/tuxtalks-rs
cargo build --release
./target/release/tuxtalks --verbose
```

## Audio Device Selection

Use `--device <INDEX>` to select a specific microphone:

```bash
./target/release/tuxtalks --device 17  # Headset
```

Device list is printed at startup.

## Troubleshooting

### Empty Partials

If you see many `Partial:` lines with no text, the microphone may not be picking up audio. Try:

1. Selecting a different device with `--device`
2. Checking your audio levels in pavucontrol
