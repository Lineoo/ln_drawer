#!/usr/bin/env bash
# pwd: should be executed at workspace root
# Export regression images into the given directory, defaults to the baselines

set -e

OUT=${1:-ln_drawer/examples}

export LNDRAWER_HEADLESS=1
export LNDRAWER_TMP_SAVE=1
export LNDRAWER_FORCE_FALLBACK_ADAPTER=1
export LNDRAWER_DESKTOP_UI=1
export LNDRAWER_SCALE_FACTOR=1.5

LNDRAWER_EXPORT_PATH=$OUT/text_preview.png cargo run --example text_preview
LNDRAWER_EXPORT_PATH=$OUT/widget_showcase.png cargo run --example widget_showcase
