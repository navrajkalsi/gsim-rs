//! # Source
//!
//! This module is responsible for reading in **raw G-Code text**,
//! and preparing it for the [`Lexer`](crate::lexer) to be tokenized.

use crate::{Verbose, config::Config};
use std::str::Lines;

/// Represents a sanitized line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line(String);

impl Line {
    /// Extracts a string slice containing the entire [`Line`].
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Verbose for Line {
    fn verbose(&self) {
        println!(
            "\nExtracted the following line from source file:\n{}",
            self.as_str()
        )
    }
}

/// Stores the data from the source file.
/// The data is sanitized, ready to be tokenized and stored in reverse(for efficient retrieval).
///
/// A program [`Config`] is also stored which can be accessed by higher level modules.
#[derive(Clone, Debug)]
pub struct Source {
    lines: Vec<Line>,
    config: Config,
}

impl Source {
    /// Constructs a new [`Source`] by using [`Config`].
    /// File at [`Config::filepath`] is read and used as the source file.
    ///
    /// Returns a [`io::Error`](std::io::Error) on failure to *read the raw file*.
    ///
    /// See [`from_lines`](Self::from_lines) for sanitization details.
    pub fn from_config(config: Config) -> Result<Self, std::io::Error> {
        let data = std::fs::read_to_string(&config.filepath)?;

        Ok(Self::from_lines(data.lines(), config))
    }

    /// Constructs a new [`Source`], from a provided *string slice* and a [`Config`].
    ///
    /// See [`from_lines`](Self::from_lines) for sanitization details.
    pub fn from_string(data: &str, config: Config) -> Self {
        Self::from_lines(data.lines(), config)
    }

    /// Constructs a new [`Source`], from [`Lines`] and [`Config`],
    /// which is stored for access by higher level functions.
    ///
    /// Each [`Line`] is computed **eagerly** on this function call.
    ///
    /// The `Source` returned is sanitized to have **NO**:
    /// - **comments**, starting with `(`.
    /// - **deleted blocks**, starting with `/`.
    /// - **end-of-block symbol**, the `;` character.
    /// - **transmission symbol**, the `%` character.
    /// - **empty lines.**
    pub fn from_lines(lines: Lines, config: Config) -> Self {
        // remove everything from '(' to end
        let uncommented = lines.map(|line| {
            line.split('(')
                .next()
                .expect("At least one element must exist after splitting.")
                .to_string()
        });

        // remove everything from ';' to end and trim
        let nocolon = uncommented.map(|line| {
            line.split(';')
                .next()
                .expect("At least one element must exist after splitting.")
                .trim()
                .to_string()
        });

        // remove deleted blocks and control character
        let filtered = nocolon
            .filter(|line| !line.is_empty() && !line.starts_with('/') && !line.starts_with('%'));

        // reverse and collect to easily pop when needed
        Self {
            lines: filtered.rev().map(|line| Line(line)).collect(),
            config,
        }
    }

    /// Returns a reference to the stored [`Config`].
    pub fn config(&self) -> &Config {
        &self.config
    }
}

impl Iterator for Source {
    type Item = Line;

