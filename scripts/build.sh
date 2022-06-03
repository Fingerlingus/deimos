#!/bin/bash

# Compile kernel
../../scripts/compile.sh || exit 1

# Bootstrap if not already done
if [ ! -d "./limine" ] 
then
    ../../scripts/bootstrap.sh || exit 2
fi

# Install the Limine BIOS stages onto the image.
./limine/limine-deploy image.hdd || exit 3
 
# Mount the loopback device.
USED_LOOPBACK=$(sudo losetup -Pf --show image.hdd) &&
 
# Format the ESP partition as FAT32.
sudo mkfs.fat -F 32 ${USED_LOOPBACK}p1 &&
 
# Mount the partition itself.
mkdir -p img_mount &&
sudo mount ${USED_LOOPBACK}p1 img_mount &&
 
# Copy the relevant files over.
sudo mkdir -p img_mount/EFI/BOOT &&
sudo cp -v deimos ../../limine.cfg limine/limine.sys img_mount/ &&
sudo cp -v limine/BOOTX64.EFI img_mount/EFI/BOOT/
 
# Sync system cache and unmount partition and loopback device.
sync &&
sudo umount img_mount &&
sudo losetup -d ${USED_LOOPBACK}
