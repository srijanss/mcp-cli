#!/bin/sh
set -eu

[ "$(uname -s)" = "Darwin" ]
[ "$(uname -m)" = "arm64" ]

cargo test --test test_install --test test_management --test test_run --test test_doctor --test test_platform_data_home
