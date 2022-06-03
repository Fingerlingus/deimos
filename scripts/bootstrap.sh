#!/bin/bash

# Create an empty zeroed out 64MiB image file.
dd if=/dev/zero bs=1M count=0 seek=1024 of=image.hdd
 
# Create a GPT partition table.
parted -s image.hdd mklabel gpt
 
# Create an ESP partition that spans the whole disk.
parted -s image.hdd mkpart ESP fat32 2048s 100%
parted -s image.hdd set 1 esp on
 
# Download the latest Limine binary release.
git clone https://github.com/limine-bootloader/limine.git --branch=v3.0-branch-binary --depth=1
 
# Build limine-deploy.
make -C limine
