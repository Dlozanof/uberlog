# Uberlog

An opinionated-ish embedded development tool that strives to be the next step in the `picocom` (or alternative) -> `logfile` -> `cat/grep` workflow. People working in many other areas have [fancy](https://log-viewer.opcodes.io/) [things](https://www.logviewplus.com/), but firmware engineers seem to think they have no right to have nice things. Just log the output of the serial port into a file and `grep` onto it, maybe some bash script to add a bit of coloring. **Not on my watch.**

This tool is pretty basic and **will not** do anything you can solve with *ix tools, so do not expect huge integrations out of the box with the propietary toolchain you are using. But being basic means it is easily pluggable: if you use UART you are good to go, and if you use RTT you just need to point to the `.elf` file you are going to flash in the device. Heck you can just open a text file and work with it.

# Features

It has some more functionality than the basic log managing:
- Add filters to modify logs containing certain expressions:
    - Highlight
    - Exclude them
    - Only include them
- Reset the device
- Export/import a logfile
- \[soon\] Manage a supported power supply
- \[soon\] Flash firmware
- \[soon\] Get logs from several devices at the same time

# Usage

The tool depends on the creation of a `.gadget.yaml` file inside the project directory that contains the information regarding your [target](https://probe.rs/targets):

```
power: !Dp100
  voltage: 5
  current: 0.5
targets:
- name: Main app (RTT)
  processor: CC2650
  log_backend: !Rtt
    elf_path: /path/to/binary.elf
  probe_id: PROBE_1_SERIAL
- name: Secondary processor (UART)
  processor: STM32F7
  log_backend: !Uart
    dev: /dev/ttyACM0
    baud: 115200
  probe_id: PROBE_2_SERIAL
```

Then you launch the tool from the project folder and it will read the `.gadget.yaml` file to know how to interact with the target. If you do not want to see the logs just use nohup:

```
$ (nohup uberlog &)
```

> See? I could have a `--fork` option or whatever, but why reinvent the wheel.

In this specific way, app logs go to /tmp/uberlog_log so if something happens you can check there for issues.
 
The fields are self explanatory, but note that `name` is whatever you want to name the target in the UI, and `processor` comes from `probe-rs` list of targets [link](https://probe.rs/targets). This means of course that the MCU management (flashing/reset/RTT) side of the project is done by the incredible `probe-rs` [project](https://probe.rs/). Go star it if you did not do it yet.

When the project is a bit more mature I will improve in the documentation, since it is quite prone to change, but I will always keep (famous last words) an up to date example here so you can just copy/paste and adapt it. StackOverflow style :D.

# Installation

So far no package manager is supported, only build for source like a madman. But assuming you have [Rust](https://www.rust-lang.org/tools/install) in your computer you can just:

```
cargo install --path .
```

And have your binary available in `$HOME/.cargo/bin/uberlog`. From this point you can do whatever you want to make the app available. Typically adding `$HOME/.cargo/bin` to your `$PATH`. But if you are reading this chances are you have a different way to managing binaries.
