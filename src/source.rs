//! # Source
//!
//! Reads in **raw G-Code text**,
//! and prepares it for the [`Lexer`](crate::lexer) to be tokenized.

use std::{
    io::{IsTerminal, Read},
    str::Lines,
};

/// Stores the data from the source file.
/// The data is sanitized, ready to be tokenized.
///
/// Also keeps track of the line to return for [`Source::next`] call.
#[derive(Clone, Debug)]
pub struct Source {
    /// Raw sanitized lines.
    lines: Vec<String>,

    /// Index of the line to return from [`Self::lines`] on [`Self::next`] call.
    ///
    /// [`Self::next`] increments it and [`Self::reload`] resets it back to `0`.
    /// At some point, this may become out of bounds of [`Self::lines`] which would mean
    /// that the `source` has been exhausted and needs to be `reload`ed.
    index: usize,
}

impl Source {
    /// Constructs a new [`Source`] by reading a file at `path`.
    ///
    /// See [`from_lines`](Self::from_lines) for sanitization details.
    ///
    /// # Errors
    /// Returns [`SourceError::IO`] on failure to *read the raw file*.
    pub fn from_file(path: &str) -> Result<Self, SourceError> {
        let data = std::fs::read_to_string(path)?;

        Ok(Self::from_lines(data.lines()))
    }

    /// Constructs a new [`Source`] by checking if `stdin` is readable,
    /// and then reading it for G-code text.
    ///
    /// See [`from_lines`](Self::from_lines) for sanitization details.
    ///
    /// # Errors
    /// Returns [`SourceError::StdinNotReadable`] on failure to *read stdin*.
    pub fn from_stdin() -> Result<Self, SourceError> {
        if !is_readable_stdin() {
            return Err(SourceError::StdinNotReadable);
        }

        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf)?;

        Ok(Self::from_str(str::from_utf8(buf.as_slice())?))
    }

    /// Constructs a new [`Source`], from a provided *string slice*.
    ///
    /// See [`from_lines`](Self::from_lines) for sanitization details.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(data: &str) -> Self {
        Self::from_lines(data.lines())
    }

    /// Constructs a new [`Source`], from [`Lines`].
    ///
    /// Each line is computed **eagerly** on this function call.
    ///
    /// The `Source` returned is sanitized to have **NO**:
    /// - **comments**, starting with `(`.
    /// - **deleted blocks**, starting with `/`.
    /// - **end-of-block symbol**, the `;` character.
    /// - **transmission symbol**, the `%` character.
    /// - **empty lines.**
    pub fn from_lines(lines: Lines) -> Self {
        // filter_map performs worse here
        let sanitized = lines
            .map(|line| {
                line.split(['(', ';'])
                    .next()
                    .expect("at least one element exists after splitting")
                    .trim() // remove everything from '(' or ';' to end
            })
            .filter(|line| !line.is_empty() && !line.starts_with('/') && !line.starts_with('%')); // delete blocks and control character

        Self {
            lines: sanitized.map(|l| l.to_string()).collect(),
            index: 0,
        }
    }

    /// Resets internal pointer to point to the beginning of the source,
    /// so that subsequent calls to [`Self::next`] return lines from the beginning.
    ///
    /// **Does not** read the source file again.
    pub fn reload(&mut self) {
        self.index = 0
    }

    /// **Optionally** returns the reference to contents of a line at provided `index`, as a `string slice`.
    pub fn get(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(|line| line.as_str())
    }

    /// Returns the total number of lines in the [`Source`].
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Returns the internal pointer index.
    ///
    /// A subsequent call to [`Self::next`] will return the line at this index.
    pub fn index(&self) -> usize {
        self.index
    }

    /// **Optionally** returns the next line as a `string slice`.
    ///
    /// **Does not** remove the returned line to support reloading the [`Source`].
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&str> {
        let line = self.lines.get(self.index)?;
        self.index += 1;

        Some(line)
    }
}

