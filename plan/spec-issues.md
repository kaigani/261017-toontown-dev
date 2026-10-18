# Spec issues

Ambiguities, contradictions or gaps found in `spec/` during implementation. One entry per issue:
section, quote, problem, decision taken (ADR link) or question for the spec owner.

## SI-001 — 09 §4.4 video quality presumes x264-style rate control (found in C0.4)

- **Section:** 09 §4.4 Video settings.
- **Quote:** "Quality | high (≈ CRF 18 equivalent) | low / medium / high / lossless where supported, or numeric
  0–100".
- **Problem:** user decision 3 (plan §6) ships only platform H.264 encoders (`h264_mf`, `h264_videotoolbox`) in an LGPL
  FFmpeg sidecar. CRF is a rate-control mode of x264, which is GPL and excluded. Neither platform encoder implements
  CRF, and neither offers lossless H.264. "≈ CRF 18 equivalent" therefore has no direct implementation.
- **Decision taken (ADR-0008):**
  - M9 maps low / medium / high / 0–100 to each encoder's quality-based mode (Media Foundation quality,
    VideoToolbox constant quality or bit rate) and measures the mapping in experiment E-FF1;
  - "lossless where supported" is offered only for formats that are lossless: PNG-in-MOV, ProRes 4444, image
    sequences. H.264 shows no lossless choice.
- **Question for the spec owner:** confirm that "≈ CRF 18 equivalent" means a visually comparable quality target, not
  x264 CRF semantics.
