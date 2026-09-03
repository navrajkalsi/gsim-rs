# GSim-RS

![GSim Demo, simulating an Adaptive toolpath](https://github.com/navrajkalsi/gsim-rs/blob/v2/media/demo.gif?raw=true)

<div align="center" style="font-size: 0.8em;">
<i>Demo GIF is capped at 15fps. Actual simulation runs smoother.</i>
</div>

<br>

<div align="center">

[![Crates](https://img.shields.io/crates/v/gsim-rs?logo=Rust&color=%23ffaa00)](https://crates.io/crates/gsim-rs) [![Github](https://img.shields.io/badge/navrajkalsi%2Fgsim-rs?logo=GitHub&label=repo&color=%234444ff)](https://github.com/navrajkalsi/gsim-rs) [![Docs](https://img.shields.io/docsrs/gsim-rs?logo=Rust)](https://docs.rs/gsim-rs/latest/gsim_rs/)

</div>

A G-code simulator written in Rust.
Parses and interprets G-code. Manages machine state. Volumetrically simulates material-cutting and toolpaths.

The simulation is done using **WGPU** and the machine state display is built in **Ratatui**.

---

**G-code** or **Geometric code** is the language used to, *among other things*, encode instructions for a CNC machine.
These instructions cause the machine to move in extremely precise & controlled
manner to make all types of geometries.

This project aims to simulate the **Fanuc** flavour of G-code for a **vertical CNC milling** machine.

<br>

## Architecture

Here is an **extremely high level** view of the architecture:
![An extremely high level architecture diagram of GSim](https://github.com/navrajkalsi/gsim-rs/blob/v2/media/arch.svg?raw=true)

**For more information on stock simulation, read this blog: [navrajkalsi.com](https://navrajkalsi.com/blogs/gsim-rs).**

<br>

## Highlights

- **Volumetric** stock simulation is implemented for **cuboidal** stocks. This is done by only doing **partial GPU buffer updates** for each frame,
  instead of re-uploading the whole stock. The stock size can be changed using the program [configuration](#json-config).

- Inward-facing voxel faces are **hidden** at startup, and are revealed as neighbouring voxels are *removed* during a cutting move.

- **GUI** and **TUI** run on different threads and communicate bi-directionally.
  GUI handles the parsing, interpretation and simulation rendering, while TUI acts as the user frontend by rendering the active state.

- State changes flow from **GUI** to **TUI** using two methods: an `Arc<Mutex>` for skippable data,
  and a `mpsc::channel` for data that must not be dropped.

- Smooth simulation of **adaptive** or **dynamic** toolpaths *(like the one shown [here](#gsim-rs))* is ensured by batching up tiny moves before rendering them to a single frame.
  This batching is bypassed when **single-block mode** is on, giving the user instant visual feedback per move, and also allows **stepping** through the program one block at a time.

- **Orbiting**, **Panning** and **Zooming** are supported via mouse input, alongside predefined **Isometric**, **Top**, **Front** and **Right** views, which can be switched between at runtime.

- Runtime **simulation speed** controls are provided.

- **Tool size** can be changed dynamically during tool change, if the tool is defined in the program [configuration](#json-config), else the default tool is used.

- **Rapid** and **Feed** moves are differentiated visually in the simulation.

- Both **metric** & **imperial** units can be used.

<br>

## Quick Start

<details>
<summary>Dependencies</summary>

<br>

- [anyhow](https://docs.rs/anyhow/latest/anyhow/index.html)
- [bytemuck](https://docs.rs/bytemuck/latest/bytemuck/)
- [clap](https://docs.rs/clap/latest/clap/)
- [ctrlc](https://docs.rs/ctrlc/latest/ctrlc/)
- [env_logger](https://docs.rs/env_logger/latest/env_logger/)
- [log](https://docs.rs/log/latest/log/)
- [pollster](https://docs.rs/pollster/latest/pollster/)
- [ratatui](https://docs.rs/ratatui/latest/ratatui/)
- [serde](https://docs.rs/serde/latest/serde/)
- [serde_json](https://docs.rs/serde_json/latest/serde_json/)
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
  gsim-rs SOURCE_PATH # if bin is on PATH
  ```
  or
  ```bash
  cargo run --release -- SOURCE_PATH # from inside the source dir
  ```
- **Stdin**.
  ```bash
  cat SOURCE_PATH | gsim-rs # if bin is on PATH
  ```
  or
  ```bash
  cat SOURCE_PATH | cargo run --release # from inside the source dir
  ```

### JSON Config

Here is the **default** program configuration, as a sample:
``` json
{
  "units": "metric",
  "stock": {
    "x": 500,
    "y": 250,
    "z": 50
  },
  "zero_pos": {
    "x": 0,
    "y": 0,
    "z": 0
  },
  "start_pos": {
    "x": 250,
    "y": 125,
    "z": 100
  },
  "tools": [
    {
      "number": 1,
      "diameter": 20,
      "length": 125
    }
  ],
  "default_tool": {
    "number": 0,
    "diameter": 25,
    "length": 100
  }
}
```
This configuration is used if **no** *config path* is provided via the command line. Config details:
  - Treats every dimension in the *metric* system.
  - Creates a cuboid shaped stock measuring *500mm*, *250mm* & *50mm*.
  - Does not offset the reference-point of the stock, and sets it as the `zero_pos`.
  - Starts the simulation at `start_pos`, which is at middle of X and Y
    and *50mm above* the stock.
  - Creates one tool(numbered *1*), with diameter *20mm* and length *125mm*.
  - Creates a default tool config, with diameter *25mm* and length *100mm*.

A custom configuration can also be provided with a JSON file:
```bash
gsim-rs SOURCE_PATH -c CONFIG_PATH # if bin is on PATH
```
or
```bash
cargo run --release -- SOURCE_PATH -c CONFIG_PATH # from inside the source dir
```

### Runtime Commands

The following **key commands** can be used to control the simulation at **runtime**:

| **Key** | **Description** | **Default** |
| :-: | :-: | :-: |
| **q** | Quit | |
| **v** | Switch between Isometric, Top, Front, Right **views** | Isometric |
| **+** | Speed **Up** | |
| **-** | Slow **Down** | |
| **Space** | Toggle **single** block execution | Off |
| **s** | Toggle **stock** visibility | On |
| **p** | Toggle **toolpath** visibility | On |
| **t** | Toggle **tool** visibility | On |

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
- Sets up program using [default config](#json-config).

### Additional Usage
```bash
curl -k https://raw.githubusercontent.com/navrajkalsi/gsim-rs/v2/gcodes/adaptive.gcode | gsim-rs
```

- Reads G-code source from `stdin`.
- Sets up program using [default config](#json-config).

<br>

## Supported Codes
<details>
<summary><b>G Codes</b></summary>

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

</details>

<br>

<details>
<summary><b>M Codes</b></summary>

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

</details>

<br>

<details>
<summary><b>Auxiliary Codes</b></summary>

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

</details>

<br>

## Demos

### Adaptive Toolpath

Showcases high-speed adaptive machining capability. Same as the one shown [here](#gsim-rs).

*This demo uses the [default config](#json-config). Therefore we don't have to provide the config file.*

![Adaptive toolpath demo](https://github.com/navrajkalsi/gsim-rs/blob/v2/media/adaptive.gif?raw=true)

```shell
cargo run --release -- gcodes/adaptive.gcode # from inside the cloned repo
# or
gsim-rs CLONED_REPO/gcodes/adaptive.gcode # if bin is on PATH
```

### Keyboard

Demonstrates an engraving program with custom configuration.

![Keyboard outline demo](https://github.com/navrajkalsi/gsim-rs/blob/v2/media/keyboard.gif?raw=true)

```shell
cargo run --release -- gcodes/keyboard.gcode -c gcodes/keyboard.json # from inside the cloned repo
# or
gsim-rs CLONED_REPO/gcodes/keyboard.gcode -c CLONED_REPO/gcodes/keyboard.json # if bin is on PATH
```

### Cone

Demonstrates arc simulation in 3D with custom configuration.

![Cone 3D demo](https://github.com/navrajkalsi/gsim-rs/blob/v2/media/cone.gif?raw=true)

```shell
cargo run --release -- gcodes/cone.gcode -c gcodes/cone.json # from inside the cloned repo
# or
gsim-rs CLONED_REPO/gcodes/cone.gcode -c CLONED_REPO/gcodes/cone.json # if bin is on PATH
```

<br>

## Benchmarks

Benchmarks for **G-code parsing** and **stock construction** are available [here](https://github.com/navrajkalsi/gsim-rs/blob/v2/BENCHMARK.md).

<br>

## Notes
- **Offsetting** can be applied using `zero_pos` in the [config](#json-config). This is **not** to be confused with **G54** offset as this offset is always applied.
- **G04 (Dwell)** does not block the threads and is ignored silently.
- **Cutter and Tool Length Compensations** do not alter the simulation and are thus ignored.
- Unsupported codes produce an error at runtime telling exactly what is invalid, the **alphabetic** prefix or the **numeric** suffix.

<br>

## Limitations
- Only **cuboidal** stock shapes are supported.
- No runtime **offsetting** is supported, with G54-59 codes.
- Only **flat-end cylindrical** tools are supported.
- **Canned** cycles are parsed but not simulated.
- **Backtracking** of G-code is not available.

<br>

## References
**Most importantly**: [WGPU tutorial](https://sotrh.github.io/learn-wgpu/)

- 3D Math: [WebGPU Fundamentals](https://webgpufundamentals.org/webgpu/lessons/webgpu-orthographic-projection.html)
- Math for arc: [Math Stack Exchange](https://math.stackexchange.com/questions/1781438/finding-the-center-of-a-circle-given-two-points-and-a-radius-algebraically)
- Line vertex shader: [Github](https://github.com/KaNaDaAT/vega-webgpu/blob/main/src/shaders/line.wgsl)
- Points on an arc: [FreeMathHelp](https://www.freemathhelp.com/forum/threads/xy-points-on-an-arc.130791/)
- G-code: [Haas](https://www.haascnc.com/service/service-content/guide-procedures/what-are-g-codes.html#gsc.tab=0)
- Lexing & parsing: [Tomassetti](https://tomassetti.me/guide-parsing-algorithms-terminology/)
- Angle between two points on an arc: [Stackoverflow]( https://stackoverflow.com/questions/2994669/how-do-i-calculate-arc-angle-between-two-points-on-a-circle)
- Ratatui: [Docs](https://docs.rs/ratatui/latest/ratatui/)
- WGPU: [Docs](https://docs.rs/wgpu/latest/wgpu/)