    /// **Optionally** removes and returns the next [`Line`].
    fn next(&mut self) -> Option<Self::Item> {
        self.lines.pop().map(|line| {
            if self.config.verbose {
                line.verbose();
            }
            line
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;

    const TESTFILE: &'static str = "source_test.nc";
    const TESTCODE: &'static str = "
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
        G1 Z-1.0 F300            ; Plunge to first depth (slow plunge)

        G1 X57.0 F800            ; Row 1  →
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
        G1 Z-1.0 F300
        G1 X80.0 F600            ; Bottom edge
        G1 Y80.0                 ; Right edge
        G1 X20.0                 ; Top edge
        G1 Y20.0                 ; Left edge

        ; --- Pocket Pass 2 — Z-2.0 ---
        G0 Z2.0
        G0 X23.0 Y23.0
        G1 Z-2.0 F300
        G1 X57.0 F800

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
        G1 Z-5.0 F300            ; Plunge full depth
        G41 D1                   ; Cutter comp LEFT (D1 = tool radius register)

        G1 X106.2 F600           ; Bottom edge
        G1 Y106.2                ; Right edge
        G1 X-6.2                 ; Top edge
        G1 Y-6.2                 ; Left edge — back to start

        G40                      ; Cancel cutter compensation
        G0 Z10.0

        ; --- Contour Pass 2 (finishing — full depth, no allowance) ---
        G0 X-6.0 Y-6.0
        G0 Z2.0
        G1 Z-5.0 F300
        G41 D1

        G1 X106.0 F400           ; Slower feed for finish quality
        G1 Y106.0
        G1 X-6.0
        G1 Y-6.0

        G40
        G0 Z10.0

        ; ============================================================
        ; OPERATION 3: DRILLING — 4x CORNER HOLES
        ; Hole diameter: 6mm | Depth: 8mm through
        ; Positions: X10Y10, X90Y10, X90Y90, X10Y90
        ; Canned cycle G81 (standard drill)
        ; ============================================================

        G81 R2.0 Z-8.0 F200      ; Drill canned cycle: R-plane 2mm, depth -8mm

        X10.0 Y10.0              ; Hole 1 — bottom-left
        X90.0 Y10.0              ; Hole 2 — bottom-right
        X90.0 Y90.0              ; Hole 3 — top-right
        X10.0 Y90.0              ; Hole 4 — top-left

        G80                      ; Cancel canned cycle
        G0 Z10.0

        ; ============================================================
        ; OPERATION 4: BORING / CIRCULAR POCKET
        ; Center: X50 Y50 | Diameter: 20mm | Depth: 3mm
        ; Using G2 (clockwise arc) in two depth passes
        ; ============================================================

        G0 X50.0 Y40.0           ; Move to start point (center - radius on Y)
        G0 Z2.0
        G1 Z-1.5 F200            ; First depth pass
        G2 X50.0 Y40.0 I0.0 J10.0 F500  ; Full circle CW (I=0, J=radius to center)

        G1 Z-3.0 F200            ; Second depth pass
        G2 X50.0 Y40.0 I0.0 J10.0 F500  ; Full circle

        G0 Z10.0

        ; ============================================================
        ; --- END OF PROGRAM ---
        ; ============================================================
        M9                       ; Coolant OFF
        M5                       ; Spindle OFF
        G91                      ; Relative positioning
        G28 Z0                   ; Return Z to machine home
        G90                      ; Back to absolute
        G28 X0 Y0                ; Return X/Y to machine home
        M30                      ; Program end and rewind
        %                        ; End of program flag

    ";

    const RESULT: &'static str = "O0001
        G21
        G17
        G40
        G49
        G80
        G90
        G94
        G54
        M6 T1
        G43 H1
        M3 S12000
        M8
        G0 Z10.0
        G0 X23.0 Y23.0
        G0 Z2.0
        G1 Z-1.0 F300
        G1 X57.0 F800
        G1 Y27.0
        G1 X23.0
        G1 Y31.0
        G1 X57.0
        G1 Y35.0
        G1 X23.0
        G1 Y39.0
        G1 X57.0
        G1 Y43.0
        G1 X23.0
        G1 Y47.0
        G1 X57.0
        G1 Y51.0
        G1 X23.0
        G1 Y55.0
        G1 X57.0
        G1 Y57.0
        G1 X23.0
        G0 X20.0 Y20.0
        G1 Z-1.0 F300
        G1 X80.0 F600
        G1 Y80.0
        G1 X20.0
        G1 Y20.0
        G0 Z2.0
        G0 X23.0 Y23.0
        G1 Z-2.0 F300
        G1 X57.0 F800
        G0 Z10.0
        G0 X-6.2 Y-6.2
        G0 Z2.0
        G1 Z-5.0 F300
        G41 D1
        G1 X106.2 F600
        G1 Y106.2
        G1 X-6.2
        G1 Y-6.2
        G40
        G0 Z10.0
        G0 X-6.0 Y-6.0
        G0 Z2.0
        G1 Z-5.0 F300
        G41 D1
        G1 X106.0 F400
        G1 Y106.0
        G1 X-6.0
        G1 Y-6.0
        G40
        G0 Z10.0
        G81 R2.0 Z-8.0 F200
        X10.0 Y10.0
        X90.0 Y10.0
        X90.0 Y90.0
        X10.0 Y90.0
        G80
        G0 Z10.0
        G0 X50.0 Y40.0
        G0 Z2.0
        G1 Z-1.5 F200
        G2 X50.0 Y40.0 I0.0 J10.0 F500
        G1 Z-3.0 F200
        G2 X50.0 Y40.0 I0.0 J10.0 F500
        G0 Z10.0
        M9
        M5
        G91
        G28 Z0
        G90
        G28 X0 Y0
        M30";

    #[test]
    fn good() {
        let config = Config {
            filepath: TESTFILE.to_string(),
            verbose: false,
        };

        std::fs::write(TESTFILE, TESTCODE).unwrap();
        let result: Vec<Line> = RESULT
            .lines()
            .map(|line| Line(line.trim().to_string()))
            .collect();

        // file
        let src = Source::from_config(config.clone()).unwrap();
        let collected: Vec<Line> = src.collect();
        std::fs::remove_file(TESTFILE).unwrap();
        assert_eq!(result, collected);

        // text
        let src = Source::from_string(TESTCODE, config);
        let collected: Vec<Line> = src.collect();
        assert_eq!(result, collected);
    }
}
