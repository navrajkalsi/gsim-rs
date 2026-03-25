; ============================================================
; Standard GCode Example — 3-Axis CNC Mill
; Operation:  Pocket + Contour on a 100mm x 100mm workpiece
; Material:   Aluminum 6061
; Tool:       6mm 2-flute flat end mill
; Units:      Millimeters | Absolute positioning
; WCS:        G54 (work zero = top-left corner of stock, Z0 = top surface)
; ============================================================

; --- PROGRAM HEADER ---
%                        ; Program start flag (required by some controllers)
O0001                    ; Program number

; --- MACHINE INITIALIZATION ---
G21                      ; Units: millimeters
G17                      ; XY plane selection
G40                      ; Cancel cutter compensation
G49                      ; Cancel tool length offset
G80                      ; Cancel canned cycles
G90                      ; Absolute positioning
G94                      ; Feed rate: units per minute
G54                      ; Work coordinate system 1

; ============================================================
; TOOL CHANGE — T1: 6mm Flat End Mill
; ============================================================
M6 T1                    ; Tool change to tool 1
G43 H1                   ; Apply tool length offset #1
M3 S12000                ; Spindle ON, clockwise, 12000 RPM
M8                       ; Coolant ON

G0 Z10.0                 ; Safe Z clearance above work

; ============================================================
; OPERATION 1: RECTANGULAR POCKET
; Pocket size:  60mm x 60mm
; Position:     X20 Y20 (bottom-left corner)
; Depth:        -5mm total, 1mm per pass
; Step-over:    4mm (66% of tool diameter)
; ============================================================

; --- Pocket Pass 1 — Z-1.0 ---
G0 X23.0 Y23.0           ; Move above pocket start (3mm inset from corner)
G0 Z2.0                  ; Approach Z
G1 Z-1.0 F300.            ; Plunge to first depth (slow plunge)

G1 X57.0 F800.            ; Row 1  →
G1 Y27.0                 ; Step over
G1 X23.0                 ; Row 2  ←
G1 Y31.0
G1 X57.0                 ; Row 3  →
G1 Y35.0
G1 X23.0                 ; Row 4  ←
G1 Y39.0
G1 X57.0                 ; Row 5  →
G1 Y43.0
G1 X23.0                 ; Row 6  ←
G1 Y47.0
G1 X57.0                 ; Row 7  →
G1 Y51.0
G1 X23.0                 ; Row 8  ←
G1 Y55.0
G1 X57.0                 ; Row 9  →
G1 Y57.0                 ; Step to near top
G1 X23.0                 ; Row 10 ←

; --- Pocket finishing pass (perimeter cleanup) at Z-1.0 ---
G0 X20.0 Y20.0
G1 Z-1.0 F300.
G1 X80.0 F600.            ; Bottom edge
G1 Y80.0                 ; Right edge
G1 X20.0                 ; Top edge
G1 Y20.0                 ; Left edge

; --- Pocket Pass 2 — Z-2.0 ---
G0 Z2.0
G0 X23.0 Y23.0
G1 Z-2.0 F300.
G1 X57.0 F800.

; --- Pocket Passes 3–5 (Z-3, Z-4, Z-5) ---
; (Pattern repeats identically — abbreviated here)
; In production GCode these would be fully expanded or use a sub-program call.

G0 Z10.0                 ; Retract to safe Z

; ============================================================
; OPERATION 2: OUTER CONTOUR
; Profile cut around full 100mm x 100mm part perimeter
; Climb milling, full depth in two passes
; Allowance: 0.2mm left on first pass, finish on second
; ============================================================

; --- Contour Pass 1 (roughing — Z-5.0, 0.2mm radial allowance) ---
G0 X-6.2 Y-6.2           ; Start outside part (tool radius + allowance)
G0 Z2.0
G1 Z-5.0 F300.            ; Plunge full depth
G41 D1                   ; Cutter comp LEFT (D1 = tool radius register)

G1 X106.2 F600.           ; Bottom edge
G1 Y106.2                ; Right edge
G1 X-6.2                 ; Top edge
G1 Y-6.2                 ; Left edge — back to start

G40                      ; Cancel cutter compensation
G0 Z10.0

; --- Contour Pass 2 (finishing — full depth, no allowance) ---
G0 X-6.0 Y-6.0
G0 Z2.0
G1 Z-5.0 F300.
G41 D1

G1 X106.0 F400.           ; Slower feed for finish quality
G1 Y106.0
G1 X-6.0
G1 Y-6.0

G40
G0 Z10.0

; ============================================================
; OPERATION 4: BORING / CIRCULAR POCKET
; Center: X50 Y50 | Diameter: 20mm | Depth: 3mm
; Using G2 (clockwise arc) in two depth passes
; ============================================================

G0 X50.0 Y40.0           ; Move to start point (center - radius on Y)
G0 Z2.0
G1 Z-1.5 F200.            ; First depth pass
G2 X50.0 Y40.0 I0.0 J10.0 F500.  ; Full circle CW (I=0, J=radius to center)

G1 Z-3.0 F200.            ; Second depth pass
G2 X50.0 Y40.0 I0.0 J10.0 F500.  ; Full circle

G0 Z10.0

; ============================================================
; --- END OF PROGRAM ---
; ============================================================
M9                       ; Coolant OFF
M5                       ; Spindle OFF
G53 Z0.                   ; Return Z to machine home
G53 X0. Y0.                ; Return X/Y to machine home
M30                      ; Program end and rewind
%                        ; End of program flag
