#!/bin/bash

cargo xbuild -Z unstable-options || exit 1
exit 0
