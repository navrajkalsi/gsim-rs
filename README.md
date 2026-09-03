# GSim-RS

![GSim Demo, simulating an Adaptive toolpath](https://github.com/navrajkalsi/gsim-rs/blob/v1/media/demo.gif?raw=true)

<div align="center">

[![Crates](https://img.shields.io/crates/v/gsim-rs?logo=Rust&color=%23ffaa00)](https://crates.io/crates/gsim-rs) [![Github](https://img.shields.io/badge/navrajkalsi%2Fgsim-rs?logo=GitHub&label=repo&color=%234444ff)](https://github.com/navrajkalsi/gsim-rs) [![Docs](https://img.shields.io/docsrs/gsim-rs?logo=Rust)](https://docs.rs/gsim-rs/latest/gsim_rs/)

</div>

A G-code simulator written in Rust.
Parses, interprets, manages machine state and simulates the toolpaths.
The control interface is built in **Ratatui** and the simulation is done using **WGPU**.

---

**G-code** or **Geometric code** is the language used to encode instructions for a CNC
machine. These instructions cause the machine to move in extremely precise & controlled
manner to make all types of geometries.

<br>

## Architecture

I have never done system diagrams for personal projects,
but I feel like this one warrants one as there are **A LOT** of moving parts.

Here is an **extremely high level** view of the architecture:
![An extremely high level architecture diagram of GSim](https://github.com/navrajkalsi/gsim-rs/blob/v1/media/arch.svg?raw=true)

<br>

## Highlights

- **TUI** and **GUI** run on different threads using a feedback cycle, ensuring that both the
  interfaces are in sync.
- Smooth simulation of **adaptive** or **dynamic** toolpaths (like the one shown [here](#gsim-rs)) is ensured by batching up tiny moves
  before rendering them to the frame. This is bypassed on **single mode on** to give the user
  instant visual feedback, thus rendering each move irrespective of the move length.
- **Single execution** of blocks is supported, allowing stepping through blocks.
- **Rapid** and **Feed** moves are differentiated visually in the simulation.
- **Isometric** and **Top** simulation views can be switched between, at runtime.
- **Machine boundary box** can be activated to visualize the extremes of machine travels.
- Parsing and interpretation only happen during the first cycle and are **cached**. This makes
  subsequent cycles more efficient.
- **Overtravel** is calculated before each move and an error is raised if the move will
  cause the machine to go off the boundary.
- Both **metric** & **imperial** units can be used.

<br>

## Quick Start

<details>
<summary>Dependencies</summary>

<br>

- [anyhow](https://docs.rs/anyhow/latest/anyhow/index.html)
- [bytemuck](https://docs.rs/bytemuck/latest/bytemuck/)
- [clap](https://docs.rs/clap/latest/clap/)
- [env_logger](https://docs.rs/env_logger/latest/env_logger/)
- [log](https://docs.rs/log/latest/log/)
- [pollster](https://docs.rs/pollster/latest/pollster/)
- [ratatui](https://docs.rs/ratatui/latest/ratatui/)
- [thiserror](https://docs.rs/thiserror/latest/thiserror/)
- [wgpu](https://docs.rs/wgpu/latest/wgpu/index.html)
- [winit](https://docs.rs/winit/latest/winit/)

</details>

### 1. Install **Rust** and **Cargo**

Install using [`rustup`](https://doc.rust-lang.org/cargo/getting-started/installation.html).

### 2a. Install from **crates.io**

```bash
cargo install gsim-rs
```

### 2b. Build from the **source**

```bash
git clone https://github.com/navrajkalsi/gsim-rs --depth 1
cd gsim-rs
cargo build --release
```

<br>

## Usage

### G-code Source

There are two ways to provide the G-code file:
- **Filepath** argument.
  ```bash
  gsim-rs FILEPATH # if bin is on PATH
  ```
  or
  ```bash
  cargo run --release -- FILEPATH # from inside the source dir
  ```
- **Stdin**.
  ```bash
  cat FILEPATH | gsim-rs # if bin is on PATH
  ```
  or
  ```bash
  cat FILEPATH | cargo run --release # from inside the source dir
  ```

### Command Line Options

The following **flags** can be used to alter the behaviour of the program during startup:

| **Flag** | **Description** | **Default** | **Max** | **Min** |
| :-: | :-: | :-: | :-: | :-: |
| -x | Maximum travel of the machine in X axis | 500 | 1500 | 150 |
| -y | Maximum travel of the machine in Y axis | 250 | 1000 | 100 |
| -z | Maximum travel of the machine in Z axis | 250 | 1000 | 100 |
| -h | Print help | | | |
| -V | Print version | | | |

### Runtime Commands

The following **key commands** can be used to control the simulation at **runtime**:

| **Key** | **Description** | **Default** |
| :-: | :-: | :-: |
| **Q** | Quit | |
| **v** | Switch b/w Isometric and Top **views** | Isometric |
| **s** | Toggle **single** block execution | Off |
| **t** | Toggle **tool** visibility | On |
| **g** | Toggle **grid** on XY plane | On |
| **o** | Toggle **origin** with axis indicators | On |
| **b** | Toggle **machine bounding box** | Off |

If **single block** execution is set to **On**, the following command is then made available:

| **Key** | **Description** |
| :-: | :-: |
| **n** | Proceed to **next** block |

### Default Usage
```bash
gsim-rs FILEPATH
```

By default:
- Reads the file at *FILEPATH* for G-code.
- Constructs a machine with maximum X, Y & Z axis travels of 500, 250 & 250 units respectively.

### Additional Usage
```bash
curl -k https://raw.githubusercontent.com/navrajkalsi/gsim-rs/v1/gcodes/adaptive.gcode | gsim-rs -x 1000 -y 750 -z 800
```

- Reads G-code source from `stdin`.
- Constructs a machine with maximum X, Y & Z axis travels of 1000, 750 & 800 units respectively.

<br>

## Supported Codes

<div style="display:flex; flex-direction: row;"><div style="margin: 10px;">

### G Codes
| **Code** | **Description** |
| :-: | :-: |
| **G00** | Rapid Move |
| **G01** | Feed Move |
| **G02** | Clockwise Arc Move |
| **G03** | Anti-Clockwise Arc Move |
| **G04** | Dwell |
| **G17** | XY Plane Selection |
| **G18** | XZ Plane Selection |
| **G19** | YZ Plane Selection |
| **G20** | Imperial Mode |
| **G21** | Metric Mode |
| **G40** | Cutter Comp Cancel |
| **G41** | Cutter Comp Left |
| **G42** | Cutter Comp Right |
| **G43** | Tool Length Comp Add |
| **G44** | Tool Length Comp Subtract |
| **G49** | Tool Length Comp Cancel |
| **G53** | Machine Position Move |
| **G54** | Workpiece Coordinate |
| **G80** | Cancel Canned Cycles |
| **G90** | Absolute Positioning |
| **G91** | Relative Positioning |
| **G94** | Feed Per Minute |
| **G95** | Feed Per Rev |
| **G98** | Initial Level Return |
| **G99** | Retract Level Return |

</div><div style="margin: 10px;">

### M Codes
| **Code** | **Description** |
| :-: | :-: |
| **M00** | Cycle Pause |
| **M01** | Optional Cycle Pause |
| **M03** | Spindle On Forward |
| **M04** | Spindle On Reverse |
| **M05** | Spindle Stop |
| **M06** | Tool Change |
| **M08** | Coolant On |
| **M09** | Coolant Off |
| **M30** | Program End |

</div><div style="margin: 10px;">

### Auxiliary Codes
| **Code** | **Description** |
| :-: | :-: |
| **D__** | Diameter Offset Register for **G40** & **G41** |
| **F__** | Feed Rate for **G01**, **G02** & **G03** |
| **H__** | Height Offset Register for **G43** & **G44** |
| **I__** | Relative Center of Arc in X axis for **G02** & **G03** |
| **J__** | Relative Center of Arc in Y axis for **G02** & **G03** |
| **K__** | Relative Center of Arc in Z axis for **G02** & **G03** |
| **N__** | Program Line Number |
| **O__** | Program Number |
| **P__** | Dwell Time in Milliseconds |
| **R__** | Arc Radius for **G02** & **G03** |
| **S__** | Spindle Speed for **M03** & **M04** |
| **T__** | Tool Number for **M06** |
| **X__** | X Axis Position for **G00**, **G01**, **G02**, **G03** & **G53** |
| **Y__** | Y Axis Position for **G00**, **G01**, **G02**, **G03** & **G53** |
| **Z__** | Z Axis Position for **G00**, **G01**, **G02**, **G03** & **G53** |

</div></div>

### Notes
- Default value for **G54** offset is **half of each machine axis travel**. Therefore each absolute move
  will be shifted to middle of the machine if activated.
- **G04 (Dwell)** does not block the threads and is ignored silently.
- **Cutter and Tool Length Compensations** do not alter the simulation and are thus ignored.
- Unsupported codes produce an error at runtime telling exactly what is invalid, the **aplhabetic** prefix or the **numeric** suffix.

<br>

## Other Demos

### Note
**Each demo G-code file, in the [*gcodes*](https://github.com/navrajkalsi/gsim-rs/blob/v1/gcodes) directory, says what max travels that file on the first line.
These need to be provided to the program via command line args.**

---

### With **Filepath** argument

```bash
gsim-rs CLONED_REPO/gcodes/keyboard.gcode -x 850 -y 425 -z 425
```

#### Isometric
![GSim demo, drawing a keyboard from Isometric view](https://github.com/navrajkalsi/gsim-rs/blob/v1/media/keyboard_iso.png?raw=true)

#### Top
![GSim demo, drawing a keyboard from Top view](https://github.com/navrajkalsi/gsim-rs/blob/v1/media/keyboard_top.png?raw=true)

---

### From **Stdin**

```bash
curl -k https://raw.githubusercontent.com/navrajkalsi/gsim-rs/v1/gcodes/outline.gcode | gsim-rs
```

#### Isometric
![GSim demo, drawing GSim logo from Isometric view](https://github.com/navrajkalsi/gsim-rs/blob/v1/media/outline_iso.png?raw=true)

#### Top
![GSim demo, drawing GSim logo from Top view](https://github.com/navrajkalsi/gsim-rs/blob/v1/media/outline_top.png?raw=true)

## References

**Most importantly**: [WGPU tutorial](https://sotrh.github.io/learn-wgpu/)

- Math for arc: [Math Stack Exchange](https://math.stackexchange.com/questions/1781438/finding-the-center-of-a-circle-given-two-points-and-a-radius-algebraically)
- Line vertex shader: [Github](https://github.com/KaNaDaAT/vega-webgpu/blob/main/src/shaders/line.wgsl)
- Points on an arc: [FreeMathHelp](https://www.freemathhelp.com/forum/threads/xy-points-on-an-arc.130791/)
- G-code: [Haas](https://www.haascnc.com/service/service-content/guide-procedures/what-are-g-codes.html#gsc.tab=0)
- Lexing & parsing: [Tomassetti](https://tomassetti.me/guide-parsing-algorithms-terminology/)
- Angle between two points on an arc: [Stackoverflow]( https://stackoverflow.com/questions/2994669/how-do-i-calculate-arc-angle-between-two-points-on-a-circle)
- Ratatui: [Docs](https://docs.rs/ratatui/latest/ratatui/)
- WGPU: [Docs](https://docs.rs/wgpu/latest/wgpu/)

