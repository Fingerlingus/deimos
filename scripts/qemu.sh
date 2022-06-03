qemu-system-x86_64 \
    -bios /usr/share/ovmf/x64/OVMF.fd \
    -drive id=disk,file=image.hdd,if=none,format=raw \
    -device ahci,id=ahci \
    -device ide-hd,drive=disk,bus=ahci.0 \
    -m 1G \
    -serial stdio \
    -no-reboot \
    -no-shutdown \
    "$@"