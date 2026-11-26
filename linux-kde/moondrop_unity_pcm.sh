#!/bin/sh
# Mantener el mezclador ALSA hardware de Moondrop/KTMicro USB DSP (31b2:*) al 100% (0 dB)
# para que WirePlumber/PipeWire aplique el control de volumen por software sin saltos ni cortes.
sleep 1
for f in /proc/asound/card*/usbid; do
    [ -r "$f" ] || continue
    case "$(cat "$f" 2>/dev/null)" in
        31b2:*)
            card="${f#/proc/asound/card}"
            card="${card%/usbid}"
            /usr/bin/amixer -c "$card" sset PCM 100% unmute >/dev/null 2>&1 || true
            ;;
    esac
done