/// Returns true if and only if stdin is believed to be readable.
///
/// This is taken directly from
/// [`ripgrep`](https://github.com/BurntSushi/ripgrep/blob/master/crates/cli/src/lib.rs).
pub fn is_readable_stdin() -> bool {
    #[cfg(unix)]
    fn imp() -> bool {
        use std::{
            fs::File,
            os::{fd::AsFd, unix::fs::FileTypeExt},
        };

        let stdin = std::io::stdin();
        let fd = match stdin.as_fd().try_clone_to_owned() {
            Ok(fd) => fd,
            Err(err) => {
                log::debug!(
                    "for heuristic stdin detection on Unix, \
                     could not clone stdin file descriptor \
                     (thus assuming stdin is not readable): {err}",
                );
                return false;
            }
        };
        let file = File::from(fd);
        let md = match file.metadata() {
            Ok(md) => md,
            Err(err) => {
                log::debug!(
                    "for heuristic stdin detection on Unix, \
                     could not get file metadata for stdin \
                     (thus assuming stdin is not readable): {err}",
                );
                return false;
            }
        };
        let ft = md.file_type();
        let is_file = ft.is_file();
        let is_fifo = ft.is_fifo();
        let is_socket = ft.is_socket();
        let is_readable = is_file || is_fifo || is_socket;
        log::debug!(
            "for heuristic stdin detection on Unix, \
             found that \
             is_file={is_file}, is_fifo={is_fifo} and is_socket={is_socket}, \
             and thus concluded that is_stdin_readable={is_readable}",
        );
        is_readable
    }

    #[cfg(windows)]
    fn imp() -> bool {
        let stdin = winapi_util::HandleRef::stdin();
        let typ = match winapi_util::file::typ(stdin) {
            Ok(typ) => typ,
            Err(err) => {
                log::debug!(
                    "for heuristic stdin detection on Windows, \
                     could not get file type of stdin \
                     (thus assuming stdin is not readable): {err}",
                );
                return false;
            }
        };
        let is_disk = typ.is_disk();
        let is_pipe = typ.is_pipe();
        let is_readable = is_disk || is_pipe;
        log::debug!(
            "for heuristic stdin detection on Windows, \
             found that is_disk={is_disk} and is_pipe={is_pipe}, \
             and thus concluded that is_stdin_readable={is_readable}",
        );
        is_readable
    }

    #[cfg(not(any(unix, windows)))]
    fn imp() -> bool {
        log::debug!("on non-{{Unix,Windows}}, assuming stdin is not readable");
        false
    }

    !std::io::stdin().is_terminal() && imp()
}

/// Possible errors that can happen during [`Source`] construction.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// Failed to read the source from the file or `stdin`.
    #[error("file/stdin read failed")]
    IO(#[from] std::io::Error),

    /// Data received from `stdin` in not encoded in UTF-8.
    #[error("could not convert stdin bytes to string")]
    UTF(#[from] std::str::Utf8Error),

    /// No G-code source target provided.
    /// No source file path was supplied and the `stdin` is also not readable.
    #[error("no gcode file provided via filepath or stdin")]
    StdinNotReadable,
}

#[cfg(test)]
mod tests {
    use super::*;

    const TESTFILE: &str = "source_test.nc";
    const TESTCODE: &str = "
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
        std::fs::write(TESTFILE, TESTCODE).unwrap();
        let result: Vec<String> = RESULT.lines().map(|line| line.trim().to_string()).collect();

        // file
        let mut src = Source::from_file(TESTFILE).unwrap();
        let mut collected = Vec::with_capacity(src.len());
        while let Some(l) = src.next() {
            collected.push(l.to_string());
        }
        std::fs::remove_file(TESTFILE).unwrap();
        assert_eq!(result, collected);

        // text
        let mut src = Source::from_str(TESTCODE);
        let mut collected = Vec::with_capacity(src.len());
        while let Some(l) = src.next() {
            collected.push(l.to_string());
        }
        assert_eq!(result, collected);
    }
}
